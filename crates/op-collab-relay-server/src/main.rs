use std::ffi::OsStr;

use op_collab_relay_server::{
    check_production, run, run_production, ProductionRelayAuthConfig, RelayConfig,
};
use tracing_subscriber::EnvFilter;

const LOG_LEVEL_ENV: &str = "OPENPENCIL_COLLAB_RELAY_LOG_LEVEL";

#[tokio::main]
async fn main() {
    let log_filter = match relay_log_filter_from_env() {
        Ok(filter) => filter,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(log_filter)
        .without_time()
        .init();

    let launch_mode = match parse_args() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    if launch_mode == LaunchMode::CheckProduction {
        match check_production() {
            Ok(()) => {
                println!("ready");
                return;
            }
            Err(error) => {
                eprintln!("relay production check failed: {error}");
                std::process::exit(1);
            }
        }
    }
    let config = match RelayConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "invalid relay configuration");
            std::process::exit(2);
        }
    };
    let result: Result<(), Box<dyn std::error::Error>> = match launch_mode {
        LaunchMode::FailClosed => run(config)
            .await
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>),
        LaunchMode::UnauthenticatedDev => {
            tracing::warn!(
                "UNAUTHENTICATED DEVELOPMENT MODE ENABLED; do not expose this listener publicly"
            );
            run(config.allow_unauthenticated_dev())
                .await
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
        }
        LaunchMode::Production {
            allow_ticket_binding_only,
        } => {
            let auth_config = match ProductionRelayAuthConfig::from_env(allow_ticket_binding_only) {
                Ok(config) => config,
                Err(error) => {
                    tracing::error!(%error, "invalid production relay authentication configuration");
                    std::process::exit(2);
                }
            };
            if auth_config.reduced_assurance() {
                tracing::warn!(
                    "REDUCED-ASSURANCE TICKET-BINDING-ONLY MODE ENABLED; \
                     configure relay X25519 proof keys for full production authentication"
                );
            }
            run_production(config, auth_config)
                .await
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
        }
        LaunchMode::CheckProduction => unreachable!("handled before listener configuration"),
    };

    if let Err(error) = result {
        tracing::error!(%error, "relay stopped with an error");
        std::process::exit(1);
    }
}

fn relay_log_filter_from_env() -> Result<EnvFilter, CliError> {
    log_filter_directive(std::env::var_os(LOG_LEVEL_ENV).as_deref()).map(EnvFilter::new)
}

