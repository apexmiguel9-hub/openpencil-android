//! Lossless TOML editing and crash-safe config writes for MCP integrations.

use std::fs::{self, File, Permissions};
use std::io::{self, Write};
use std::path::Path;

use toml_edit::{value, DocumentMut, InlineTable, Item, Table, Value};

use crate::mcp_config_error::McpConfigError;

const SERVER_NAME: &str = "openpencil";

#[derive(Debug)]
pub(crate) struct FileSnapshot {
    bytes: Option<Vec<u8>>,
    permissions: Option<Permissions>,
}

impl FileSnapshot {
    pub(crate) fn capture(path: &Path) -> Result<Self, McpConfigError> {
        match fs::read(path) {
            Ok(bytes) => {
                let permissions = fs::metadata(path)
                    .map(|metadata| metadata.permissions())
                    .map_err(|error| McpConfigError::Metadata {
                        path: path.to_path_buf(),
                        message: error.to_string(),
                    })?;
                Ok(Self {
                    bytes: Some(bytes),
                    permissions: Some(permissions),
                })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self {
                bytes: None,
                permissions: None,
            }),
            Err(error) => Err(McpConfigError::Read {
                path: path.to_path_buf(),
                message: error.to_string(),
            }),
        }
    }

    pub(crate) fn restore(&self, path: &Path) -> Result<(), McpConfigError> {
        match &self.bytes {
            Some(bytes) => atomic_write_with_permissions(path, bytes, self.permissions.clone()),
            None => match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(McpConfigError::Remove {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                }),
            },
        }
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), McpConfigError> {
    let permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    atomic_write_with_permissions(path, bytes, permissions)
}

