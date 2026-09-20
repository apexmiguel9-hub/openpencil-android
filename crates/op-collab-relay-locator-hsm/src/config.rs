use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::{SignerError, SignerResult};

pub const CONFIG_VERSION: u8 = 1;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    Cn,
    Global,
}

impl Region {
    pub const fn wire_value(self) -> u8 {
        match self {
            Self::Cn => 1,
            Self::Global => 2,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyConfig {
    pub kid: String,
    pub object_id_hex: String,
}

impl KeyConfig {
    pub fn object_id(&self) -> SignerResult<Vec<u8>> {
        decode_object_id(&self.object_id_hex)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignerConfig {
    pub version: u8,
    pub region: Region,
    pub socket_path: PathBuf,
    pub expected_client_uid: u32,
    pub expected_client_gid: u32,
    pub pkcs11_module_path: PathBuf,
    pub token_label: String,
    pub pin_file: PathBuf,
    pub active_kid: String,
    pub keys: Vec<KeyConfig>,
    #[serde(default = "default_timeout_ms")]
    pub request_timeout_ms: u64,
}

impl SignerConfig {
    pub fn from_bytes(bytes: &[u8]) -> SignerResult<Self> {
        let config: Self = serde_json::from_slice(bytes)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> SignerResult<()> {
        if self.version != CONFIG_VERSION {
            return Err(config_error("unsupported config version"));
        }
        validate_path(&self.socket_path, "socket_path")?;
        validate_path(&self.pkcs11_module_path, "pkcs11_module_path")?;
        validate_path(&self.pin_file, "pin_file")?;
        if self.expected_client_uid == 0 || self.expected_client_gid == 0 {
            return Err(config_error("expected locator uid/gid must be non-root"));
        }
        if self.socket_path.as_os_str().as_encoded_bytes().len() > 100 {
            return Err(config_error("socket_path exceeds Unix socket limit"));
        }
        if self.token_label.is_empty()
            || self.token_label.len() > 32
            || !self.token_label.bytes().all(is_safe_ascii)
        {
            return Err(config_error("token_label must be 1-32 safe ASCII bytes"));
        }
        if !(100..=5_000).contains(&self.request_timeout_ms) {
            return Err(config_error("request_timeout_ms must be 100-5000"));
        }
        if self.keys.len() != 2 {
            return Err(config_error(
                "keys must contain exactly active and next entries",
            ));
        }
        let mut active_matches = 0;
        for (index, key) in self.keys.iter().enumerate() {
            validate_kid(&key.kid)?;
            key.object_id()?;
            if key.kid == self.active_kid {
                active_matches += 1;
            }
            for other in self.keys.iter().skip(index + 1) {
                if key.kid == other.kid || key.object_id_hex == other.object_id_hex {
                    return Err(config_error("key ids and object ids must be unique"));
                }
            }
        }
        if active_matches != 1 {
            return Err(config_error(
                "active_kid must identify exactly one configured key",
            ));
        }
        Ok(())
    }

    pub fn active_key(&self) -> &KeyConfig {
        self.keys
            .iter()
            .find(|key| key.kid == self.active_kid)
            .expect("validated active key")
    }

    pub fn key(&self, kid: &str) -> Option<&KeyConfig> {
        self.keys.iter().find(|key| key.kid == kid)
    }
}

fn default_timeout_ms() -> u64 {
    2_000
}

fn validate_path(path: &Path, field: &str) -> SignerResult<()> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || bytes.is_empty()
        || bytes.len() > MAX_PATH_BYTES
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(config_error(format!(
            "{field} must be an absolute bounded path without parent traversal"
        )));
    }
    Ok(())
}

fn validate_kid(kid: &str) -> SignerResult<()> {
    if kid.is_empty() || kid.len() > 64 || !kid.bytes().all(is_safe_ascii) {
        return Err(config_error("kid must be 1-64 safe ASCII bytes"));
    }
    Ok(())
}

fn decode_object_id(value: &str) -> SignerResult<Vec<u8>> {
    if value.len() < 2 || value.len() > 64 || !value.len().is_multiple_of(2) {
        return Err(config_error("object_id_hex must contain 1-32 bytes"));
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair).map_err(|_| config_error("invalid object id"))?;
        output.push(
            u8::from_str_radix(text, 16)
                .map_err(|_| config_error("object_id_hex must be lowercase hexadecimal"))?,
        );
    }
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(config_error("object_id_hex must be lowercase hexadecimal"));
    }
    Ok(output)
}

fn is_safe_ascii(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
}

fn config_error(message: impl Into<String>) -> SignerError {
    SignerError::Config(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> SignerConfig {
        SignerConfig {
            version: 1,
            region: Region::Global,
            socket_path: "/run/openpencil-hsm/signer.sock".into(),
            expected_client_uid: 65532,
            expected_client_gid: 65532,
            pkcs11_module_path: "/usr/lib/softhsm/libsofthsm2.so".into(),
            token_label: "openpencil-locator-global".into(),
            pin_file: "/run/secrets/locator-hsm-pin".into(),
            active_kid: "locator-global-2026-a".into(),
            keys: vec![
                KeyConfig {
                    kid: "locator-global-2026-a".into(),
                    object_id_hex: "10a1".into(),
                },
                KeyConfig {
                    kid: "locator-global-2026-b".into(),
                    object_id_hex: "10a2".into(),
                },
            ],
            request_timeout_ms: 2_000,
        }
    }

    #[test]
    fn accepts_rotation_pair() {
        assert!(valid_config().validate().is_ok());
    }

    #[test]
    fn rejects_duplicate_object_id() {
        let mut config = valid_config();
        config.keys[1].object_id_hex = config.keys[0].object_id_hex.clone();
        assert!(config.validate().is_err());
    }
}
