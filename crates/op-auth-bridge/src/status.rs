//! Status snapshots surfaced to hosts, decoded from FFI payload JSON.

/// Mirror of the library's status codes; payload shapes documented per
/// variant. Unknown codes decode to [`AuthStatus::Idle`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthStatus {
    /// Session handle: signed out. Flow handle: unknown handle.
    Idle,
    /// Login flow: start request in flight.
    Starting,
    /// Waiting for the user to approve in the browser; the host opens
    /// `verification_uri` the first time it observes this state.
    WaitingApproval {
        verification_uri: String,
    },
    /// Approved; exchanging the pairing for a device token.
    Exchanging,
    /// Signed in (terminal for flows; steady state for the session).
    SignedIn {
        display_name: String,
        username: Option<String>,
        primary_email: Option<String>,
        avatar_url: Option<String>,
        device_id: String,
    },
    /// Terminal failure; `code` is one of `denied`, `expired`, `network`,
    /// `protocol`, `server`.
    Error {
        code: String,
    },
    Canceled,
}

impl AuthStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AuthStatus::SignedIn { .. } | AuthStatus::Error { .. } | AuthStatus::Canceled
        )
    }

    /// Decode one FFI observation. Used by the real backend; the stub
    /// never produces payloads (hence dead code in stub builds).
    #[cfg_attr(not(op_auth_prebuilt), allow(dead_code))]
    pub(crate) fn decode(code: i32, payload: Option<&str>) -> Self {
        let field = |key: &str| -> Option<String> {
            let parsed: serde_json::Value = serde_json::from_str(payload?).ok()?;
            parsed[key].as_str().map(str::to_string)
        };
        match code {
            1 => AuthStatus::Starting,
            2 => AuthStatus::WaitingApproval {
                verification_uri: field("verification_uri").unwrap_or_default(),
            },
            3 => AuthStatus::Exchanging,
            4 => AuthStatus::SignedIn {
                display_name: field("display_name").unwrap_or_default(),
                username: field("username"),
                primary_email: field("primary_email"),
                avatar_url: field("avatar_url"),
                device_id: field("device_id").unwrap_or_default(),
            },
            5 => AuthStatus::Error {
                code: field("code").unwrap_or_else(|| "protocol".to_string()),
            },
            6 => AuthStatus::Canceled,
            _ => AuthStatus::Idle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_signed_in_payload() {
        let payload = r#"{"display_name":"Kay","username":"kayshen_7","primary_email":"kay@example.com","avatar_url":"https://cdn.example/kay.png","device_id":"d1"}"#;
        assert_eq!(
            AuthStatus::decode(4, Some(payload)),
            AuthStatus::SignedIn {
                display_name: "Kay".to_string(),
                username: Some("kayshen_7".to_string()),
                primary_email: Some("kay@example.com".to_string()),
                avatar_url: Some("https://cdn.example/kay.png".to_string()),
                device_id: "d1".to_string(),
            }
        );
    }

    #[test]
    fn older_signed_in_payloads_decode_without_a_username() {
        let payload =
            r#"{"display_name":"Kay","primary_email":"kay@example.com","device_id":"d1"}"#;
        assert!(matches!(
            AuthStatus::decode(4, Some(payload)),
            AuthStatus::SignedIn { username: None, .. }
        ));
    }

    #[test]
    fn decodes_waiting_approval_and_error() {
        assert_eq!(
            AuthStatus::decode(2, Some(r#"{"verification_uri":"https://x/login"}"#)),
            AuthStatus::WaitingApproval {
                verification_uri: "https://x/login".to_string()
            }
        );
        assert_eq!(
            AuthStatus::decode(5, Some(r#"{"code":"denied"}"#)),
            AuthStatus::Error {
                code: "denied".to_string()
            }
        );
    }

    #[test]
    fn unknown_codes_and_garbage_payloads_are_safe() {
        assert_eq!(AuthStatus::decode(99, None), AuthStatus::Idle);
        assert_eq!(
            AuthStatus::decode(5, Some("not json")),
            AuthStatus::Error {
                code: "protocol".to_string()
            }
        );
    }
}
