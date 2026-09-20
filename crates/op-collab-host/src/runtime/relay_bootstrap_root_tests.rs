use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};
use sha2::{Digest as _, Sha256};

use super::bootstrap_roots::{
    add_development_roots_for_build, BUILTIN_ROOTS, CURRENT_BUILTIN_ROOT_KID,
    CURRENT_BUILTIN_ROOT_X, LEGACY_BUILTIN_ROOT_KID, LEGACY_BUILTIN_ROOT_X,
};
use super::*;

const NOW: u64 = 1_900_000_000;
const PRODUCTION_GENERATION_3_ENVELOPE_BASE64: &[u8] =
    include_bytes!("relay_bootstrap_testdata/op-hub-production-generation-3-envelope.base64");
const PRODUCTION_GENERATION_3_NOW: u64 = 1_786_259_263;
const PRODUCTION_GENERATION_3_SHA256: &str =
    "bbe68bfd2486a3ecf89335d825b8017bfc34c064512e5b42b182e8452913d63b";

fn bootstrap_key(kid: &str, bytes: [u8; 32]) -> BootstrapKey {
    BootstrapKey {
        kid: kid.to_owned(),
        x: URL_SAFE_NO_PAD.encode(bytes),
    }
}

fn payload(generation: u64) -> BootstrapPayload {
    let locator_cn = SigningKey::from_bytes(&[0x41; 32]);
    let locator_global = SigningKey::from_bytes(&[0x42; 32]);
    let relay_cn = DeviceStaticKey::from_private([0x43; 32]).unwrap();
    let relay_global = DeviceStaticKey::from_private([0x44; 32]).unwrap();
    BootstrapPayload {
        version: BOOTSTRAP_VERSION,
        generation,
        not_before_unix: NOW - 60,
        not_after_unix: NOW + 3_600,
        regions: vec![
            BootstrapRegion {
                region: "cn".to_owned(),
                relay_url: "wss://relay-cn.example/v1/tunnel".to_owned(),
                locator_url: "https://locator-cn.example/v1/locator".to_owned(),
                locator_keys: vec![bootstrap_key(
                    "locator_cn_1",
                    locator_cn.verifying_key().to_bytes(),
                )],
                relay_x25519_keys: vec![bootstrap_key("relay_cn_1", *relay_cn.public_key())],
            },
            BootstrapRegion {
                region: "global".to_owned(),
                relay_url: "wss://relay-global.example/v1/tunnel".to_owned(),
                locator_url: "https://locator-global.example/v1/locator".to_owned(),
                locator_keys: vec![bootstrap_key(
                    "locator_global_1",
                    locator_global.verifying_key().to_bytes(),
                )],
                relay_x25519_keys: vec![bootstrap_key(
                    "relay_global_1",
                    *relay_global.public_key(),
                )],
            },
        ],
    }
}

fn signed_envelope(signing: &SigningKey, kid: &str, generation: u64) -> Vec<u8> {
    let payload = serde_json::to_vec(&payload(generation)).unwrap();
    let mut signing_bytes = BOOTSTRAP_CONTEXT.to_vec();
    signing_bytes.extend_from_slice(&payload);
    serde_json::to_vec(&BootstrapEnvelope {
        version: BOOTSTRAP_VERSION,
        kid: kid.to_owned(),
        payload: URL_SAFE_NO_PAD.encode(payload),
        signature: URL_SAFE_NO_PAD.encode(signing.sign(&signing_bytes).to_bytes()),
    })
    .unwrap()
}

#[test]
fn builtin_bootstrap_roots_pin_the_legacy_and_current_keys() {
    assert_eq!(BUILTIN_ROOTS.len(), 2);
    let roots = builtin_roots().unwrap();
    assert_eq!(roots.len(), 2);
    for (kid, encoded, expected_spki_sha256) in [
        (
            LEGACY_BUILTIN_ROOT_KID,
            LEGACY_BUILTIN_ROOT_X,
            "53700c011a688b8077850f1330567c265f97cd5e34c9b67aa6695a3fe8afb20c",
        ),
        (
            CURRENT_BUILTIN_ROOT_KID,
            CURRENT_BUILTIN_ROOT_X,
            "7100466d7d118d6bf8f6f027febaae569f880690d223d2d794d7638b79252f41",
        ),
    ] {
        let expected = decode_fixed::<32>(encoded).unwrap();
        assert_eq!(roots.get(kid).unwrap().to_bytes(), expected);
        let mut spki = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        spki.extend_from_slice(&expected);
        assert_eq!(format!("{:x}", Sha256::digest(&spki)), expected_spki_sha256);
    }
}

