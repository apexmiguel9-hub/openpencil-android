//! Lazy HTML directory import.
//!
//! Directory enumeration retains browser `File` handles and project-relative
//! paths only. This session reads the selected HTML entry first, then repeats
//! the importer with a fetcher that records missing project resources. Only
//! those referenced CSS/image files are copied into Wasm memory.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::rc::Rc;

use url::Url;
use wasm_bindgen::JsCast;

use super::browser_files::{read_blob_range, BlobReadError};
use super::drop_entries::DroppedProjectFile;
use crate::file_actions::IngestedDoc;

type DirectoryImportResult = Result<IngestedDoc, DirectoryImportError>;
type DirectoryImportDone = Box<dyn FnOnce(DirectoryImportResult)>;
type EntryLoaded = Box<dyn FnOnce(Result<(), DirectoryImportError>)>;

/// A lazy HTML directory import that could not be completed.
///
/// `Display` reproduces the ad-hoc `String` messages this enum replaced byte
/// for byte, so the `[import-html] …` console lines stay identical.
/// `SelectEntry` carries `op_html`'s own typed error already rendered, exactly
/// as the previous `error.to_string()` adapter did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DirectoryImportError {
    /// The shared HTML entry selector rejected the file set.
    SelectEntry(String),
    /// A project-relative path is empty, absolute, or escapes the root.
    InvalidPath(String),
    /// Two entries normalize to the same project-relative path.
    DuplicatePath(String),
    /// The selected entry vanished while the index was being built.
    SelectedEntryMissing,
    /// A requested entry index is outside the index.
    EntryIndexInvalid(usize),
    /// The browser `File` behind an index entry is gone.
    SourceUnavailable(String),
    /// A `File.size` value cannot drive a range read.
    InvalidFileSize(String),
    /// A range read of a project file failed.
    ReadFailed { path: String, source: BlobReadError },
    /// The selected HTML entry has not been read yet.
    HtmlEntryUnavailable(String),
    /// The HTML entry parsed but produced no nodes.
    NoImportableContent,
}

impl std::fmt::Display for DirectoryImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirectoryImportError::SelectEntry(error) => write!(f, "{error}"),
            DirectoryImportError::InvalidPath(path) => {
                write!(f, "invalid project path `{path}`")
            }
            DirectoryImportError::DuplicatePath(path) => {
                write!(f, "duplicate project path `{path}`")
            }
            DirectoryImportError::SelectedEntryMissing => {
                write!(f, "selected HTML entry is missing from directory index")
            }
            DirectoryImportError::EntryIndexInvalid(index) => {
                write!(f, "directory entry index {index} is invalid")
            }
            DirectoryImportError::SourceUnavailable(path) => {
                write!(f, "directory source for `{path}` is unavailable")
            }
            DirectoryImportError::InvalidFileSize(path) => {
                write!(f, "invalid file size for `{path}`")
            }
            DirectoryImportError::ReadFailed { path, source } => {
                write!(f, "failed to read `{path}`: {source}")
            }
            DirectoryImportError::HtmlEntryUnavailable(path) => {
                write!(f, "HTML entry `{path}` is unavailable")
            }
            DirectoryImportError::NoImportableContent => {
                write!(f, "HTML directory entry contains no importable content")
            }
        }
    }
}

impl std::error::Error for DirectoryImportError {}

impl From<DirectoryImportError> for String {
    fn from(error: DirectoryImportError) -> String {
        error.to_string()
    }
}
type IsActive = Box<dyn Fn() -> bool>;
type SessionRef = Rc<RefCell<DirectoryImportSession>>;

const MAX_SAFE_FILE_SIZE: f64 = 9_007_199_254_740_991.0;

struct DirectoryImportSession {
    files: Vec<DroppedProjectFile>,
    index: DirectoryProjectIndex,
    loaded_entries: HashMap<usize, Vec<u8>>,
    is_active: IsActive,
    done: Option<DirectoryImportDone>,
}

#[derive(Debug)]
struct DirectoryIndexEntry {
    relative_path: String,
    source_index: usize,
    virtual_url: String,
}