fn log_filter_directive(value: Option<&OsStr>) -> Result<&'static str, CliError> {
    let Some(value) = value else {
        return Ok("op_collab_relay_server=info");
    };
    match value.to_str() {
        None => Err(CliError::InvalidLogLevel),
        Some("error") => Ok("op_collab_relay_server=error"),
        Some("warn") => Ok("op_collab_relay_server=warn"),
        Some("info") => Ok("op_collab_relay_server=info"),
        Some("debug") => Ok("op_collab_relay_server=debug"),
        Some(_) => Err(CliError::InvalidLogLevel),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum LaunchMode {
    FailClosed,
    UnauthenticatedDev,
    Production { allow_ticket_binding_only: bool },
    CheckProduction,
}

fn parse_args() -> Result<LaunchMode, CliError> {
    parse_arg_values(std::env::args().skip(1))
}

fn parse_arg_values<I, S>(args: I) -> Result<LaunchMode, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut allow_unauthenticated_dev = false;
    let mut production = false;
    let mut allow_ticket_binding_only = false;
    let mut check_production = false;
    for arg in args {
        match arg.as_ref() {
            "--allow-unauthenticated-dev" if !allow_unauthenticated_dev => {
                allow_unauthenticated_dev = true;
            }
            "--production" if !production => {
                production = true;
            }
            "--allow-ticket-binding-only" if !allow_ticket_binding_only => {
                allow_ticket_binding_only = true;
            }
            "--check-production" if !check_production => {
                check_production = true;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: op-collab-relay-server [--production \
                     [--allow-ticket-binding-only] | --allow-unauthenticated-dev | \
                     --check-production]\n\
                     \n\
                     --production loads pinned ticket-policy, locator, region, and relay X25519\n\
                     verifier configuration from OPENPENCIL_COLLAB_RELAY_* environment variables.\n\
                     --allow-ticket-binding-only explicitly selects reduced assurance when no\n\
                     challenge-proof key is configured.\n\
                     --check-production verifies the mounted production trust inputs and exits.\n\
                     The development flag enables capability-only routing without a ticket.\n\
                     With no mode flag the server fails closed."
                );
                std::process::exit(0);
            }
            _ => return Err(CliError::UnknownOrDuplicateArgument),
        }
    }
    if allow_unauthenticated_dev && production {
        return Err(CliError::ConflictingModes);
    }
    if check_production && (allow_unauthenticated_dev || production || allow_ticket_binding_only) {
        return Err(CliError::ConflictingCheckMode);
    }
    if allow_ticket_binding_only && !production {
        return Err(CliError::BindingOnlyRequiresProduction);
    }
    Ok(if check_production {
        LaunchMode::CheckProduction
    } else if allow_unauthenticated_dev {
        LaunchMode::UnauthenticatedDev
    } else if production {
        LaunchMode::Production {
            allow_ticket_binding_only,
        }
    } else {
        LaunchMode::FailClosed
    })
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
enum CliError {
    #[error("unknown or duplicate relay command-line argument")]
    UnknownOrDuplicateArgument,
    #[error("--production and --allow-unauthenticated-dev are mutually exclusive")]
    ConflictingModes,
    #[error("--allow-ticket-binding-only requires --production")]
    BindingOnlyRequiresProduction,
    #[error("--check-production cannot be combined with a server launch mode")]
    ConflictingCheckMode,
    #[error("OPENPENCIL_COLLAB_RELAY_LOG_LEVEL must be one of error, warn, info, or debug")]
    InvalidLogLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_and_reduced_assurance_require_explicit_flags() {
        assert_eq!(
            parse_arg_values(Vec::<&str>::new()),
            Ok(LaunchMode::FailClosed)
        );
        assert_eq!(
            parse_arg_values(["--production"]),
            Ok(LaunchMode::Production {
                allow_ticket_binding_only: false,
            })
        );
        assert_eq!(
            parse_arg_values(["--production", "--allow-ticket-binding-only"]),
            Ok(LaunchMode::Production {
                allow_ticket_binding_only: true,
            })
        );
        assert_eq!(
            parse_arg_values(["--allow-ticket-binding-only"]),
            Err(CliError::BindingOnlyRequiresProduction)
        );
    }

    #[test]
    fn production_and_unauthenticated_development_are_exclusive() {
        assert_eq!(
            parse_arg_values(["--production", "--allow-unauthenticated-dev"]),
            Err(CliError::ConflictingModes)
        );
        assert_eq!(
            parse_arg_values(["--production", "--production"]),
            Err(CliError::UnknownOrDuplicateArgument)
        );
    }

    #[test]
    fn production_check_is_a_standalone_mode() {
        assert_eq!(
            parse_arg_values(["--check-production"]),
            Ok(LaunchMode::CheckProduction)
        );
        assert_eq!(
            parse_arg_values(["--check-production", "--production"]),
            Err(CliError::ConflictingCheckMode)
        );
        assert_eq!(
            parse_arg_values(["--check-production", "--check-production"]),
            Err(CliError::UnknownOrDuplicateArgument)
        );
    }

    #[test]
    fn relay_log_filter_is_crate_scoped_and_bounded() {
        assert_eq!(
            log_filter_directive(None),
            Ok("op_collab_relay_server=info")
        );
        assert_eq!(
            log_filter_directive(Some(OsStr::new("debug"))),
            Ok("op_collab_relay_server=debug")
        );
        assert_eq!(
            log_filter_directive(Some(OsStr::new("trace"))),
            Err(CliError::InvalidLogLevel)
        );
        assert_eq!(
            log_filter_directive(Some(OsStr::new("tungstenite=trace"))),
            Err(CliError::InvalidLogLevel)
        );
    }
}
