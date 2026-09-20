use op_collab_relay_locator_server::{
    build_production_pairing, build_production_publisher, check_production, serve_listener_until,
    LocatorServerConfig, ProductionLocatorConfig,
};
use tracing_subscriber::EnvFilter;

const LOG_LEVEL_ENV: &str = "OPENPENCIL_COLLAB_LOCATOR_LOG_LEVEL";

#[tokio::main]
async fn main() {
    let filter = match locator_log_filter() {
        Ok(value) => value,
        Err(()) => {
            eprintln!("{LOG_LEVEL_ENV} must be one of error, warn, info, or debug");
            std::process::exit(2);
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .init();

    let command = match parse_arguments(std::env::args().skip(1)) {
        Ok(value) => value,
        Err(()) => {
            eprintln!(
                "usage: op-collab-relay-locator-server \
                 <--production|--check-production>"
            );
            std::process::exit(2);
        }
    };
    if command == Command::CheckProduction {
        match check_production() {
            Ok(()) => {
                println!("ready");
                return;
            }
            Err(error) => {
                eprintln!("locator production check failed: {error}");
                std::process::exit(1);
            }
        }
    }
    let production = match ProductionLocatorConfig::from_env() {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "invalid locator service configuration");
            std::process::exit(2);
        }
    };
    let server = production.server().clone();
    let publisher = match build_production_publisher(&production) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "locator service dependencies unavailable");
            std::process::exit(2);
        }
    };
    let pairing = match build_production_pairing(&production) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "pairing service dependencies unavailable");
            std::process::exit(2);
        }
    };
    if let Err(error) = run(server, publisher, pairing).await {
        tracing::error!(%error, "locator service stopped with an error");
        std::process::exit(1);
    }
}

async fn run(
    config: LocatorServerConfig,
    publisher: std::sync::Arc<dyn op_collab_relay_locator_server::LocatorPublisher>,
    pairing: std::sync::Arc<dyn op_collab_relay_locator_server::PairingEndpoints>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    serve_listener_until(listener, config, publisher, pairing, shutdown_signal()).await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Production,
    CheckProduction,
}

fn parse_arguments<I, S>(arguments: I) -> Result<Command, ()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut arguments = arguments.into_iter();
    let first = arguments.next().ok_or(())?;
    let command = match first.as_ref() {
        "--production" => Command::Production,
        "--check-production" => Command::CheckProduction,
        _ => return Err(()),
    };
    if arguments.next().is_some() {
        return Err(());
    }
    Ok(command)
}

fn locator_log_filter() -> Result<EnvFilter, ()> {
    let configured = std::env::var_os(LOG_LEVEL_ENV);
    let level = locator_log_level(configured.as_deref())?;
    Ok(EnvFilter::new(format!(
        "op_collab_relay_locator_server={level}"
    )))
}

fn locator_log_level(configured: Option<&std::ffi::OsStr>) -> Result<&'static str, ()> {
    match configured.and_then(std::ffi::OsStr::to_str) {
        None if configured.is_none() => Ok("info"),
        Some("error") => Ok("error"),
        Some("warn") => Ok("warn"),
        Some("info") => Ok("info"),
        Some("debug") => Ok("debug"),
        _ => Err(()),
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(_) => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{locator_log_level, parse_arguments, Command};

    #[test]
    fn production_check_is_a_standalone_mode() {
        assert_eq!(
            parse_arguments(["--check-production"]),
            Ok(Command::CheckProduction)
        );
        assert_eq!(parse_arguments(["--production"]), Ok(Command::Production));
        assert_eq!(
            parse_arguments(["--check-production", "--production"]),
            Err(())
        );
        assert_eq!(parse_arguments(Vec::<&str>::new()), Err(()));
    }

    #[test]
    fn log_filter_level_is_bounded_and_cannot_enable_dependency_traces() {
        assert_eq!(locator_log_level(None), Ok("info"));
        for level in ["error", "warn", "info", "debug"] {
            assert_eq!(locator_log_level(Some(OsStr::new(level))), Ok(level));
        }
        for rejected in ["trace", "info,hyper=trace", "", "INFO"] {
            assert_eq!(locator_log_level(Some(OsStr::new(rejected))), Err(()));
        }
    }
}
