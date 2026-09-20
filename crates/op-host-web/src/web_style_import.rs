//! Browser arm of the Styles tab's `DESIGN.md` file import.
//!
//! The widget layer only raises a request — `pending_file_pick` on the Asset
//! Center's import state — because opening a file dialog is a host capability.
//! This drains it into a hidden `<input type=file>`, exactly the channel every
//! other browser-side pick uses.
//!
//! What comes back is *text and nothing else*. Parsing it, allocating an id,
//! registering the guide, pinning it, closing the box: all of that is the
//! shared flow the paste route already runs, so a guide picked as a file and
//! the same guide pasted into the box are the same guide. The browser has no
//! disk, so the persist request the flow raises is dropped here as it is on
//! the paste route — the documented M1 boundary, where an imported style lasts
//! the session.

use std::cell::RefCell;
use std::rc::Rc;

use crate::repaint_ctx::RepaintContext;

type InnerRc<C> = Rc<RefCell<C>>;

/// File extensions the picker offers. Matches the desktop dialog's filter.
const ACCEPT: &str = ".md,.markdown,text/markdown";

/// Drain a pending style-import file pick into a browser file dialog.
pub(crate) fn drain_pending_style_import<C: RepaintContext + 'static>(inner: &InnerRc<C>) {
    let requested = {
        let Ok(mut b) = inner.try_borrow_mut() else {
            return;
        };
        b.host_mut()
            .editor_state_mut()
            .editor_ui
            .scene_template_center
            .take_pending_style_import_file()
    };
    if !requested {
        return;
    }
    let inner = inner.clone();
    crate::dom_io::open_file_picker(
        ACCEPT,
        Box::new(move |file| {
            let name = file.name();
            let inner2 = inner.clone();
            crate::dom_io::read_file(
                file,
                crate::dom_io::ReadMode::Text,
                Box::new(move |value| match value.as_string() {
                    // `readAsText` yields a string for anything it can decode
                    // and nothing for what it cannot — a binary file picked
                    // past the accept filter lands here, not in the parser.
                    Some(markdown) => apply_picked_file(&inner2, &markdown, &name),
                    None => {
                        web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format!(
                            "[style-import] {name}: file read produced no text"
                        )));
                        report_unreadable(&inner2);
                    }
                }),
            );
        }),
    );
}

fn apply_picked_file<C: RepaintContext + 'static>(inner: &InnerRc<C>, markdown: &str, name: &str) {
    let Ok(mut b) = inner.try_borrow_mut() else {
        return;
    };
    // The file name is a fallback, used only when the document names nothing:
    // a guide called `untitled` is worse than one called after the file the
    // user went and found.
    b.host_mut()
        .import_picked_style_guide(markdown, crate::file_actions::file_stem(name));
    let _ = b.repaint();
}

fn report_unreadable<C: RepaintContext + 'static>(inner: &InnerRc<C>) {
    let Ok(mut b) = inner.try_borrow_mut() else {
        return;
    };
    b.host_mut().report_unreadable_style_import();
    let _ = b.repaint();
}
