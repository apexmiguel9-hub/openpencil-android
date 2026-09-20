//! Lazy browser ZIP import over `Blob.slice()`.
//!
//! The desktop can hand a seekable file directly to `op-html`. Browsers only
//! expose asynchronous Blob reads, so this session mirrors the same ZIP flow:
//! read the EOCD suffix, read the central directory, then fetch only the HTML
//! entry and resources the importer actually requests. The complete archive is
//! never copied into Wasm memory.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::rc::Rc;

use wasm_bindgen::JsCast;

use super::browser_files::{read_blob_range, BlobReadError};
use crate::file_actions::{self, IngestedDoc};

type ZipImportResult = Result<IngestedDoc, ZipImportError>;
type ZipImportDone = Box<dyn FnOnce(ZipImportResult)>;

/// A browser ZIP import that could not be completed.
///
/// `Display` reproduces the ad-hoc `String` messages this enum replaced byte
/// for byte, so the `[import-html] …` console lines stay identical. `Zip`
/// carries `op_html`'s own typed error already rendered, exactly as the
/// previous `error.to_string()` adapters did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ZipImportError {
    /// The picked file's size cannot drive `Blob.slice()` range reads.
    SizeInvalid,
    /// A range read failed.
    Blob(BlobReadError),
    /// The shared ZIP reader rejected the archive.
    Zip(String),
    /// The central directory has not been parsed yet.
    ManifestUnavailable,
    /// A requested entry index is outside the manifest.
    EntryIndexInvalid(usize),
    /// The selected HTML entry has not been decoded yet.
    HtmlEntryUnavailable,
    /// The HTML entry parsed but produced no nodes.
    NoImportableContent,
}

impl std::fmt::Display for ZipImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZipImportError::SizeInvalid => {
                write!(f, "ZIP file size is invalid for browser range reads")
            }
            ZipImportError::Blob(error) => write!(f, "{error}"),
            ZipImportError::Zip(error) => write!(f, "{error}"),
            ZipImportError::ManifestUnavailable => write!(f, "ZIP manifest is unavailable"),
            ZipImportError::EntryIndexInvalid(index) => {
                write!(f, "ZIP entry index {index} is invalid")
            }
            ZipImportError::HtmlEntryUnavailable => write!(f, "ZIP HTML entry is unavailable"),
            ZipImportError::NoImportableContent => {
                write!(f, "HTML entry contains no importable content")
            }
        }
    }
}

impl std::error::Error for ZipImportError {}

impl From<BlobReadError> for ZipImportError {
    fn from(error: BlobReadError) -> Self {
        ZipImportError::Blob(error)
    }
}

impl From<ZipImportError> for String {
    fn from(error: ZipImportError) -> String {
        error.to_string()
    }
}
type IsActive = Box<dyn Fn() -> bool>;
type SessionRef = Rc<RefCell<ZipImportSession>>;

const MAX_SAFE_FILE_SIZE: f64 = 9_007_199_254_740_991.0;

struct ZipImportSession {
    file: web_sys::File,
    archive_len: u64,
    document_name: String,
    manifest: Option<op_html::HtmlZipRangeManifest>,
    loaded_entries: HashMap<usize, Vec<u8>>,
    is_active: IsActive,
    done: Option<ZipImportDone>,
}

pub(super) fn start(file: web_sys::File, is_active: IsActive, done: ZipImportDone) {
    let size = file.size();
    if !size.is_finite()
        || size < op_html::HTML_ZIP_EOCD_MIN_BYTES as f64
        || size > MAX_SAFE_FILE_SIZE
        || size.fract() != 0.0
    {
        done(Err(ZipImportError::SizeInvalid));
        return;
    }
    let archive_len = size as u64;
    let document_name = file_actions::file_stem(&file.name()).to_string();
    let session = Rc::new(RefCell::new(ZipImportSession {
        file,
        archive_len,
        document_name,
        manifest: None,
        loaded_entries: HashMap::new(),
        is_active,
        done: Some(done),
    }));
    read_tail(session);
}

