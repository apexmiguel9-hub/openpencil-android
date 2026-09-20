//! Typed failures for the offline `.fig` → `.op` conversion endpoint
//! (`figma_convert.rs`), which the VS Code extension POSTs raw Figma bytes to
//! because it cannot parse fig-kiwi itself.
//!
//! Style follows `op_orchestrator::OrchestratorError`: a plain enum plus a
//! hand-written `Display`, no `thiserror` and no new dependency. The variants
//! carry STRUCTURED fields and `Display` re-formats the sentence, so the
//! `{"ok":false,"error":…}` body the extension reads is reproduced byte for
//! byte while the route can match on the reason instead of the prose.
//!
//! What the enum adds is the REQUEST / DOCUMENT split the flat strings could
//! not express: `BadRequest` / `BadBase64` mean the extension sent something
//! malformed and re-sending the same bytes will fail again, while `Parse` /
//! `Encode` mean the `.fig` payload arrived intact and this build could not
//! turn it into a document. The route answers `400` for every variant today,
//! exactly as before — a caller that wants to distinguish them can now match
//! on the variant instead of substring-matching the message.
//!
//! Two inbound seams speak `String` in crates this pass does not own and are
//! carried verbatim: `op_figma::parse_fig_binary`'s `Debug`-formatted
//! rejection (`Parse`) and the `jian_ops_schema` / `serde_json` /
//! `String::from_utf8` writers behind the response assembly (`Encode`).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FigmaConvertError {
    /// The request envelope is not the `{name, bytesB64}` JSON this endpoint
    /// accepts.
    BadRequest { detail: String },
    /// The envelope parsed but `bytesB64` is not valid standard base64.
    BadBase64 { detail: String },
    /// The decoded bytes are not a fig-kiwi document this build can read.
    /// `detail` is `op_figma`'s own `Debug` rendering, carried verbatim
    /// because that is what the pre-conversion `{e:?}` emitted.
    Parse { name: String, detail: String },
    /// The document converted, but assembling the JSON response failed —
    /// the streaming document writer, the warnings serializer, or the final
    /// UTF-8 check.
    Encode { detail: String },
}

impl fmt::Display for FigmaConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FigmaConvertError::BadRequest { detail } => {
                write!(f, "bad convert request: {detail}")
            }
            FigmaConvertError::BadBase64 { detail } => write!(f, "bad base64: {detail}"),
            FigmaConvertError::Parse { name, detail } => write!(f, "parse {name}: {detail}"),
            FigmaConvertError::Encode { detail } => f.write_str(detail),
        }
    }
}

impl std::error::Error for FigmaConvertError {}
