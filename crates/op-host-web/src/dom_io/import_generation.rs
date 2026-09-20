//! Per-mount generation guard shared by asynchronous document imports.

use op_editor_core::figma_import_state::ImportSource;

use super::InnerRc;
use crate::repaint_ctx::RepaintContext;
use crate::widget_host::WidgetHost;

pub(super) type IsDocumentImportActive = Box<dyn Fn() -> bool>;

fn next_generation(current: u64) -> u64 {
    current.wrapping_add(1).max(1)
}

fn advance_host_document_generation(host: &mut WidgetHost) -> u64 {
    let generation = next_generation(host.document_import_generation);
    host.document_import_generation = generation;
    generation
}

fn begin_host_document_import(host: &mut WidgetHost, source: ImportSource) -> u64 {
    let generation = advance_host_document_generation(host);
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.import_source = source;
    ui.figma_import_in_progress = true;
    generation
}

fn begin_host_document_replacement(host: &mut WidgetHost) -> u64 {
    let generation = advance_host_document_generation(host);
    let ui = &mut host.editor_state_mut().editor_ui;
    ui.figma_import_in_progress = false;
    ui.figma_import_open = false;
    ui.figma_import_hover = None;
    generation
}

fn host_document_import_is_current(
    host: &WidgetHost,
    generation: u64,
    source: ImportSource,
) -> bool {
    let ui = &host.editor_state().editor_ui;
    host.document_import_generation == generation
        && ui.figma_import_in_progress
        && ui.import_source == source
}

fn host_document_replacement_is_current(host: &WidgetHost, generation: u64) -> bool {
    host.document_import_generation == generation
}

fn clear_host_document_import_if_owned(
    host: &mut WidgetHost,
    generation: u64,
    source: ImportSource,
) -> bool {
    if !host_document_import_is_current(host, generation, source) {
        return false;
    }
    host.editor_state_mut().editor_ui.figma_import_in_progress = false;
    true
}

pub(super) fn begin_document_import<C: RepaintContext + 'static>(
    inner: &InnerRc<C>,
    source: ImportSource,
) -> u64 {
    let generation = {
        let mut shell = inner.borrow_mut();
        let generation = begin_host_document_import(shell.host_mut(), source);
        shell.host_mut().mark_editor_state_dirty();
        let _ = shell.repaint();
        generation
    };
    // Advance ownership before cancellation. The canceled Worker's deferred
    // callback then observes a stale generation and cannot start its fallback.
    crate::figma_temp_bridge::cancel_all();
    generation
}

pub(super) fn begin_document_replacement<C: RepaintContext + 'static>(inner: &InnerRc<C>) -> u64 {
    let generation = {
        let mut shell = inner.borrow_mut();
        let generation = begin_host_document_replacement(shell.host_mut());
        shell.host_mut().mark_editor_state_dirty();
        let _ = shell.repaint();
        generation
    };
    crate::figma_temp_bridge::cancel_all();
    generation
}

pub(super) fn document_import_is_current<C: RepaintContext + 'static>(
    inner: &InnerRc<C>,
    generation: u64,
    source: ImportSource,
) -> bool {
    host_document_import_is_current(inner.borrow().host(), generation, source)
}

pub(super) fn document_replacement_is_current<C: RepaintContext + 'static>(
    inner: &InnerRc<C>,
    generation: u64,
) -> bool {
    host_document_replacement_is_current(inner.borrow().host(), generation)
}

pub(super) fn document_import_activity<C: RepaintContext + 'static>(
    inner: &InnerRc<C>,
    generation: u64,
    source: ImportSource,
) -> IsDocumentImportActive {
    let inner = inner.clone();
    Box::new(move || document_import_is_current(&inner, generation, source))
}

pub(super) fn clear_document_import_if_owned<C: RepaintContext + 'static>(
    inner: &InnerRc<C>,
    generation: u64,
    source: ImportSource,
) -> bool {
    let mut shell = inner.borrow_mut();
    if !clear_host_document_import_if_owned(shell.host_mut(), generation, source) {
        return false;
    }
    shell.host_mut().mark_editor_state_dirty();
    let _ = shell.repaint();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generations_are_per_mount_and_cross_source_cancellation_is_owned() {
        let mut left = WidgetHost::new();
        let mut right = WidgetHost::new();

        let left_figma = begin_host_document_import(&mut left, ImportSource::Figma);
        let right_html = begin_host_document_import(&mut right, ImportSource::Html);
        assert!(host_document_import_is_current(
            &left,
            left_figma,
            ImportSource::Figma
        ));
        assert!(host_document_import_is_current(
            &right,
            right_html,
            ImportSource::Html
        ));

        let left_html = begin_host_document_import(&mut left, ImportSource::Html);
        assert!(!host_document_import_is_current(
            &left,
            left_figma,
            ImportSource::Figma
        ));
        assert!(host_document_import_is_current(
            &left,
            left_html,
            ImportSource::Html
        ));
        assert!(host_document_import_is_current(
            &right,
            right_html,
            ImportSource::Html
        ));

        assert!(!clear_host_document_import_if_owned(
            &mut left,
            left_figma,
            ImportSource::Figma
        ));
        assert!(left.editor_state().editor_ui.figma_import_in_progress);
        assert!(clear_host_document_import_if_owned(
            &mut left,
            left_html,
            ImportSource::Html
        ));
        assert!(!left.editor_state().editor_ui.figma_import_in_progress);
        assert!(right.editor_state().editor_ui.figma_import_in_progress);
    }

    #[test]
    fn ordinary_open_and_import_cancel_each_other_in_both_directions() {
        let mut host = WidgetHost::new();

        let html = begin_host_document_import(&mut host, ImportSource::Html);
        let opened = begin_host_document_replacement(&mut host);
        assert!(!host_document_import_is_current(
            &host,
            html,
            ImportSource::Html
        ));
        assert!(host_document_replacement_is_current(&host, opened));
        assert!(!host.editor_state().editor_ui.figma_import_in_progress);

        let figma = begin_host_document_import(&mut host, ImportSource::Figma);
        assert!(!host_document_replacement_is_current(&host, opened));
        assert!(host_document_import_is_current(
            &host,
            figma,
            ImportSource::Figma
        ));
    }
}
