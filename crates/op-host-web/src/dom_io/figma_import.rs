//! Isolated browser Figma import Worker plus the compatibility fallback.

use op_editor_core::figma_import_state::ImportSource;

use crate::file_actions;
use crate::repaint_ctx::RepaintContext;

use super::import_generation::{begin_document_import, document_import_is_current};
use super::{
    console_warn, finish_document_import, js_bytes, open_file_picker, read_file, InnerRc, ReadMode,
};

/// A `.fig` file that could not be installed, tagged with the file name the
/// console line prefixes.
///
/// `Display` reproduces the ad-hoc `format!("{name}: {error}")` messages this
/// enum replaced byte for byte.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FigmaImportError {
    /// The converted canonical document could not be ingested.
    Ingest {
        file_name: String,
        source: file_actions::DocumentIngestError,
    },
    /// The browser `FileReader` produced no bytes for the fallback path.
    NoBytes { file_name: String },
}

impl std::fmt::Display for FigmaImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigmaImportError::Ingest { file_name, source } => {
                write!(f, "{file_name}: {source}")
            }
            FigmaImportError::NoBytes { file_name } => {
                write!(f, "{file_name}: file read produced no bytes")
            }
        }
    }
}

impl std::error::Error for FigmaImportError {}

/// Figma modal drop-zone → hidden `.fig` picker → isolated conversion Worker.
pub(super) fn import_figma<C: RepaintContext + 'static>(inner: &InnerRc<C>) {
    let inner = inner.clone();
    open_file_picker(
        ".fig",
        Box::new(move |file| {
            ingest_figma_file(&inner, file);
        }),
    );
}

/// Shared `.fig` ingestion for the import picker and drag-drop.
pub(super) fn ingest_figma_file<C: RepaintContext + 'static>(
    inner: &InnerRc<C>,
    file: web_sys::File,
) {
    let name = file.name();
    let generation = begin_document_import(inner, ImportSource::Figma);
    let stem = file_actions::file_stem(&name).to_string();
    let session_id = figma_temp_session_id(generation);
    let inner2 = inner.clone();
    let fallback_file = file.clone();
    let fallback_name = name.clone();
    let started = crate::figma_temp_bridge::start(
        &file,
        &stem,
        &session_id,
        move |worker_result| match worker_result {
            Ok(temp) => {
                if !document_import_is_current(&inner2, generation, ImportSource::Figma) {
                    crate::figma_temp_bridge::delete_session(&temp.session_id, temp.page_count);
                    return;
                }
                let result = file_actions::ingest_figma_temp_source(
                    &temp.full_document_json,
                    &temp.warnings_json,
                )
                .map_err(|source| FigmaImportError::Ingest {
                    file_name: fallback_name.clone(),
                    source,
                });
                match result {
                    Ok(ingested) => {
                        // The committed IndexedDB copy remains the loss-safe
                        // owner until Rust has parsed and installed the full
                        // canonical document. Only then release its shards.
                        if finish_document_import(
                            &inner2,
                            generation,
                            ImportSource::Figma,
                            Ok::<_, FigmaImportError>(ingested),
                            "import-figma",
                        ) {
                            crate::figma_temp_bridge::delete_session(
                                &temp.session_id,
                                temp.page_count,
                            );
                        }
                    }
                    Err(error) => {
                        crate::figma_temp_bridge::delete_session(&temp.session_id, temp.page_count);
                        finish_document_import(
                            &inner2,
                            generation,
                            ImportSource::Figma,
                            Err(error),
                            "import-figma",
                        );
                    }
                }
            }
            Err(error) => {
                // Cancellation runs its callback one task after a replacement
                // advances the generation. Do not copy the old file into main
                // WASM merely to have the fallback discard it later.
                if !document_import_is_current(&inner2, generation, ImportSource::Figma) {
                    return;
                }
                console_warn(&format!(
                    "[import-figma] isolated Worker unavailable ({error}); using main-thread fallback"
                ));
                ingest_figma_file_fallback(&inner2, fallback_file, fallback_name, generation);
            }
        },
    );
    if let Err(error) = started {
        console_warn(&format!(
            "[import-figma] could not start isolated Worker ({error:?}); using main-thread fallback"
        ));
        ingest_figma_file_fallback(inner, file, name, generation);
    }
}

/// Compatibility fallback for browsers whose CSP, Worker implementation, or
/// IndexedDB quota prevents the isolated path.
fn ingest_figma_file_fallback<C: RepaintContext + 'static>(
    inner: &InnerRc<C>,
    file: web_sys::File,
    name: String,
    generation: u64,
) {
    let inner = inner.clone();
    read_file(
        file,
        ReadMode::Bytes,
        Box::new(move |value| {
            if !document_import_is_current(&inner, generation, ImportSource::Figma) {
                return;
            }
            let stem = file_actions::file_stem(&name).to_string();
            // Parse outside any `inner` borrow — it is the heavy fallback step.
            let result = match js_bytes(&value) {
                Some(bytes) => file_actions::ingest_figma_bytes(&bytes, &stem).map_err(|source| {
                    FigmaImportError::Ingest {
                        file_name: name.clone(),
                        source,
                    }
                }),
                None => Err(FigmaImportError::NoBytes {
                    file_name: name.clone(),
                }),
            };
            finish_document_import(
                &inner,
                generation,
                ImportSource::Figma,
                result,
                "import-figma",
            );
        }),
    );
}

fn figma_temp_session_id(generation: u64) -> String {
    let millis = js_sys::Date::now() as u64;
    let nonce = (js_sys::Math::random() * 1_000_000_000.0) as u64;
    format!("fig-{millis:013x}-{generation:016x}-{nonce:08x}")
}