fn read_tail(session: SessionRef) {
    let (file, archive_len) = {
        let session = session.borrow();
        (session.file.clone(), session.archive_len)
    };
    let length = archive_len.min(op_html::HTML_ZIP_TAIL_BYTES as u64);
    let offset = archive_len - length;
    let session2 = session.clone();
    read_blob_range(
        file.unchecked_ref(),
        offset,
        length,
        Box::new(move |result| {
            if abandon_if_stale(&session2) {
                return;
            }
            let tail = match result {
                Ok(bytes) => bytes,
                Err(error) => {
                    finish(&session2, Err(error.into()));
                    return;
                }
            };
            let locator = match op_html::locate_html_zip_directory(archive_len, &tail) {
                Ok(locator) => locator,
                Err(error) => {
                    finish(&session2, Err(ZipImportError::Zip(error.to_string())));
                    return;
                }
            };
            read_directory(session2, locator);
        }),
    );
}

fn read_directory(session: SessionRef, locator: op_html::HtmlZipDirectoryLocator) {
    let file = session.borrow().file.clone();
    let session2 = session.clone();
    read_blob_range(
        file.unchecked_ref(),
        locator.central_directory_offset,
        locator.central_directory_size,
        Box::new(move |result| {
            if abandon_if_stale(&session2) {
                return;
            }
            let directory = match result {
                Ok(bytes) => bytes,
                Err(error) => {
                    finish(&session2, Err(error.into()));
                    return;
                }
            };
            let manifest = match op_html::parse_html_zip_central_directory(&locator, &directory) {
                Ok(manifest) => manifest,
                Err(error) => {
                    finish(&session2, Err(ZipImportError::Zip(error.to_string())));
                    return;
                }
            };
            let html_entry = manifest.html_entry_index;
            session2.borrow_mut().manifest = Some(manifest);
            let session3 = session2.clone();
            load_entry(
                session2,
                html_entry,
                Box::new(move |result| match result {
                    Ok(()) => import_until_resolved(session3),
                    Err(error) => finish(&session3, Err(error)),
                }),
            );
        }),
    );
}

type EntryLoaded = Box<dyn FnOnce(Result<(), ZipImportError>)>;

fn load_entry(session: SessionRef, index: usize, done: EntryLoaded) {
    if session.borrow().loaded_entries.contains_key(&index) {
        done(Ok(()));
        return;
    }
    let (file, manifest, entry) = {
        let session = session.borrow();
        let Some(manifest) = session.manifest.clone() else {
            done(Err(ZipImportError::ManifestUnavailable));
            return;
        };
        let Some(entry) = manifest.entries.get(index).cloned() else {
            done(Err(ZipImportError::EntryIndexInvalid(index)));
            return;
        };
        (session.file.clone(), manifest, entry)
    };

    let session2 = session.clone();
    read_blob_range(
        file.unchecked_ref(),
        entry.local_header_offset,
        op_html::HTML_ZIP_LOCAL_HEADER_FIXED_BYTES as u64,
        Box::new(move |result| {
            if abandon_if_stale(&session2) {
                return;
            }
            let local_header_prefix = match result {
                Ok(bytes) => bytes,
                Err(error) => {
                    done(Err(error.into()));
                    return;
                }
            };
            let header_length =
                match op_html::html_zip_local_header_length(&entry, &local_header_prefix) {
                    Ok(length) => length,
                    Err(error) => {
                        done(Err(ZipImportError::Zip(error.to_string())));
                        return;
                    }
                };
            let file = session2.borrow().file.clone();
            let session3 = session2.clone();
            read_blob_range(
                file.unchecked_ref(),
                entry.local_header_offset,
                header_length,
                Box::new(move |result| {
                    if abandon_if_stale(&session3) {
                        return;
                    }
                    let local_header = match result {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            done(Err(error.into()));
                            return;
                        }
                    };
                    if let Err(error) =
                        op_html::validate_html_zip_local_header(&entry, &local_header)
                    {
                        done(Err(ZipImportError::Zip(error.to_string())));
                        return;
                    }
                    let range = match op_html::html_zip_entry_data_range(
                        &manifest,
                        &entry,
                        &local_header,
                    ) {
                        Ok(range) => range,
                        Err(error) => {
                            done(Err(ZipImportError::Zip(error.to_string())));
                            return;
                        }
                    };
                    let file = session3.borrow().file.clone();
                    let session4 = session3.clone();
                    read_blob_range(
                        file.unchecked_ref(),
                        range.offset,
                        range.length,
                        Box::new(move |result| {
                            if abandon_if_stale(&session4) {
                                return;
                            }
                            let compressed = match result {
                                Ok(bytes) => bytes,
                                Err(error) => {
                                    done(Err(error.into()));
                                    return;
                                }
                            };
                            let decoded = match op_html::decode_html_zip_entry(
                                &entry,
                                &local_header,
                                &compressed,
                            ) {
                                Ok(bytes) => bytes,
                                Err(error) => {
                                    done(Err(ZipImportError::Zip(error.to_string())));
                                    return;
                                }
                            };
                            session4.borrow_mut().loaded_entries.insert(index, decoded);
                            done(Ok(()));
                        }),
                    );
                }),
            );
        }),
    );
}