/// Crash-safe write: sibling temp + fsync + atomic replace. The temp
/// creation and the (Windows FFI) destination swap delegate to
/// `op_host_services::doc_io::atomic_file` — the desktop crate no longer
/// keeps its own ReplaceFileW/MoveFileExW copy.
fn atomic_write_with_permissions(
    path: &Path,
    bytes: &[u8],
    permissions: Option<Permissions>,
) -> Result<(), McpConfigError> {
    use op_host_services::doc_io::atomic_file;

    let parent = path
        .parent()
        .ok_or_else(|| McpConfigError::NoParentDirectory {
            path: path.to_path_buf(),
        })?;
    fs::create_dir_all(parent).map_err(|error| McpConfigError::CreateDir {
        path: parent.to_path_buf(),
        message: error.to_string(),
    })?;

    // `atomic_file` belongs to `op-host-services`, a crate this pass does not
    // own; carry its message so the text is unchanged either way.
    let (temp_path, mut temp) = atomic_file::create_sibling_temp(path)
        .map_err(|error| McpConfigError::AtomicFile(error.to_string()))?;
    let result = (|| {
        temp.write_all(bytes)
            .map_err(|error| McpConfigError::Write {
                path: temp_path.clone(),
                message: error.to_string(),
            })?;
        temp.sync_all().map_err(|error| McpConfigError::Sync {
            path: temp_path.clone(),
            message: error.to_string(),
        })?;
        drop(temp);

        if let Some(permissions) = permissions {
            fs::set_permissions(&temp_path, permissions).map_err(|error| {
                McpConfigError::Permissions {
                    path: temp_path.clone(),
                    message: error.to_string(),
                }
            })?;
        }
        atomic_file::replace_file(&temp_path, path)
            .map_err(|error| McpConfigError::AtomicFile(error.to_string()))?;
        sync_parent(parent);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn sync_parent(parent: &Path) {
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
}

pub(crate) fn update_grok_config(
    path: &Path,
    enabled: bool,
    server_url: &str,
) -> Result<(), McpConfigError> {
    if !enabled && !path.exists() {
        return Ok(());
    }
    let existing = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(McpConfigError::Read {
                path: path.to_path_buf(),
                message: error.to_string(),
            })
        }
    };
    let mut document = existing
        .parse::<DocumentMut>()
        .map_err(|error| McpConfigError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    edit_grok_server(&mut document, enabled, server_url)?;
    atomic_write(path, document.to_string().as_bytes())
}

pub(crate) fn grok_config_has_openpencil(input: &str) -> bool {
    let Ok(document) = input.parse::<DocumentMut>() else {
        return false;
    };
    let Some(servers) = document
        .as_table()
        .get("mcp_servers")
        .and_then(Item::as_table_like)
    else {
        return false;
    };
    let Some(server) = servers.get(SERVER_NAME).and_then(Item::as_table_like) else {
        return false;
    };
    if server.get("enabled").and_then(Item::as_bool) == Some(false) {
        return false;
    }
    server
        .get("url")
        .and_then(Item::as_str)
        .map(is_usable_http_url)
        .unwrap_or(false)
}

fn edit_grok_server(
    document: &mut DocumentMut,
    enabled: bool,
    server_url: &str,
) -> Result<(), McpConfigError> {
    let root = document.as_table_mut();
    if !root.contains_key("mcp_servers") {
        if !enabled {
            return Ok(());
        }
        root.insert("mcp_servers", Item::Table(Table::new()));
    }
    let servers_item = root
        .get_mut("mcp_servers")
        .ok_or(McpConfigError::GrokMcpServersMissing)?;
    let inline = servers_item.is_inline_table();
    let servers = servers_item
        .as_table_like_mut()
        .ok_or(McpConfigError::GrokMcpServersNotATable)?;

    if enabled {
        let server = if inline {
            let mut table = InlineTable::new();
            table.insert("url", Value::from(server_url));
            Item::Value(Value::InlineTable(table))
        } else {
            let mut table = Table::new();
            table.insert("url", value(server_url));
            Item::Table(table)
        };
        servers.insert(SERVER_NAME, server);
    } else {
        servers.remove(SERVER_NAME);
    }
    Ok(())
}

fn is_usable_http_url(input: &str) -> bool {
    reqwest::Url::parse(input)
        .map(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Distinguishes the per-test scratch directories.
    static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "openpencil-mcp-io-{name}-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp directory");
        path.join("config.toml")
    }

    #[test]
    fn grok_inline_table_round_trip_preserves_comments_and_other_servers() {
        let path = temp_path("inline");
        let input = "# keep this comment\nmcp_servers = { openpencil = { url = \"http://old/mcp\", headers = { Authorization = \"stale\" } }, other = { url = \"https://example.com/mcp\" } } # keep tail\n";
        fs::write(&path, input).expect("seed config");

        update_grok_config(&path, true, "http://127.0.0.1:3400/mcp").expect("enable");
        let enabled = fs::read_to_string(&path).expect("read enabled config");
        assert!(grok_config_has_openpencil(&enabled), "{enabled}");
        assert!(enabled.contains("# keep this comment"), "{enabled}");
        assert!(enabled.contains("# keep tail"), "{enabled}");
        assert!(enabled.contains("other"), "{enabled}");
        assert!(!enabled.contains("stale"), "{enabled}");

        update_grok_config(&path, false, "http://unused").expect("disable");
        let disabled = fs::read_to_string(&path).expect("read disabled config");
        assert!(!grok_config_has_openpencil(&disabled), "{disabled}");
        assert!(disabled.contains("other"), "{disabled}");
        assert!(disabled.contains("# keep this comment"), "{disabled}");
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn atomic_write_replaces_an_existing_file_without_temp_debris() {
        let path = temp_path("atomic-replace");
        fs::write(&path, b"old").expect("seed config");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, Permissions::from_mode(0o600)).expect("set permissions");
        }
        atomic_write(&path, b"new").expect("replace config");
        assert_eq!(fs::read(&path).expect("read config"), b"new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
                0o600
            );
        }
        let parent = path.parent().expect("parent");
        let names: Vec<_> = fs::read_dir(parent)
            .expect("read parent")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, vec![path.file_name().expect("file name")]);
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn grok_dotted_and_normal_tables_stay_valid() {
        for (name, input) in [
            (
                "dotted",
                "mcp_servers.openpencil.url = \"http://old/mcp\"\nmcp_servers.other.url = \"https://example.com/mcp\"\n",
            ),
            (
                "normal",
                "[mcp_servers.openpencil]\nurl = \"http://old/mcp\"\n\n[mcp_servers.other]\nurl = \"https://example.com/mcp\"\n",
            ),
        ] {
            let path = temp_path(name);
            fs::write(&path, input).expect("seed config");
            update_grok_config(&path, true, "http://127.0.0.1:3401/mcp").expect("enable");
            let text = fs::read_to_string(&path).expect("read config");
            text.parse::<DocumentMut>().expect("valid TOML");
            assert!(grok_config_has_openpencil(&text), "{text}");
            assert!(text.contains("other"), "{text}");
            let _ = fs::remove_dir_all(path.parent().expect("parent"));
        }
    }

    #[test]
    fn grok_detection_rejects_missing_or_invalid_urls() {
        for input in [
            "[mcp_servers.openpencil]\nurl = \"not-a-url\"\n",
            "mcp_servers = { openpencil = { url = \"file:///tmp/mcp\" } }\n",
            "[mcp_servers.openpencil]\ncommand = \"mcp-server\"\n",
            "[mcp_servers.openpencil]\nurl = \"https://example.com/mcp\"\nenabled = false\n",
        ] {
            assert!(!grok_config_has_openpencil(input), "{input}");
        }
        assert!(grok_config_has_openpencil(
            "mcp_servers = { openpencil = { url = \"https://example.com/mcp\" } }\n"
        ));
    }
}