#[derive(Debug)]
struct DirectoryProjectIndex {
    entries: Vec<DirectoryIndexEntry>,
    resource_by_url: HashMap<String, usize>,
    html_entry_index: usize,
    html_entry_count: usize,
}

impl DirectoryProjectIndex {
    fn prepare(paths: &[String]) -> Result<Self, DirectoryImportError> {
        let placeholders: Vec<_> = paths
            .iter()
            .map(|path| op_html::HtmlProjectFile::new(path, Vec::new()))
            .collect();
        let selected_path = op_html::select_html_entry(&placeholders)
            .map_err(|error| DirectoryImportError::SelectEntry(error.to_string()))?;

        let mut prepared = Vec::with_capacity(paths.len());
        for (source_index, path) in paths.iter().enumerate() {
            let relative_path = normalize_relative_path(path)?;
            if is_ignored_metadata_path(&relative_path) {
                continue;
            }
            prepared.push((source_index, relative_path));
        }
        strip_common_root(&mut prepared);

        let mut seen = HashSet::with_capacity(prepared.len());
        let mut entries = Vec::with_capacity(prepared.len());
        let mut resource_by_url = HashMap::with_capacity(prepared.len());
        for (source_index, relative_path) in prepared {
            if !seen.insert(relative_path.clone()) {
                return Err(DirectoryImportError::DuplicatePath(relative_path));
            }
            let virtual_url = virtual_project_url(&relative_path);
            let entry_index = entries.len();
            resource_by_url.insert(virtual_url.clone(), entry_index);
            entries.push(DirectoryIndexEntry {
                relative_path,
                source_index,
                virtual_url,
            });
        }
        let html_entry_index = entries
            .iter()
            .position(|entry| entry.relative_path == selected_path)
            .ok_or(DirectoryImportError::SelectedEntryMissing)?;
        let html_entry_count = entries
            .iter()
            .filter(|entry| is_html_path(&entry.relative_path))
            .count();
        Ok(Self {
            entries,
            resource_by_url,
            html_entry_index,
            html_entry_count,
        })
    }

    fn entry_for_resource_url(&self, requested: &str) -> Option<usize> {
        let key = canonical_virtual_resource_url(requested)?;
        self.resource_by_url.get(&key).copied()
    }
}

pub(super) fn start(
    files: Vec<DroppedProjectFile>,
    is_active: IsActive,
    done: DirectoryImportDone,
) {
    if !is_active() {
        return;
    }
    let paths: Vec<_> = files
        .iter()
        .map(|entry| entry.relative_path.clone())
        .collect();
    let index = match DirectoryProjectIndex::prepare(&paths) {
        Ok(index) => index,
        Err(error) => {
            done(Err(error));
            return;
        }
    };
    let html_entry_index = index.html_entry_index;
    let session = Rc::new(RefCell::new(DirectoryImportSession {
        files,
        index,
        loaded_entries: HashMap::new(),
        is_active,
        done: Some(done),
    }));
    let session2 = session.clone();
    load_entry(
        session,
        html_entry_index,
        Box::new(move |result| match result {
            Ok(()) => import_until_resolved(session2),
            Err(error) => finish(&session2, Err(error)),
        }),
    );
}

fn load_entry(session: SessionRef, index: usize, done: EntryLoaded) {
    if abandon_if_stale(&session) {
        return;
    }
    if session.borrow().loaded_entries.contains_key(&index) {
        done(Ok(()));
        return;
    }
    let (file, path) = {
        let session = session.borrow();
        let Some(entry) = session.index.entries.get(index) else {
            done(Err(DirectoryImportError::EntryIndexInvalid(index)));
            return;
        };
        let Some(source) = session.files.get(entry.source_index) else {
            done(Err(DirectoryImportError::SourceUnavailable(
                entry.relative_path.clone(),
            )));
            return;
        };
        (source.file.clone(), entry.relative_path.clone())
    };
    let size = file.size();
    if !size.is_finite() || !(0.0..=MAX_SAFE_FILE_SIZE).contains(&size) || size.fract() != 0.0 {
        done(Err(DirectoryImportError::InvalidFileSize(path)));
        return;
    }

    let session2 = session.clone();
    read_blob_range(
        file.unchecked_ref(),
        0,
        size as u64,
        Box::new(move |result| {
            if abandon_if_stale(&session2) {
                return;
            }
            match result {
                Ok(bytes) => {
                    session2.borrow_mut().loaded_entries.insert(index, bytes);
                    done(Ok(()));
                }
                Err(error) => done(Err(DirectoryImportError::ReadFailed {
                    path,
                    source: error,
                })),
            }
        }),
    );
}

