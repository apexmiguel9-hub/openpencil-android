use std::fs::Metadata;
use std::path::Path;
use std::time::SystemTime;

use super::error::OutputStateError;

/// The state of the sibling output at the instant overwrite consent is
/// granted. The inner representation stays private to keep the import mode
/// opaque outside this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OutputEntryState(EntryState);

impl OutputEntryState {
    pub(crate) fn is_missing(self) -> bool {
        matches!(self.0, EntryState::Missing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryState {
    Missing,
    Present(OutputFingerprint),
}

/// Cheap identity and mutation indicators avoid hashing a potentially huge
/// `.op` on the UI thread while detecting replacement and ordinary edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputFingerprint {
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    is_file: bool,
    is_dir: bool,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

impl OutputFingerprint {
    fn from_metadata(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt as _;

        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            changed_seconds: metadata.ctime(),
            #[cfg(unix)]
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

pub(crate) fn capture_output_state(
    output_path: &Path,
) -> Result<OutputEntryState, OutputStateError> {
    match std::fs::metadata(output_path) {
        Ok(metadata) => Ok(OutputEntryState(EntryState::Present(
            OutputFingerprint::from_metadata(&metadata),
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(OutputEntryState(EntryState::Missing))
        }
        // `std::io::Error` belongs to a crate this pass does not own, so its
        // message is carried as text; `to_string` keeps the adapter valid if
        // the source ever becomes a typed error of its own.
        Err(error) => Err(OutputStateError::Inspect {
            path: output_path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}
