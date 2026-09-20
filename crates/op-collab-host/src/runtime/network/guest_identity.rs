//! Which accounts a guest may accept as the owner of a session it joins.

use op_collab_transport::PeerIdentityPolicy;

/// Whether the guest worker will stop and make a human name the owner before
/// anything from the session is accepted.
///
/// This is the guest's counterpart to the owner's approval prompt, and it is
/// the only thing that can stand in for a pinned key. Claiming `Enforced`
/// without actually running that gate is a hole, not a relaxation, which is
/// why `guest_admission_plan` derives both halves from one decision instead of
/// letting a call site assert this on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GuestOwnerConfirmation {
    /// The worker blocks on an explicit human confirmation of the verified
    /// ticket identity before it authorizes or activates the connection.
    Enforced,
    /// No confirmation is requested; something else already authenticates the
    /// peer.
    NotRequired,
}

/// The account policy a guest admits under, paired with the human gate that
/// justifies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GuestAdmissionPlan<'a> {
    pub(super) policy: PeerIdentityPolicy<'a>,
    pub(super) confirmation: GuestOwnerConfirmation,
}

/// Decide how a guest admits the owner of the session it is joining.
///
/// An invite pins the owner's Noise static in its signed locator, and that pin
/// is checked before admission runs: the device is already authenticated, the
/// account behind it is beside the point, and no prompt is needed.
///
/// An unpinned LAN join has no such anchor — mDNS is spoofable and nothing
/// else names the peer. The subject check used to stand in for authentication
/// there, which also ruled out ever joining anyone else's session. It is
/// replaced rather than dropped: the guest is now shown the verified identity
/// and must accept it explicitly, the same kind of human decision the owner
/// has always made about each guest.
pub(super) fn guest_admission_plan<'a>(
    pinned_remote_static: Option<&[u8; 32]>,
    local_subject: &'a str,
) -> GuestAdmissionPlan<'a> {
    let confirmation = if pinned_remote_static.is_some() {
        GuestOwnerConfirmation::NotRequired
    } else {
        GuestOwnerConfirmation::Enforced
    };
    GuestAdmissionPlan {
        policy: guest_identity_policy(pinned_remote_static, confirmation, local_subject),
        confirmation,
    }
}

/// The account policy itself. A foreign account is admitted only when the
/// device key was pinned out of band or a human will be asked to name the
/// owner; with neither, the subject remains the authentication.
pub(super) fn guest_identity_policy<'a>(
    pinned_remote_static: Option<&[u8; 32]>,
    confirmation: GuestOwnerConfirmation,
    local_subject: &'a str,
) -> PeerIdentityPolicy<'a> {
    if pinned_remote_static.is_some() || confirmation == GuestOwnerConfirmation::Enforced {
        PeerIdentityPolicy::AnyIssuedAccount
    } else {
        PeerIdentityPolicy::SameAccount {
            subject: local_subject,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        guest_admission_plan, guest_identity_policy, GuestAdmissionPlan, GuestOwnerConfirmation,
    };
    use op_collab_transport::PeerIdentityPolicy;

    const LOCAL_SUBJECT: &str = "00000000-0000-0000-0000-000000000001";

    #[test]
    fn a_pinned_owner_key_admits_any_account_without_a_prompt() {
        // An invite pins the owner's Noise static in its signed locator, and
        // that pin is checked before admission runs. The device is therefore
        // already authenticated and the account behind it is irrelevant, which
        // is what makes cross-account collaboration by invite safe — and why
        // this path asks the guest to confirm nothing.
        assert_eq!(
            guest_admission_plan(Some(&[9_u8; 32]), LOCAL_SUBJECT),
            GuestAdmissionPlan {
                policy: PeerIdentityPolicy::AnyIssuedAccount,
                confirmation: GuestOwnerConfirmation::NotRequired,
            }
        );
    }

    #[test]
    fn an_unpinned_join_without_confirmation_still_requires_this_account() {
        // Regression guard. With no pinned key and no human decision, the
        // subject is the only thing authenticating the owner. Relaxing this
        // would let anyone on the LAN holding any valid ticket pose as the
        // owner, silently.
        assert_eq!(
            guest_identity_policy(None, GuestOwnerConfirmation::NotRequired, LOCAL_SUBJECT),
            PeerIdentityPolicy::SameAccount {
                subject: LOCAL_SUBJECT
            }
        );
    }

    #[test]
    fn an_unpinned_join_admits_a_foreign_account_only_behind_the_confirmation_gate() {
        // The confirmation is what replaces the subject check, so the two move
        // together: any issued account, but only once a human has been shown
        // whose account it is.
        assert_eq!(
            guest_identity_policy(None, GuestOwnerConfirmation::Enforced, LOCAL_SUBJECT),
            PeerIdentityPolicy::AnyIssuedAccount
        );
        assert_eq!(
            guest_admission_plan(None, LOCAL_SUBJECT),
            GuestAdmissionPlan {
                policy: PeerIdentityPolicy::AnyIssuedAccount,
                confirmation: GuestOwnerConfirmation::Enforced,
            }
        );
    }
}