fn import_until_resolved(session: SessionRef) {
    if abandon_if_stale(&session) {
        return;
    }
    let (mut imported, missing) =
        match probe_import(&session.borrow().index, &session.borrow().loaded_entries) {
            Ok(result) => result,
            Err(error) => {
                finish(&session, Err(error));
                return;
            }
        };
    if !missing.is_empty() {
        load_entry_queue(session, missing.into());
        return;
    }
    if imported.document.children.is_empty() {
        finish(&session, Err(DirectoryImportError::NoImportableContent));
        return;
    }

    let (entry_path, html_entry_count) = {
        let session = session.borrow();
        (
            session.index.entries[session.index.html_entry_index]
                .relative_path
                .clone(),
            session.index.html_entry_count,
        )
    };
    apply_entry_name_fallback(&mut imported, &entry_path);
    if html_entry_count > 1 {
        imported.push_warning(op_html::ImportWarning::MultipleHtmlEntries {
            count: html_entry_count,
            entry: entry_path.clone(),
        });
    }
    finish(
        &session,
        Ok(IngestedDoc::from_html(
            op_editor_core::EditorState::from_document(imported.document),
            imported.warnings,
            &imported.diagnostics,
        )),
    );
}

fn probe_import(
    index: &DirectoryProjectIndex,
    loaded_entries: &HashMap<usize, Vec<u8>>,
) -> Result<(op_html::HtmlDocumentResult, Vec<usize>), DirectoryImportError> {
    let entry = &index.entries[index.html_entry_index];
    let html_bytes = loaded_entries
        .get(&index.html_entry_index)
        .ok_or_else(|| DirectoryImportError::HtmlEntryUnavailable(entry.relative_path.clone()))?;
    let html = op_html::html_encoding::decode_html_bytes(html_bytes);
    let requested = RefCell::new(BTreeSet::new());
    let fetcher = |url: &str| {
        let resource_index = index.entry_for_resource_url(url)?;
        match loaded_entries.get(&resource_index) {
            Some(bytes) => Some(bytes.clone()),
            None => {
                requested.borrow_mut().insert(resource_index);
                None
            }
        }
    };
    let options = op_html::HtmlImportOptions {
        base_url: Some(entry.virtual_url.clone()),
        ..Default::default()
    };
    let imported = op_html::import_html_document(&html, &options, Some(&fetcher), None);
    Ok((imported, requested.into_inner().into_iter().collect()))
}

fn load_entry_queue(session: SessionRef, mut queue: VecDeque<usize>) {
    if abandon_if_stale(&session) {
        return;
    }
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

fn finish(session: &SessionRef, result: DirectoryImportResult) {
    if abandon_if_stale(session) {
        return;
    }
    let done = session.borrow_mut().done.take();
    if let Some(done) = done {
        done(result);
    }
}

fn apply_entry_name_fallback(imported: &mut op_html::HtmlDocumentResult, entry_path: &str) {
    if imported.document.name.as_deref() != Some("HTML Import") {
        return;
    }
    let stem = entry_file_stem(entry_path).to_string();
    imported.document.name = Some(stem.clone());
    if let Some(jian_ops_schema::node::PenNode::Frame(root)) =
        imported.document.children.first_mut()
    {
        if root.base.name.as_deref() == Some("HTML Import") {
            root.base.name = Some(stem);
        }
    }
}

fn normalize_relative_path(path: &str) -> Result<String, DirectoryImportError> {
    if path.is_empty() || path.contains('\0') || path.starts_with(['/', '\\']) {
        return Err(DirectoryImportError::InvalidPath(path.to_string()));
    }
    let unified = path.replace('\\', "/");
    if unified
        .as_bytes()
        .get(1)
        .is_some_and(|byte| *byte == b':' && unified.as_bytes()[0].is_ascii_alphabetic())
    {
        return Err(DirectoryImportError::InvalidPath(path.to_string()));
    }
    let mut components = Vec::new();
    for component in unified.split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(DirectoryImportError::InvalidPath(path.to_string())),
            value => components.push(value),
        }
    }
    if components.is_empty() {
        return Err(DirectoryImportError::InvalidPath(path.to_string()));
    }
    Ok(components.join("/"))
}