fn import_until_resolved(session: SessionRef) {
    if abandon_if_stale(&session) {
        return;
    }
    let probe = probe_import(&session);
    let (imported, missing) = match probe {
        Ok(result) => result,
        Err(error) => {
            finish(&session, Err(error));
            return;
        }
    };
    if missing.is_empty() {
        if imported.document.children.is_empty() {
            finish(&session, Err(ZipImportError::NoImportableContent));
            return;
        }
        finish(
            &session,
            Ok(IngestedDoc::from_html(
                op_editor_core::EditorState::from_document(imported.document),
                imported.warnings,
                &imported.diagnostics,
            )),
        );
        return;
    }
    load_entry_queue(session, missing.into());
}

fn probe_import(
    session: &SessionRef,
) -> Result<(op_html::HtmlDocumentResult, Vec<usize>), ZipImportError> {
    let session = session.borrow();
    let manifest = session
        .manifest
        .as_ref()
        .ok_or(ZipImportError::ManifestUnavailable)?;
    let entry = manifest.html_entry();
    let html_bytes = session
        .loaded_entries
        .get(&manifest.html_entry_index)
        .ok_or(ZipImportError::HtmlEntryUnavailable)?;
    let html = op_html::html_encoding::decode_html_bytes(html_bytes);
    let requested = RefCell::new(BTreeSet::new());
    let fetcher = |url: &str| {
        let index = manifest.entry_for_resource_url(url)?;
        match session.loaded_entries.get(&index) {
            Some(bytes) => Some(bytes.clone()),
            None => {
                requested.borrow_mut().insert(index);
                None
            }
        }
    };
    let options = op_html::HtmlImportOptions {
        document_name: Some(session.document_name.clone()),
        base_url: Some(op_html::html_zip_project_url(&entry.relative_path)),
        ..Default::default()
    };
    let mut imported = op_html::import_html_document(&html, &options, Some(&fetcher), None);
    if manifest.html_entry_count > 1 {
        imported.push_warning(op_html::ImportWarning::MultipleHtmlEntries {
            count: manifest.html_entry_count,
            entry: entry.relative_path.clone(),
        });
    }
    let missing = requested.into_inner().into_iter().collect();
    Ok((imported, missing))
}

fn load_entry_queue(session: SessionRef, mut queue: VecDeque<usize>) {
    let Some(index) = queue.pop_front() else {
        import_until_resolved(session);
        return;
    };
    let session2 = session.clone();
    load_entry(
        session,
        index,
        Box::new(move |result| match result {
            Ok(()) => load_entry_queue(session2, queue),
            Err(error) => finish(&session2, Err(error)),
        }),
    );
}

fn finish(session: &SessionRef, result: ZipImportResult) {
    let done = session.borrow_mut().done.take();
    if let Some(done) = done {
        done(result);
    }
}

fn abandon_if_stale(session: &SessionRef) -> bool {
    let active = {
        let session = session.borrow();
        (session.is_active)()
    };
    if active {
        return false;
    }
    let mut session = session.borrow_mut();
    session.done.take();
    session.loaded_entries.clear();
    true
}
