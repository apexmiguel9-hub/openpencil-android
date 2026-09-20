use std::path::Path;

use base64::Engine as _;
use serde_json::Value;

use crate::cli_error::CliError;
use crate::command_helpers::flag_value;
use crate::mcp_http_cli::{post, tool_call_body};
use crate::{Command, Flags};

pub(crate) fn map_export(flags: &Flags) -> Result<Command, CliError> {
    let item_id = flag_value(flags, "item");
    let selection = flags.contains_key("selection");
    if item_id.is_some() && selection {
        return Err(CliError::usage(
            "--item and --selection cannot be used together",
        ));
    }
    let format_flag = flag_value(flags, "format");
    let formats_flag = flag_value(flags, "formats");
    if let (Some(format), Some(formats)) = (&format_flag, &formats_flag) {
        if format != formats {
            return Err(CliError::usage(
                "--format and --formats cannot specify different values",
            ));
        }
    }
    let format = format_flag.or(formats_flag).unwrap_or_else(|| "png".into());
    if !matches!(format.as_str(), "png" | "jpeg" | "jpg" | "webp" | "pdf") {
        return Err(CliError::Usage(format!(
            "unsupported export format {format:?}"
        )));
    }
    let output =
        flag_value(flags, "output").ok_or_else(|| CliError::usage("--output is required"))?;
    let scale = flag_value(flags, "scale");
    if let Some(value) = &scale {
        value
            .parse::<f32>()
            .map_err(|_| CliError::Usage(format!("--scale must be a number, got {value:?}")))?;
    }
    Ok(Command::Export {
        item_id,
        selection,
        output,
        format,
        scale,
    })
}

pub(crate) fn run_export(
    port: u16,
    token: &str,
    item_id: Option<&str>,
    output: &str,
    format: &str,
    scale: Option<&str>,
) -> Result<String, CliError> {
    let mut arguments = serde_json::Map::new();
    if let Some(item_id) = item_id {
        arguments.insert("itemId".into(), Value::String(item_id.into()));
    }
    arguments.insert("format".into(), Value::String(format.into()));
    if let Some(scale) = scale {
        let scale = scale
            .parse::<f64>()
            .map_err(|_| CliError::Usage(format!("--scale must be a number, got {scale:?}")))?;
        arguments.insert("scale".into(), Value::from(scale));
    }
    let response = post(
        port,
        token,
        &tool_call_body("export_item", &Value::Object(arguments).to_string()),
    )?;
    write_export_response(&response, Path::new(output))
}

pub(crate) fn map_export_deck(flags: &Flags) -> Result<Command, CliError> {
    let format = flag_value(flags, "format").unwrap_or_else(|| "pptx".into());
    if !matches!(format.as_str(), "pptx" | "html" | "pdf") {
        return Err(CliError::Usage(format!(
            "unsupported deck format {format:?}: must be one of pptx, html, pdf"
        )));
    }
    let output =
        flag_value(flags, "output").ok_or_else(|| CliError::usage("--output is required"))?;
    Ok(Command::ExportDeck { output, format })
}

/// Export the active page's boards as a deck.
///
/// Unlike `run_export`, the bytes never travel: the daemon writes the deck
/// itself. That makes the path the daemon's to resolve, and its working
/// directory is not the caller's — so a relative `--output` is made absolute
/// here, against the shell the user actually typed in.
pub(crate) fn run_export_deck(
    port: u16,
    token: &str,
    output: &str,
    format: &str,
) -> Result<String, CliError> {
    let output_path = Path::new(output);
    let absolute = if output_path.is_absolute() {
        output_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| CliError::Usage(format!("cannot resolve --output: {error}")))?
            .join(output_path)
    };
    let mut arguments = serde_json::Map::new();
    arguments.insert("format".into(), Value::String(format.into()));
    arguments.insert(
        "outputPath".into(),
        Value::String(absolute.display().to_string()),
    );
    post(
        port,
        token,
        &tool_call_body("export_deck", &Value::Object(arguments).to_string()),
    )
}

pub(crate) fn write_export_response(response: &str, output: &Path) -> Result<String, CliError> {
    let value: Value = serde_json::from_str(response).map_err(|error| {
        CliError::Payload(format!("export_item returned invalid JSON: {error}"))
    })?;
    let encoded = value
        .get("bytes_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| CliError::Payload("export_item response is missing bytes_base64".into()))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| {
            CliError::Payload(format!("export_item returned invalid Base64: {error}"))
        })?;
    std::fs::write(output, bytes).map_err(|error| {
        CliError::Io(format!(
            "cannot write export to {}: {error}",
            output.display()
        ))
    })?;

    Ok(serde_json::json!({
        "output": output.to_string_lossy(),
        "itemId": value.get("itemId").and_then(Value::as_str).unwrap_or(""),
        "itemType": value.get("itemType").and_then(Value::as_str).unwrap_or(""),
        "format": value.get("format").and_then(Value::as_str).unwrap_or(""),
    })
    .to_string())
}

pub(crate) fn map_export_frames(flags: &Flags) -> Result<Command, CliError> {
    let format = flag_value(flags, "format").unwrap_or_else(|| "png".into());
    if !matches!(format.as_str(), "png" | "jpeg" | "jpg" | "webp") {
        return Err(CliError::Usage(format!(
            "unsupported frame format {format:?}: must be one of png, jpeg, webp"
        )));
    }
    let output_dir = flag_value(flags, "output-dir")
        .or_else(|| flag_value(flags, "output"))
        .ok_or_else(|| CliError::usage("--output-dir is required"))?;
    Ok(Command::ExportFrames { output_dir, format })
}

/// Batch-export every top-level frame. Like `run_export_deck`, the daemon
/// does the writing, so a relative directory is resolved against the caller's
/// shell rather than the daemon's working directory.
pub(crate) fn run_export_frames(
    port: u16,
    token: &str,
    output_dir: &str,
    format: &str,
) -> Result<String, CliError> {
    let directory = Path::new(output_dir);
    let absolute = if directory.is_absolute() {
        directory.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| CliError::Usage(format!("cannot resolve --output-dir: {error}")))?
            .join(directory)
    };
    let mut arguments = serde_json::Map::new();
    arguments.insert("format".into(), Value::String(format.into()));
    arguments.insert(
        "outputDir".into(),
        Value::String(absolute.display().to_string()),
    );
    post(
        port,
        token,
        &tool_call_body("export_frames", &Value::Object(arguments).to_string()),
    )
}