fn is_ignored_metadata_path(path: &str) -> bool {
    let components: Vec<_> = path.split('/').collect();
    components
        .iter()
        .any(|component| component.eq_ignore_ascii_case("__MACOSX"))
        || components
            .last()
            .is_some_and(|name| name.eq_ignore_ascii_case(".DS_Store"))
}

fn strip_common_root(entries: &mut [(usize, String)]) {
    loop {
        let Some(root) = entries
            .first()
            .and_then(|(_, path)| path.split_once('/').map(|(root, _)| root.to_string()))
        else {
            return;
        };
        if !entries.iter().all(|(_, path)| {
            path.split_once('/')
                .is_some_and(|(candidate, _)| candidate == root)
        }) {
            return;
        }
        let prefix_len = root.len() + 1;
        for (_, path) in &mut *entries {
            path.drain(..prefix_len);
        }
    }
}

fn virtual_project_url(path: &str) -> String {
    let mut url = Url::parse(op_html::VIRTUAL_PROJECT_ORIGIN)
        .expect("the shared virtual project origin is valid");
    let mut segments = url
        .path_segments_mut()
        .expect("the virtual project origin is hierarchical");
    segments.pop_if_empty();
    segments.extend(path.split('/'));
    drop(segments);
    url.to_string()
}

fn canonical_virtual_resource_url(requested: &str) -> Option<String> {
    let mut url = Url::parse(requested).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("openpencil.local")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

fn is_html_path(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
    })
}

fn entry_file_stem(path: &str) -> &str {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn index_strips_common_root_and_maps_encoded_resource_urls() {
        let index = DirectoryProjectIndex::prepare(&paths(&[
            "bundle/index.html",
            "bundle/assets/hero image.png",
            "bundle/unused-500mb.bin",
        ]))
        .unwrap();

        assert_eq!(
            index.entries[index.html_entry_index].relative_path,
            "index.html"
        );
        assert_eq!(
            index.entry_for_resource_url(
                "https://openpencil.local/assets/hero%20image.png?cache=1#preview"
            ),
            Some(1)
        );
        assert_eq!(
            index.entry_for_resource_url("https://example.test/assets/hero%20image.png"),
            None
        );
    }

    #[test]
    fn probe_requests_referenced_files_but_never_unreferenced_large_entries() {
        let index = DirectoryProjectIndex::prepare(&paths(&[
            "site/index.html",
            "site/styles/main.css",
            "site/images/hero.png",
            "site/unreferenced-500mb.bin",
        ]))
        .unwrap();
        let mut loaded = HashMap::from([(
            index.html_entry_index,
            br#"<link rel="stylesheet" href="styles/main.css"><div class="hero"></div>"#.to_vec(),
        )]);

        let (_, first_missing) = probe_import(&index, &loaded).unwrap();
        assert_eq!(first_missing, vec![1]);

        loaded.insert(
            1,
            b".hero{width:20px;height:20px;background-image:url('../images/hero.png')}".to_vec(),
        );
        let (_, second_missing) = probe_import(&index, &loaded).unwrap();
        assert_eq!(second_missing, vec![2]);
        assert!(!first_missing.contains(&3));
        assert!(!second_missing.contains(&3));
    }

    #[test]
    fn preparation_uses_shared_entry_selection_and_rejects_traversal() {
        let index = DirectoryProjectIndex::prepare(&paths(&[
            "project/landing.html",
            "project/node_modules/demo/index.html",
        ]))
        .unwrap();
        assert_eq!(
            index.entries[index.html_entry_index].relative_path,
            "landing.html"
        );
        assert!(DirectoryProjectIndex::prepare(&paths(&["../index.html"])).is_err());
    }
}
