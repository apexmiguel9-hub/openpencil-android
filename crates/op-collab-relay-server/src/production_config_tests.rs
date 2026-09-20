#![cfg(test)]

use std::num::NonZeroU64;

use op_collab_relay_protocol::RelayRegion;

use crate::{ProductionRelayAuthConfig, ProductionRelayAuthConfigError};

#[test]
fn production_config_requires_x25519_or_explicit_reduced_mode() {
    let root = std::env::temp_dir().join("openpencil-production-config-test");
    let policy = root.join("private-policy.json");
    let locator = root.join("private-locator-keys.json");
    let x25519 = root.join("private-relay-x25519.json");
    let max_age = NonZeroU64::new(60).expect("non-zero max age");

    assert!(matches!(
        ProductionRelayAuthConfig::new(
            RelayRegion::Cn,
            policy.clone(),
            locator.clone(),
            None,
            max_age,
            false,
        ),
        Err(ProductionRelayAuthConfigError::ChallengeProofRequired)
    ));
    assert!(matches!(
        ProductionRelayAuthConfig::new(
            RelayRegion::Cn,
            policy.clone(),
            locator.clone(),
            Some(x25519.clone()),
            max_age,
            true,
        ),
        Err(ProductionRelayAuthConfigError::ConflictingProofPolicy)
    ));
    let full = ProductionRelayAuthConfig::new(
        RelayRegion::Cn,
        policy.clone(),
        locator.clone(),
        Some(x25519),
        max_age,
        false,
    )
    .expect("full proof configuration");
    assert_eq!(full.home_region(), RelayRegion::Cn);
    assert!(!full.reduced_assurance());
    let reduced =
        ProductionRelayAuthConfig::new(RelayRegion::Cn, policy, locator, None, max_age, true)
            .expect("explicit reduced-assurance configuration");
    assert!(reduced.reduced_assurance());
    let debug = format!("{reduced:?}");
    assert!(!debug.contains("private-policy"));
    assert!(!debug.contains("private-locator"));
}
