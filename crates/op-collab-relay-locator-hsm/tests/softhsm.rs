// The crate under test only exists on Unix; see its lib.rs for why.
#![cfg(unix)]

use std::{env, path::PathBuf};

use ed25519_dalek::{Signature, VerifyingKey};
use op_collab_relay_locator_hsm::{protocol, KeyConfig, KeyStore, Region, SignerConfig};
use tempfile::tempdir;

const USER_PIN: &[u8] = b"integration-user-pin-42";
const SO_PIN: &[u8] = b"integration-so-pin-73";

#[test]
fn provisions_non_extractable_rotation_pair_and_signs() {
    let Ok(module) = env::var("OPENPENCIL_SOFTHSM2_MODULE") else {
        eprintln!("skipped: OPENPENCIL_SOFTHSM2_MODULE is not set");
        return;
    };
    let directory = tempdir().unwrap();
    let token_directory = directory.path().join("tokens");
    std::fs::create_dir(&token_directory).unwrap();
    let softhsm_config = directory.path().join("softhsm2.conf");
    std::fs::write(
        &softhsm_config,
        format!(
            "directories.tokendir = {}\nobjectstore.backend = file\nlog.level = ERROR\nslots.removable = false\n",
            token_directory.display()
        ),
    )
    .unwrap();
    env::set_var("SOFTHSM2_CONF", &softhsm_config);

    let config = signer_config(module.into(), directory.path().join("signer.sock"));
    config.validate().unwrap();
    KeyStore::initialize_token(&config, USER_PIN, SO_PIN).unwrap();
    for key in &config.keys {
        let public = KeyStore::provision(&config, USER_PIN, key).unwrap();
        assert_eq!(public.kid, key.kid);
        assert!(!public.public_key_ed25519.is_empty());
    }
    assert!(KeyStore::provision(&config, USER_PIN, &config.keys[0]).is_err());

    let store = KeyStore::open(&config, USER_PIN).unwrap();
    assert_eq!(store.public_key_set().keys.len(), 2);
    let canonical = protocol::canary(config.region, &config.active_kid).unwrap();
    let signature = store.sign_locator(&canonical).unwrap();
    let record = store
        .public_key_set()
        .keys
        .into_iter()
        .find(|key| key.kid == config.active_kid)
        .unwrap();
    let public: [u8; 32] = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(record.public_key_ed25519)
        .unwrap()
        .try_into()
        .unwrap();
    VerifyingKey::from_bytes(&public)
        .unwrap()
        .verify_strict(&canonical, &Signature::from_bytes(&signature))
        .unwrap();

    let mut foreign_domain = canonical;
    foreign_domain[1] = Region::Cn.wire_value();
    assert!(store.sign_locator(&foreign_domain).is_err());
}

fn signer_config(module: PathBuf, socket_path: PathBuf) -> SignerConfig {
    SignerConfig {
        version: 1,
        region: Region::Global,
        socket_path,
        expected_client_uid: 65532,
        expected_client_gid: 65532,
        pkcs11_module_path: module,
        token_label: "openpencil-test-locator".into(),
        pin_file: "/run/secrets/not-used-by-library-test".into(),
        active_kid: "locator-test-active".into(),
        keys: vec![
            KeyConfig {
                kid: "locator-test-active".into(),
                object_id_hex: "7101".into(),
            },
            KeyConfig {
                kid: "locator-test-next".into(),
                object_id_hex: "7102".into(),
            },
        ],
        request_timeout_ms: 2_000,
    }
}

use base64::Engine as _;
