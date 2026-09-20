use std::path::{Component, Path};

use jian_ops_schema::node::PenNode;
use serde_json::json;

use super::{flag_value, pair, push_file_path, required_pos, tool_call, Command, Flags};
use crate::cli_error::CliError;

/// The virtual origin `op-html` rebases project-local resources onto.
const LOCAL_RESOURCE_ORIGIN: &str = op_html::VIRTUAL_PROJECT_ORIGIN;

pub(super) fn map_import_svg(positionals: &[String], flags: &Flags) -> Result<Command, CliError> {
    let path = required_pos(
        positionals,
        1,
        "Usage: op import:svg <file.svg> [--x N] [--y N] [--parent P] [--page PAGE]",
    )?;
    let mut pairs = vec![pair("svgPath", path)];
    for (flag, key) in [
        ("x", "x"),
        ("y", "y"),
        ("parent", "parent"),
        ("page", "pageId"),
    ] {
        if let Some(value) = flag_value(flags, flag) {
            pairs.push(pair(key, value));
        }
    }
    push_file_path(&mut pairs, flags);
    tool_call("import_svg", pairs)
}

pub(super) fn map_import_html(positionals: &[String], flags: &Flags) -> Result<Command, CliError> {
    let source = required_pos(
        positionals,
        1,
        "Usage: op import:html <file.html|url> [--x N] [--y N] [--parent P] [--page PAGE] [--viewport-height N] [--out out.op]",
    )?;
    let is_url = source.starts_with("http://") || source.starts_with("https://");
    let viewport_height = match flag_value(flags, "viewport-height") {
        Some(raw) => Some(parse_viewport_height(&raw)?.to_string()),
        None => None,
    };
    if let Some(out_path) = flag_value(flags, "out") {
        if is_url {
            return Err(CliError::usage(
                "--out requires a local file; URL import needs a running editor",
            ));
        }
        return Ok(Command::ImportHtml {
            html_path: source,
            out_path,
            viewport_height,
        });
    }

    let mut pairs = if is_url {
        vec![pair("url", source)]
    } else {
        vec![pair("htmlPath", source)]
    };
    for (flag, key) in [
        ("x", "x"),
        ("y", "y"),
        ("parent", "parent"),
        ("page", "pageId"),
    ] {
        if let Some(value) = flag_value(flags, flag) {
            pairs.push(pair(key, value));
        }
    }
    if let Some(height) = viewport_height {
        pairs.push(pair("viewportHeight", height));
    }
    push_file_path(&mut pairs, flags);
    tool_call(
        if is_url {
            "import_html_url"
        } else {
            "import_html"
        },
        pairs,
    )
}

/// `--viewport-height` accepts a positive finite number of CSS pixels.
fn parse_viewport_height(raw: &str) -> Result<f64, CliError> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| CliError::usage("--viewport-height expects a positive number of CSS pixels"))
}

pub(super) fn map_import_snapshot(
    positionals: &[String],
    flags: &Flags,
) -> Result<Command, CliError> {
    let json_path = required_pos(
        positionals,
        1,
        "Usage: op import:snapshot <snapshot.json> [--x N] [--y N] [--parent P] [--page PAGE] [--out out.op]",
    )?;
    if let Some(out_path) = flag_value(flags, "out") {
        return Ok(Command::ImportSnapshot {
            json_path,
            out_path,
        });
    }

    let mut pairs = vec![pair("snapshotPath", json_path)];
    for (flag, key) in [
        ("x", "x"),
        ("y", "y"),
        ("parent", "parent"),
        ("page", "pageId"),
    ] {
        if let Some(value) = flag_value(flags, flag) {
            pairs.push(pair(key, value));
        }
    }
    push_file_path(&mut pairs, flags);
    tool_call("import_web_snapshot", pairs)
}

