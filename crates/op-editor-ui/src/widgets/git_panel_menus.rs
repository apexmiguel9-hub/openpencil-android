//! Ready-state header popovers for [`GitPanel`] — the branch-picker
//! dropdown (`⎇ <branch> ▾`) and the overflow `…` menu.
//!
//! A port of the TS `GitPanelBranchPicker` (list mode) + the
//! `GitPanelHeader` overflow popover. Split out of `git_panel_ready.rs`
//! to keep both files under the repo's 800-line cap.
//!
//! These paint as overlays ON TOP of the ready view and are hit-tested
//! BEFORE it (so an open popover captures the click). Geometry is
//! shared between paint + hit-test through the `*_rects` helpers so the
//! pure-geometry hit-test agrees with paint.

use crate::widgets::button::paint_button_feedback_wash;
use crate::widgets::git_panel::{truncate, GitPanel, GitPanelHit, PAD};
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect};
use jian_widgets::components::menu::MenuHit;
use op_editor_core::{GitBranchPickerMode, GitOverflowView};

/// Dropdown row height (TS menu item ≈ 28 px).
const MENU_ROW_H: f32 = 28.0;
/// Inner padding around a popover's row stack.
const MENU_PAD: f32 = 4.0;
/// Most branches the picker lists (matches the classic section cap).
const MENU_MAX_BRANCHES: usize = 8;
/// Branch-name truncation inside the dropdown.
const BRANCH_NAME_MAX: usize = 26;
/// Fixed branch-picker popover width (TS `w-[280px]`), clamped to the
/// available panel column.
const PICKER_W: f32 = 280.0;
/// "分支" section-header band height (TS `px-2 py-1` ≈ 22 px).
const PICKER_HEADER_H: f32 = 22.0;
/// Two-line branch row (name + last-commit subtitle) ≈ 40 px.
const PICKER_ROW_H: f32 = 40.0;
/// Divider band between the branch list and the footer actions.
const PICKER_DIVIDER_H: f32 = 9.0;
/// Footer band holding the "新建分支" / "合并分支" actions (TS justify-end).
const PICKER_FOOTER_H: f32 = 30.0;
/// Create-mode body height — name input (30) + gap (8) + submit (24) + pad.
const PICKER_CREATE_H: f32 = 70.0;
/// Overflow-menu width (TS `w-56` ≈ 224 px, clamped to the panel).
const OVERFLOW_W: f32 = 224.0;
/// Remote-settings subview width (TS `w-[300px]`, clamped).
const RS_W: f32 = 280.0;
/// Inner padding inside the remote-settings subview.
const RS_PAD: f32 = 10.0;
/// Input / button row height inside the subview.
const RS_ROW_H: f32 = 28.0;
/// The `‹ Back` header-row height.
const RS_BACK_H: f32 = 24.0;
/// Width of the "Set" / "Login" buttons at an input row's right edge.
const RS_BTN_W: f32 = 52.0;
/// Gap between an input and its trailing button.
const RS_GAP: f32 = 8.0;
/// Height of the ahead/behind + credentials section appended below the
/// remote-URL input (divider + ahead/behind row + divider + credentials row,
/// plus bottom padding).
const RS_SECTION: f32 = 70.0;

/// One overflow-menu entry — an icon, a label key, and the
/// [`GitPanelHit`] it dispatches.
struct OverflowItem {
    icon: Icon,
    label_key: &'static str,
    hit: GitPanelHit,
    /// A `›` submenu affordance (the entry opens a subview).
    submenu: bool,
    /// A divider band painted below this row (TS `<Separator>`).
    divider_after: bool,
}

/// Height of a divider band between overflow-menu groups (TS
/// `<Separator className="my-1">` ≈ 1px line + 8px margins).
const OVERFLOW_DIVIDER_H: f32 = 9.0;

mod branch_picker;
mod overflow;
mod remote_settings;

impl GitPanel<'_> {
    /// A simple popover input box — rounded field + draft / placeholder
    /// text + a blink-free caret bar when focused.
    pub(super) fn paint_menu_input(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        input: &jian_core::text_input::TextInputState,
        placeholder: &str,
        focused: bool,
    ) {
        let t = self.theme;
        cx.backend.fill_round_rect(rect, 6.0, t.muted);
        let border = if focused {
            alpha(t.primary, 0.50)
        } else {
            alpha(t.border, 0.70)
        };
        cx.backend.stroke_round_rect(rect, 6.0, border, 1.0);
        self.paint_text_input_view(cx, rect, input, placeholder, focused, 11.0, 8.0);
    }
}

/// A colour at `factor` of its current alpha (Tailwind `/NN`).
fn alpha(c: Color, factor: f32) -> Color {
    crate::util::alpha(c, factor)
}

/// Rough rendered width of a 12px label without a backend — CJK glyphs
/// count as ~13px, ASCII as ~6.5px. Lets the footer button hit rects
/// match paint without measuring text.
fn label_px(s: &str) -> f32 {
    s.chars()
        .map(|c| if (c as u32) >= 0x1100 { 13.0 } else { 6.5 })
        .sum()
}
