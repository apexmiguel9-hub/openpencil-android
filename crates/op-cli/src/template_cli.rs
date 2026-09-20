//! `op templates` and `op use-template` — the shipped scene-template
//! catalogue, including the 16:9 presentation deck templates.
//!
//! Both are thin passes over the matching MCP tools; the catalogue itself is
//! resolved editor-side so the CLI never carries a second copy of it.

use serde_json::Value;

use crate::cli_error::CliError;
use crate::command_helpers::flag_value;
use crate::mcp_http_cli::{post, tool_call_body};
use crate::{Command, Flags};

pub(crate) fn map_templates(flags: &Flags) -> Result<Command, CliError> {
    Ok(Command::Templates {
        scene: flag_value(flags, "scene"),
        tag: flag_value(flags, "tag"),
    })
}

pub(crate) fn map_use_template(flags: &Flags, positionals: &[String]) -> Result<Command, CliError> {
    // Accept both `op use-template <id>` and `--template <id>`: the bare
    // positional is what a person types, the flag is what a script generates.
    // `positionals[0]` is the command word itself, so the id is at index 1.
    let template_id = positionals
        .get(1)
        .cloned()
        .or_else(|| flag_value(flags, "template"))
        .ok_or_else(|| CliError::usage("a template id is required (op templates lists them)"))?;
    Ok(Command::UseTemplate { template_id })
}

pub(crate) fn run_templates(
    port: u16,
    token: &str,
    scene: Option<&str>,
    tag: Option<&str>,
) -> Result<String, CliError> {
    let mut arguments = serde_json::Map::new();
    if let Some(scene) = scene {
        arguments.insert("scene".into(), Value::String(scene.into()));
    }
    if let Some(tag) = tag {
        arguments.insert("tag".into(), Value::String(tag.into()));
    }
    post(
        port,
        token,
        &tool_call_body(
            "list_scene_templates",
            &Value::Object(arguments).to_string(),
        ),
    )
}

pub(crate) fn run_use_template(
    port: u16,
    token: &str,
    template_id: &str,
) -> Result<String, CliError> {
    let mut arguments = serde_json::Map::new();
    arguments.insert("templateId".into(), Value::String(template_id.into()));
    post(
        port,
        token,
        &tool_call_body("use_scene_template", &Value::Object(arguments).to_string()),
    )
}

pub(crate) fn map_styles(flags: &Flags, positionals: &[String]) -> Result<Command, CliError> {
    Ok(Command::Styles {
        // `positionals[0]` is the command word; a bare id after it selects one
        // guide and pulls its markdown down with it.
        id: positionals
            .get(1)
            .cloned()
            .or_else(|| flag_value(flags, "id")),
        tag: flag_value(flags, "tag"),
        platform: flag_value(flags, "platform"),
    })
}

pub(crate) fn run_styles(
    port: u16,
    token: &str,
    id: Option<&str>,
    tag: Option<&str>,
    platform: Option<&str>,
) -> Result<String, CliError> {
    let mut arguments = serde_json::Map::new();
    for (key, value) in [("id", id), ("tag", tag), ("platform", platform)] {
        if let Some(value) = value {
            arguments.insert(key.into(), Value::String(value.into()));
        }
    }
    post(
        port,
        token,
        &tool_call_body("list_style_guides", &Value::Object(arguments).to_string()),
    )
}