pub(super) fn run_import_html(
    html_path: &str,
    out_path: &str,
    viewport_height: Option<&str>,
) -> Result<String, CliError> {
    let viewport_height = viewport_height.map(parse_viewport_height).transpose()?;
    let source_bytes = std::fs::read(html_path)
        .map_err(|error| CliError::Io(format!("read {html_path:?}: {error}")))?;
    let source = op_html::html_encoding::decode_html_bytes(&source_bytes);
    let source_path = Path::new(html_path);
    let resource_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    let document_name = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("HTML Import")
        .to_string();
    let fetcher = |url: &str| {
        let href = url.strip_prefix(LOCAL_RESOURCE_ORIGIN).unwrap_or(url);
        local_resource_fetch(resource_dir, href)
    };
    let options = op_html::HtmlImportOptions {
        document_name: Some(document_name),
        base_url: Some(format!("{LOCAL_RESOURCE_ORIGIN}document.html")),
        viewport_height,
        ..op_html::HtmlImportOptions::default()
    };
    let imported = op_html::import_html_document(source.as_ref(), &options, Some(&fetcher), None);
    if imported.document.children.is_empty() {
        let detail = imported
            .warnings
            .first()
            .map(String::as_str)
            .unwrap_or("input produced no nodes");
        return Err(CliError::Document(format!(
            "no importable content: {detail}"
        )));
    }
    let value = serde_json::to_value(&imported.document)
        .map_err(|error| CliError::Payload(format!("serialize {out_path:?}: {error}")))?;
    std::fs::write(out_path, value.to_string())
        .map_err(|error| CliError::Io(format!("write {out_path:?}: {error}")))?;
    Ok(json!({
        "ok": true,
        "filePath": out_path,
        "nodeCount": count_nodes(&imported.document.children),
        "warnings": imported.warnings,
    })
    .to_string())
}

pub(super) fn run_import_snapshot(json_path: &str, out_path: &str) -> Result<String, CliError> {
    let source = std::fs::read_to_string(json_path)
        .map_err(|error| CliError::Io(format!("read {json_path:?}: {error}")))?;
    let imported =
        op_html::import_snapshot_document(&source, &op_html::HtmlImportOptions::default());
    if imported.document.children.is_empty() {
        let detail = imported
            .warnings
            .first()
            .map(String::as_str)
            .unwrap_or("input produced no nodes");
        return Err(CliError::Document(format!(
            "no importable content: {detail}"
        )));
    }
    let value = serde_json::to_value(&imported.document)
        .map_err(|error| CliError::Payload(format!("serialize {out_path:?}: {error}")))?;
    std::fs::write(out_path, value.to_string())
        .map_err(|error| CliError::Io(format!("write {out_path:?}: {error}")))?;
    Ok(json!({
        "ok": true,
        "filePath": out_path,
        "nodeCount": count_nodes(&imported.document.children),
        "warnings": imported.warnings,
    })
    .to_string())
}

fn local_resource_fetch(dir: &Path, href: &str) -> Option<Vec<u8>> {
    let href = href.split(['?', '#']).next()?.trim_start_matches('/');
    let relative = Path::new(href);
    if href.is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let root = dir.canonicalize().ok()?;
    let path = dir.join(relative).canonicalize().ok()?;
    path.starts_with(&root)
        .then(|| std::fs::read(path).ok())
        .flatten()
}