#[test]
fn hsm_signed_production_generation_three_fixture_verifies_byte_exactly() {
    let encoded = PRODUCTION_GENERATION_3_ENVELOPE_BASE64
        .strip_suffix(b"\n")
        .expect("the base64 fixture must have one repository line terminator");
    assert_eq!(encoded.len(), 2_280);
    assert!(!encoded.contains(&b'\r'));
    assert!(!encoded.contains(&b'\n'));

    let body = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    assert_eq!(body.len(), 1_709);
    assert_eq!(body.last(), Some(&b'}'));
    assert!(!body.ends_with(b"\n"));
    assert_eq!(
        format!("{:x}", Sha256::digest(&body)),
        PRODUCTION_GENERATION_3_SHA256
    );

    let roots = builtin_roots().unwrap();
    let verified =
        verify_bootstrap(&body, &roots, PRODUCTION_GENERATION_3_NOW, false, true).unwrap();
    assert_eq!(verified.generation, 3);

    let cn = verified.region(RelayRegion::Cn).unwrap();
    assert_eq!(
        cn.relay_endpoint,
        RelayEndpoint::parse("wss://op.zseven.cn/v1/tunnel").unwrap()
    );
    assert_eq!(cn.locator_url, "https://op.zseven.cn/v1/locator");

    let global = verified.region(RelayRegion::Global).unwrap();
    assert_eq!(
        global.relay_endpoint,
        RelayEndpoint::parse("wss://op.zseven.tech/v1/tunnel").unwrap()
    );
    assert_eq!(global.locator_url, "https://op.zseven.tech/v1/locator");

    let mut trailing_lf = body;
    trailing_lf.push(b'\n');
    assert_eq!(
        verify_bootstrap(
            &trailing_lf,
            &roots,
            PRODUCTION_GENERATION_3_NOW,
            false,
            true,
        )
        .unwrap_err(),
        BootstrapError::InvalidResponse
    );
}

#[test]
fn envelope_kid_selects_exactly_one_generation_authorized_root() {
    let legacy = SigningKey::from_bytes(&[0x51; 32]);
    let current = SigningKey::from_bytes(&[0x52; 32]);
    let roots = HashMap::from([
        (LEGACY_BUILTIN_ROOT_KID.to_owned(), legacy.verifying_key()),
        (CURRENT_BUILTIN_ROOT_KID.to_owned(), current.verifying_key()),
    ]);

    for (signing, kid, generation) in [
        (&legacy, LEGACY_BUILTIN_ROOT_KID, 2),
        (&current, CURRENT_BUILTIN_ROOT_KID, 3),
    ] {
        assert!(verify_bootstrap(
            &signed_envelope(signing, kid, generation),
            &roots,
            NOW,
            false,
            true,
        )
        .is_ok());
    }

    assert_eq!(
        verify_bootstrap(
            &signed_envelope(&current, LEGACY_BUILTIN_ROOT_KID, 2),
            &roots,
            NOW,
            false,
            true,
        )
        .unwrap_err(),
        BootstrapError::InvalidSignature
    );

    for (signing, kid, generation) in [
        (&legacy, LEGACY_BUILTIN_ROOT_KID, 3),
        (&current, CURRENT_BUILTIN_ROOT_KID, 2),
    ] {
        assert_eq!(
            verify_bootstrap(
                &signed_envelope(signing, kid, generation),
                &roots,
                NOW,
                false,
                true,
            )
            .unwrap_err(),
            BootstrapError::InvalidSignature
        );
    }
}

#[test]
fn bootstrap_rejects_unknown_root_and_tampered_payload() {
    let current = SigningKey::from_bytes(&[0x52; 32]);
    let roots = HashMap::from([(CURRENT_BUILTIN_ROOT_KID.to_owned(), current.verifying_key())]);

    assert_eq!(
        verify_bootstrap(
            &signed_envelope(&current, "unknown-bootstrap-root", 3),
            &roots,
            NOW,
            false,
            true,
        )
        .unwrap_err(),
        BootstrapError::UnknownRoot
    );

    let body = signed_envelope(&current, CURRENT_BUILTIN_ROOT_KID, 3);
    let mut envelope: BootstrapEnvelope = serde_json::from_slice(&body).unwrap();
    let mut tampered: BootstrapPayload =
        serde_json::from_slice(&decode_bounded(&envelope.payload, MAX_PAYLOAD_BYTES).unwrap())
            .unwrap();
    tampered.generation += 1;
    envelope.payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&tampered).unwrap());
    let tampered = serde_json::to_vec(&envelope).unwrap();
    assert_eq!(
        verify_bootstrap(&tampered, &roots, NOW, false, true).unwrap_err(),
        BootstrapError::InvalidSignature
    );
}

#[test]
fn release_profile_ignores_environment_root_material() {
    let injected = SigningKey::from_bytes(&[0x61; 32]);
    let raw = format!(
        "injected_root={}",
        URL_SAFE_NO_PAD.encode(injected.verifying_key().to_bytes())
    );
    let mut release_roots = builtin_roots().unwrap();
    add_development_roots_for_build(&mut release_roots, false, Some(&raw)).unwrap();
    assert_eq!(release_roots.len(), BUILTIN_ROOTS.len());
    assert!(!release_roots.contains_key("injected_root"));

    let mut debug_roots = builtin_roots().unwrap();
    add_development_roots_for_build(&mut debug_roots, true, Some(&raw)).unwrap();
    assert_eq!(
        debug_roots.get("injected_root").unwrap(),
        &injected.verifying_key()
    );
}
