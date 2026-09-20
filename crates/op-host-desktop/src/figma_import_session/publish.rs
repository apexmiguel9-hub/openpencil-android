//! Staging and publication of the converted `.op` beside its `.fig` source.
//!
//! Pure code motion out of `figma_import_session.rs`: the spine kept the
//! session state machine and the worker plumbing, while this sibling owns the
//! filesystem tail — allocate a hidden staging file, serialize into it, and
//! publish it under the fixed `Design.op` name or a numbered `Design (N).op`
//! copy. The split keeps both files under the repo's 800-line-per-file cap
//! now that the failure paths carry [`FigmaImportError`] instead of `String`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use op_host_services::doc_io::{commit_staged_document, save_to_path};

use super::error::FigmaImportError;
use super::output_guard::capture_output_state;
use super::worker_control::CancellationToken;
use super::{ImportOutputMode, PreparedImport};

static IMPORT_OUTPUT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct PersistedFile {
    // Publication is durable once the completed sibling atomically replaces
    // the fixed output. If UI ownership changes in the tiny post-publish race,
    // leave that valid file in place; deleting by path could race another
    // importer replacing the same destination.
    output_path: PathBuf,
}

impl PersistedFile {
    pub(super) fn new(output_path: PathBuf) -> Self {
        Self { output_path }
    }

    pub(super) fn path(&self) -> &Path {
        &self.output_path
    }

    pub(super) fn commit(self) -> PathBuf {
        self.output_path
    }
}

pub(super) struct CompletedImport {
    pub(super) prepared: PreparedImport,
    pub(super) persisted: Result<PersistedFile, FigmaImportError>,
}

pub(super) type PersistResult = Result<CompletedImport, FigmaImportError>;

/// The primary imported document is the fixed sibling `Design.op`. Re-imports
/// replace it only after confirmation; keeping both publishes `Design (N).op`.
pub(super) fn adjacent_op_base_path(source_path: &Path) -> Result<PathBuf, FigmaImportError> {
    if source_path.file_name().is_none() {
        return Err(FigmaImportError::SourceHasNoFileName);
    }
    let output_path = source_path.with_extension("op");
    if output_path == source_path {
        return Err(FigmaImportError::SourceAlreadyOp);
    }
    Ok(output_path)
}

fn import_staging_path(source_path: &Path) -> Result<PathBuf, FigmaImportError> {
    let base = adjacent_op_base_path(source_path)?;
    let parent = base.parent().unwrap_or_else(|| Path::new(""));
    let name = base
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "Figma Import.op".into());
    for _ in 0..100 {
        let sequence = IMPORT_OUTPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{name}.op-import-{}-{sequence}",
            std::process::id()
        ));
        // `std::io::Error` is not ours to type; carry its message.
        if !candidate
            .try_exists()
            .map_err(|error| FigmaImportError::StagingProbe {
                path: candidate.clone(),
                message: error.to_string(),
            })?
        {
            return Ok(candidate);
        }
    }
    Err(FigmaImportError::StagingNamesExhausted {
        source_path: source_path.to_path_buf(),
    })
}

fn remove_import_artifact(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => eprintln!(
            "[import-figma] could not remove staging artifact {}: {error}",
            path.display()
        ),
    }
}

fn remove_legacy_sidecar(path: &Path) {
    match std::fs::remove_file(op_host_services::doc_io::sidecar_path(path)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => eprintln!(
            "[import-figma] stale view-state sidecar cleanup failed for {}: {error}",
            path.display()
        ),
    }
}

fn publish_new_link(staging_path: &Path, output_path: &Path) -> Result<PathBuf, std::io::Error> {
    std::fs::hard_link(staging_path, output_path)?;
    remove_legacy_sidecar(output_path);
    Ok(output_path.to_path_buf())
}