fn count_nodes(nodes: &[PenNode]) -> usize {
    nodes
        .iter()
        .map(|node| {
            1 + match node {
                PenNode::Frame(node) => node
                    .children
                    .as_deref()
                    .map(count_nodes)
                    .unwrap_or_default(),
                PenNode::Group(node) => node
                    .children
                    .as_deref()
                    .map(count_nodes)
                    .unwrap_or_default(),
                PenNode::Rectangle(node) => node
                    .children
                    .as_deref()
                    .map(count_nodes)
                    .unwrap_or_default(),
                _ => 0,
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn temp_html_path(tag: &str, extension: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!(
            "op-cli-html-{tag}-{}-{nanos}.{extension}",
            std::process::id()
        ))
    }

    #[test]
    fn parse_args_maps_import_figma_to_direct_converter() {
        let parsed = parse_args(&[
            "import:figma".to_string(),
            "/tmp/source.fig".to_string(),
            "--out".to_string(),
            "/tmp/converted.op".to_string(),
        ])
        .expect("parse import:figma");
        assert_eq!(
            parsed.command,
            Command::ImportFigma {
                fig_path: "/tmp/source.fig".to_string(),
                out_path: "/tmp/converted.op".to_string(),
            }
        );
    }

    #[test]
    fn figma_default_out_path_matches_ts_suffix_replacement() {
        assert_eq!(figma_default_out_path("checkout.fig"), "checkout.op");
        assert_eq!(
            figma_default_out_path("/tmp/checkout.fig"),
            "/tmp/checkout.op"
        );
        assert_eq!(
            figma_default_out_path("/tmp/checkout.FIG"),
            "/tmp/checkout.FIG"
        );
    }

    #[test]
    fn import_html_maps_to_tool_call() {
        let args = vec![
            "import:html".to_string(),
            "page.html".to_string(),
            "--x".to_string(),
            "10".to_string(),
            "--page".to_string(),
            "p1".to_string(),
        ];
        let parsed = parse_args(&args).expect("parse");
        assert_eq!(
            parsed.command,
            Command::ToolCall {
                tool: "import_html".to_string(),
                args: vec![
                    ("htmlPath".to_string(), "page.html".to_string()),
                    ("x".to_string(), "10".to_string()),
                    ("pageId".to_string(), "p1".to_string()),
                ],
            }
        );
    }

    #[test]
    fn import_html_url_routes_to_url_tool() {
        let args = vec![
            "import:html".to_string(),
            "https://example.com/p".to_string(),
        ];
        let parsed = parse_args(&args).expect("parse");
        assert_eq!(
            parsed.command,
            Command::ToolCall {
                tool: "import_html_url".to_string(),
                args: vec![("url".to_string(), "https://example.com/p".to_string())],
            }
        );
    }

    #[test]
    fn import_html_with_out_is_local_command() {
        let args = vec![
            "import:html".to_string(),
            "a.html".to_string(),
            "--out".to_string(),
            "a.op".to_string(),
        ];
        let parsed = parse_args(&args).expect("parse");
        assert_eq!(
            parsed.command,
            Command::ImportHtml {
                html_path: "a.html".to_string(),
                out_path: "a.op".to_string(),
                viewport_height: None,
            }
        );
    }

    #[test]
    fn import_html_url_with_out_errors() {
        let args = vec![
            "import:html".to_string(),
            "https://e.com/".to_string(),
            "--out".to_string(),
            "a.op".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn local_import_decodes_utf16le_bom() {
        let input = temp_html_path("utf16", "html");
        let output = temp_html_path("utf16", "op");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "<html><body><p>你好</p></body></html>".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&input, bytes).expect("write UTF-16 fixture");

        let result = super::run_import_html(
            input.to_str().expect("UTF-8 input path"),
            output.to_str().expect("UTF-8 output path"),
            None,
        )
        .expect("import UTF-16 HTML");
        let document = std::fs::read_to_string(&output).expect("read imported document");

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
        assert!(result.contains("\"ok\":true"));
        assert!(
            document.contains("你好"),
            "BOM-decoded text must survive the CLI conversion: {document}"
        );
    }

    #[test]
    fn import_snapshot_maps_to_tool_call() {
        let args = vec![
            "import:snapshot".to_string(),
            "s.json".to_string(),
            "--page".to_string(),
            "p1".to_string(),
        ];
        let parsed = parse_args(&args).expect("parse");
        assert_eq!(
            parsed.command,
            Command::ToolCall {
                tool: "import_web_snapshot".to_string(),
                args: vec![
                    ("snapshotPath".to_string(), "s.json".to_string()),
                    ("pageId".to_string(), "p1".to_string()),
                ],
            }
        );
    }

    #[test]
    fn import_snapshot_with_out_is_local() {
        let args = vec![
            "import:snapshot".to_string(),
            "s.json".to_string(),
            "--out".to_string(),
            "s.op".to_string(),
        ];
        let parsed = parse_args(&args).expect("parse");
        assert_eq!(
            parsed.command,
            Command::ImportSnapshot {
                json_path: "s.json".to_string(),
                out_path: "s.op".to_string(),
            }
        );
    }
}
