//! Desktop arm of the Styles tab's `DESIGN.md` import.
//!
//! The Asset Center never touches the filesystem: it raises requests and this
//! drains them, the same split the template-open path uses. Three of them, and
//! they are genuinely different jobs — open a file dialog, write a guide the
//! shared flow already registered, delete one it already forgot.
//!
//! Persist and delete are deliberately *after the fact*. Memory is the source
//! of truth for what the catalogue contains, so the card list, the pin, and
//! the grid are all correct the instant the user acts, whether or not the disk
//! cooperates. A write that fails costs the guide at next launch and says so;
//! it does not make the button appear broken now.

use op_editor_ui::widgets::press_flow;
use op_host_native::widget_host::WidgetHostNative;

use crate::{frame, user_style_store, DesktopApp};

/// Drain every pending style-import request. Returns whether anything changed
/// on screen.
pub(crate) fn drain_pending_style_import(app: &mut DesktopApp) -> bool {
    let mut changed = drain_pending_file_pick(app);
    changed |= drain_pending_persist(&mut app.host);
    changed |= drain_pending_delete(&mut app.host);
    changed
}

/// Ask for a `.md` and import it.
///
/// The host's whole job here is *reading bytes*. Everything that decides what
/// an imported style guide is — parsing, id allocation, registering it,
/// pinning it, closing the box — belongs to the shared flow the paste route
/// already goes through, so the two routes cannot drift into producing
/// different styles from the same document. That was not true before: this
/// arm used to run its own parse-and-write path, and the paste box ran
/// another.
fn drain_pending_file_pick(app: &mut DesktopApp) -> bool {
    if !app
        .host
        .editor_state_mut()
        .editor_ui
        .scene_template_center
        .take_pending_style_import_file()
    {
        return false;
    }
    // The press that raised the request has not been painted yet — this drain
    // runs at the top of the redraw, before the frame. `rfd::FileDialog` then
    // blocks the loop, so without a synchronous frame here the button the user
    // just clicked stays un-pressed underneath the dialog for its whole life.
    if let (Some(ctx), Some(backend)) = (app.ctx.as_mut(), app.backend.as_mut()) {
        frame::paint(
            ctx,
            backend,
            &mut app.host,
            app.viewport_width,
            app.viewport_height,
            app.dpi,
        );
    }
    let locale = app.host.editor_state().editor_ui.locale;
    let picked = rfd::FileDialog::new()
        .set_title(op_i18n::translate(locale, "assetCenter.style.importTitle"))
        .add_filter("Markdown", &["md", "markdown"])
        .pick_file();
    let Some(path) = picked else {
        return true;
    };
    let state = app.host.editor_state_mut();
    match std::fs::read_to_string(&path) {
        Ok(raw) => {
            let fallback = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default();
            press_flow::import_style_guide_text(state, &raw, fallback);
        }
        Err(error) => {
            // Not readable as UTF-8 text, or not readable at all. Both are the
            // same thing to a user holding a file that is not a `DESIGN.md`,
            // and the box says so where they are looking.
            eprintln!("[styles] {}: could not be read: {error}", path.display());
            press_flow::fail_style_import_unreadable(state);
        }
    }
    app.host.mark_editor_state_dirty();
    true
}

/// Write guides the shared flow registered but could not store.
fn drain_pending_persist(host: &mut WidgetHostNative) -> bool {
    let ids = host
        .editor_state_mut()
        .editor_ui
        .scene_template_center
        .take_pending_style_persist();
    if ids.is_empty() {
        return false;
    }
    for id in ids {
        if let Err(error) = user_style_store::persist_user_style_guide(&id) {
            // Reported, not surfaced as a dialog: the guide is live and
            // usable this session, and the only casualty is that it will not
            // be there after a restart.
            eprintln!("[styles] {id}: could not be written: {error}");
        }
    }
    false
}

/// Remove the files of guides the shared flow already forgot.
fn drain_pending_delete(host: &mut WidgetHostNative) -> bool {
    let ids = host
        .editor_state_mut()
        .editor_ui
        .scene_template_center
        .take_pending_style_delete();
    if ids.is_empty() {
        return false;
    }
    for id in ids {
        if let Err(error) = user_style_store::delete_user_style_guide(&id) {
            eprintln!("[styles] {id}: could not be deleted: {error}");
        }
    }
    false
}
