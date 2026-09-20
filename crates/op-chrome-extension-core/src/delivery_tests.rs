//! Tests for [`crate::delivery`] — where a capture is allowed to go.

use crate::delivery::{resolve, row_visible, Target, ACCOUNT_AVAILABLE};

/// The flag decides whether a capture may leave the machine at all, so its
/// value is asserted at COMPILE time: flipping it must fail the build until
/// the tests below — and the popup copy that follows from them — are
/// revisited. It is `true` now that op-hub answers `POST /api/v1/snapshots`;
/// turning it off again is equally a deliberate act.
const _: () = assert!(ACCOUNT_AVAILABLE);

#[test]
fn targets_round_trip_their_persisted_spelling() {
    assert_eq!(Target::Local.as_str(), "local");
    assert_eq!(Target::Account.as_str(), "account");
    assert_eq!(Target::from_stored("local"), Target::Local);
    assert_eq!(Target::from_stored("account"), Target::Account);
    assert_eq!(Target::from_stored(" account "), Target::Account);
}

#[test]
fn an_unknown_or_absent_target_is_the_local_editor() {
    assert_eq!(Target::default(), Target::Local);
    for raw in ["", "  ", "Account", "cloud", "null", "undefined", "true"] {
        assert_eq!(Target::from_stored(raw), Target::Local, "for {raw:?}");
    }
}

#[test]
fn signing_out_moves_delivery_back_to_the_local_editor() {
    // The session can expire between the popup opening and the button being
    // pressed. A capture must never be aimed at an account that is no longer
    // there — it goes where it has always gone instead.
    assert_eq!(resolve("account", false), Target::Local);
    assert_eq!(resolve("local", false), Target::Local);
    assert_eq!(resolve("local", true), Target::Local);
}

#[test]
fn a_signed_in_user_who_chose_the_account_gets_the_account() {
    assert_eq!(resolve("account", true), Target::Account);
    assert_eq!(resolve(" account ", true), Target::Account);
}

#[test]
fn only_the_exact_stored_spelling_opens_the_network_path() {
    // Every other stored value — a typo, a future build's spelling, a
    // JSON-ified `null` — must collapse to the local editor rather than
    // produce an upload the user did not ask for.
    for stored in [
        "",
        "  ",
        "Account",
        "cloud",
        "null",
        "undefined",
        "true",
        "hub",
    ] {
        assert_eq!(
            resolve(stored, true),
            Target::Local,
            "signed in, stored {stored:?}"
        );
    }
}

#[test]
fn the_delivery_row_is_only_worth_showing_to_a_signed_in_user() {
    assert!(row_visible(true));
    assert!(!row_visible(false));
}
