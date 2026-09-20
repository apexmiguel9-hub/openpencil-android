use super::*;
use crate::{AuthenticatedCollabSession, CollabAvailability, CollabUiRole};

fn key() -> CollabAdmissionRequestKey {
    CollabAdmissionRequestKey::new("owner-confirm-1").unwrap()
}

fn identity() -> CollabOwnerIdentityUi {
    CollabOwnerIdentityUi::from_verified(
        "11111111-1111-1111-1111-111111111111",
        "22222222-2222-2222-2222-222222222222",
        Some("Ada"),
        Some("https://cdn.example/a.png"),
    )
    .unwrap()
}

fn authenticating() -> CollabUiState {
    let mut state = CollabUiState::default();
    state.availability = CollabAvailability::Ready;
    state.set_phase(CollabConnectionPhase::Authenticating);
    state
}

#[test]
fn confirmation_is_published_only_while_authenticating_and_never_once_active() {
    let mut state = authenticating();
    assert!(state.publish_owner_confirmation(key(), identity()));
    assert_eq!(
        state
            .pending_owner_confirmation()
            .map(|pending| pending.identity().subject().to_owned()),
        Some("11111111-1111-1111-1111-111111111111".to_owned())
    );

    // A live session has already made the decision; a stale projection must
    // not resurface as a second prompt, and no other phase may create one.
    state.set_authenticated_session(
        CollabConnectionPhase::Active,
        AuthenticatedCollabSession {
            session_name: "Design".into(),
            role: CollabUiRole::Editor,
            share_endpoint: None,
        },
        Vec::new(),
    );
    assert!(state.pending_owner_confirmation().is_none());
    assert!(!state.publish_owner_confirmation(key(), identity()));

    let mut idle = CollabUiState::default();
    idle.availability = CollabAvailability::Ready;
    assert!(!idle.publish_owner_confirmation(key(), identity()));
    assert!(idle.pending_owner_confirmation().is_none());
}

#[test]
fn a_chosen_display_name_cannot_impersonate_the_authoritative_identifiers() {
    // A peer-chosen name that tries to read as somebody else's account line,
    // using an invisible right-to-left override to hide the seam.
    let hostile = CollabOwnerIdentityUi::from_verified(
        "11111111-1111-1111-1111-111111111111",
        "22222222-2222-2222-2222-222222222222",
        Some("Account\u{202e} 99999999-9999-9999-9999-999999999999"),
        None,
    )
    .unwrap();
    let claimed = hostile.claimed_display_name().unwrap();
    assert!(!claimed.contains('\u{202e}'));
    // The claim is kept in its own field: it can never be read back as the
    // subject or device id the confirmation is actually about.
    assert_eq!(hostile.subject(), "11111111-1111-1111-1111-111111111111");
    assert_eq!(hostile.device_id(), "22222222-2222-2222-2222-222222222222");
    assert!(claimed.chars().count() <= MAX_COLLAB_OWNER_DISPLAY_NAME_CHARS);

    // A name made only of invisible characters is dropped rather than painted
    // as an empty but present identity.
    let blank = CollabOwnerIdentityUi::from_verified(
        "11111111-1111-1111-1111-111111111111",
        "22222222-2222-2222-2222-222222222222",
        Some("\u{200b}\u{feff}"),
        Some("http://cdn.example/a.png"),
    )
    .unwrap();
    assert!(blank.claimed_display_name().is_none());
    // Non-https avatars never reach the prompt.
    assert!(blank.claimed_avatar_url().is_none());

    // Without an authoritative half there is nothing to confirm.
    assert!(CollabOwnerIdentityUi::from_verified("", "device", None, None).is_none());
    assert!(
        CollabOwnerIdentityUi::from_verified("subject", "\u{200b}", None, None).is_none(),
        "an identifier made of invisible characters is not a name"
    );
}

#[test]
fn owner_identity_is_redacted_in_debug_projections() {
    let mut state = authenticating();
    assert!(state.publish_owner_confirmation(key(), identity()));
    let debug = format!("{:?}", state.pending_owner_confirmation());
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("11111111-1111-1111-1111-111111111111"));
    assert!(!debug.contains("Ada"));
    assert!(!debug.contains("owner-confirm-1"));

    state.clear_owner_confirmation();
    assert!(state.pending_owner_confirmation().is_none());
}