pub(super) fn publish_numbered_copy(
    staging_path: &Path,
    source_path: &Path,
) -> Result<PathBuf, FigmaImportError> {
    let base = adjacent_op_base_path(source_path)?;
    let parent = base.parent().unwrap_or_else(|| Path::new(""));
    let stem = base
        .file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_else(|| "Figma Import".into());
    for suffix in 1..=10_000 {
        let candidate = parent.join(format!("{stem} ({suffix}).op"));
        match publish_new_link(staging_path, &candidate) {
            Ok(path) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(FigmaImportError::Publish {
                    path: candidate,
                    message: error.to_string(),
                });
            }
        }
    }
    Err(FigmaImportError::OutputNamesExhausted {
        source_path: source_path.to_path_buf(),
    })
}

fn publish_staged_op(
    staging_path: &Path,
    source_path: &Path,
    output_mode: ImportOutputMode,
    cancellation: &CancellationToken,
) -> Result<PathBuf, FigmaImportError> {
    let output_path = adjacent_op_base_path(source_path)?;
    if cancellation.is_cancelled() {
        return Err(FigmaImportError::Cancelled);
    }
    match output_mode {
        ImportOutputMode::ReplaceFixed { expected } => {
            let unchanged = capture_output_state(&output_path)
                .map(|current| current == expected)
                .unwrap_or_else(|error| {
                    eprintln!(
                        "[import-figma] fixed output could not be revalidated; preserving it: {error}"
                    );
                    false
                });
            if !unchanged {
                // Consent applied to the exact entry observed after Yes. A
                // later edit, replacement, deletion, or creation is a new
                // state; preserve it and publish without another worker-side
                // prompt.
                return publish_numbered_copy(staging_path, source_path);
            }
            if expected.is_missing() {
                return match publish_new_link(staging_path, &output_path) {
                    Ok(path) => Ok(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        publish_numbered_copy(staging_path, source_path)
                    }
                    Err(error) => Err(FigmaImportError::Publish {
                        path: output_path,
                        message: error.to_string(),
                    }),
                };
            }
            // `commit_staged_document` lives in `op-host-services`, a crate
            // this pass does not own; carry its message.
            commit_staged_document(staging_path, &output_path).map_err(|error| {
                FigmaImportError::Publish {
                    path: output_path.clone(),
                    message: error.to_string(),
                }
            })?;
            Ok(output_path)
        }
        ImportOutputMode::NumberedCopy => publish_numbered_copy(staging_path, source_path),
        ImportOutputMode::CreateFixed => match publish_new_link(staging_path, &output_path) {
            Ok(path) => Ok(path),
            // A fixed file appeared after the initial existence check. Never
            // overwrite it without consent; preserve both with a numbered copy.
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                publish_numbered_copy(staging_path, source_path)
            }
            Err(error) => Err(FigmaImportError::Publish {
                path: output_path,
                message: error.to_string(),
            }),
        },
    }
}

/// Persist on the conversion worker before moving the imported state to the
/// UI thread. This keeps the large canonical serialization off the event loop
/// and makes the returned path a truthful, already-committed `current_path`.
pub(super) fn persist_import_next_to_source(
    prepared: PreparedImport,
    source_path: &Path,
    output_mode: ImportOutputMode,
    cancellation: &CancellationToken,
) -> CompletedImport {
    let persisted = (|| {
        if cancellation.is_cancelled() {
            return Err(FigmaImportError::Cancelled);
        }
        let staging_path = import_staging_path(source_path)?;
        if let Err(error) = save_to_path(&prepared.state, &staging_path) {
            remove_import_artifact(&staging_path);
            // `WriteStaged` re-formats its own "beside <source>" envelope
            // around whatever the writer reported, so the underlying reason
            // is kept as text rather than nested. `to_string` renders it
            // through `Display`, which holds whether `doc_io::save_to_path`
            // reports a `String` or its own typed error.
            return Err(FigmaImportError::WriteStaged {
                source_path: source_path.to_path_buf(),
                message: error.to_string(),
            });
        }
        if cancellation.is_cancelled() {
            remove_import_artifact(&staging_path);
            return Err(FigmaImportError::Cancelled);
        }
        let output_path = publish_staged_op(&staging_path, source_path, output_mode, cancellation);
        remove_import_artifact(&staging_path);
        Ok(PersistedFile::new(output_path?))
    })();
    CompletedImport {
        prepared,
        persisted,
    }
}
