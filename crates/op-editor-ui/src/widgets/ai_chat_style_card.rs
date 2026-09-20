//! The pinned-style chip's hover card — the rest of the answer.
//!
//! The chip states two things: which style is in force, and whether anything
//! could be read out of it. That is the right amount for a row wedged above the
//! input, and it leaves the question a user asks when they actually stop on it
//! unanswered: *which* of my styles is this one? A name and four unlabelled
//! swatches cannot separate an imported `DESIGN.md` from a shipped guide of a
//! similar palette — and believing a pin points at the wrong catalogue is how
//! someone concludes their import "didn't work" when it did.
//!
//! So the card leads with provenance. Everything else on it — the wider band
//! with its hex values, the families, the guide's own opening sentence — is
//! there to make the pin recognisable at a glance, and every row of it is
//! omitted rather than faked when the file did not state it. A card that
//! invented a palette would be a new way to be wrong about exactly the thing
//! the chip was added to make visible.
//!
//! Three rules, borrowed from the top bar's tooltips because a user has
//! already learned them there:
//!
//! 1. **Dwell.** Nothing appears until the cursor has rested on the chip for
//!    [`STYLE_CARD_DWELL_MS`]; passing over it on the way to the input is
//!    silent.
//! 2. **A scheduled repaint.** The dwell expiring is not an input event, so
//!    [`next_deadline_ms`] hands the due instant to the hosts' animation
//!    scheduler.
//! 3. **Nothing but a read-out.** The card takes no clicks and is painted last,
//!    above the input block it hangs over, but always ABOVE the chip — the ✕ it
//!    would otherwise cover is the only control on that row.

use op_ai_skills::style_guide::StyleGuideCard;
use op_editor_core::editor_ui_state::EditorUiState;
use op_editor_core::EditorState;
use op_util::hex_color;

use crate::theme::Theme;
use crate::widgets::text_metrics;
use crate::widgets::tooltip::{TOOLTIP_ANCHOR_GAP, TOOLTIP_RADIUS};
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

/// How long the cursor must rest on the chip before the card appears. The
/// top bar's dwell, deliberately: two different waits for the same gesture in
/// one window reads as lag rather than as design.
pub const STYLE_CARD_DWELL_MS: u64 = crate::widgets::top_bar_tooltip::TOOLTIP_DWELL_MS;

/// Widest the card is allowed to be. Past this the description stops reading
/// as a caption on the chip and starts reading as a paragraph in the panel.
const CARD_MAX_W: f32 = 268.0;
/// Narrowest usable card. Below it the swatch grid cannot hold a hex label,
/// and a card that shows colours it cannot name is the chip again.
const CARD_MIN_W: f32 = 150.0;
/// Inner padding on all four sides.
const CARD_PAD: f32 = 10.0;
/// Distance the card keeps from the panel's own edges.
const PANEL_INSET: f32 = 4.0;

const TITLE_FONT: f32 = 11.5;
const TITLE_LINE_H: f32 = 16.0;
const BADGE_FONT: f32 = 9.5;
const BADGE_H: f32 = 15.0;
const BADGE_PAD_X: f32 = 6.0;
const BODY_FONT: f32 = 10.0;
const BODY_LINE_H: f32 = 13.0;
const HEX_FONT: f32 = 8.0;
const HEX_LINE_H: f32 = 10.0;

const SWATCH_SIZE: f32 = 22.0;
const SWATCH_RADIUS: f32 = 4.0;
const SWATCH_GAP: f32 = 6.0;
/// Swatches per grid row. Four keeps every cell wide enough for a `#RRGGBB`
/// label at [`HEX_FONT`], which is what makes the band more than a wider
/// version of the chip's.
const SWATCHES_PER_ROW: usize = 4;

/// Gap between stacked blocks inside the card.
const BLOCK_GAP: f32 = 8.0;
/// Longest the description may run before it is cut.
const DESCRIPTION_LINES: usize = 2;

/// Which catalogue the style in force came from.
///
/// This is the fact the chip cannot carry and the card exists for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StyleCardSource {
    /// A guide baked into this build.
    Builtin,
    /// A `DESIGN.md` the user imported.
    Imported,
    /// The document's own design.md, which outranks any pin.
    DocumentDesignMd,
}

impl StyleCardSource {
    /// The i18n key naming this source.
    fn label_key(self) -> &'static str {
        match self {
            StyleCardSource::Builtin => "ai.styleCard.builtin",
            StyleCardSource::Imported => "ai.styleCard.imported",
            StyleCardSource::DocumentDesignMd => "ai.styleCard.documentDesignMd",
        }
    }
}

/// One colour on the card's band, with the value it was stated as.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StyleCardSwatch {
    pub(crate) color: Color,
    /// The hex exactly as the guide wrote it, upper-cased for the label.
    pub(crate) hex: String,
}

/// Everything the card shows, resolved off the paint path.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StyleCard {
    pub(crate) name: String,
    pub(crate) source: StyleCardSource,
    /// Empty means the file stated no colours anything could read — shown as
    /// no band, the same way the chip shows it.
    pub(crate) swatches: Vec<StyleCardSwatch>,
    /// Families, already joined for display. `None` when none were stated.
    pub(crate) fonts: Option<String>,
    /// The guide's own opening sentence, or the design.md's stated theme.
    pub(crate) description: Option<String>,
}

impl StyleCard {
    /// The card for the current editor state, or `None` when none is due.
    ///
    /// Returning `None` before the dwell elapses is what keeps this off the
    /// paint path: no hover, no resolve, no allocation. Precedence mirrors
    /// `ai_chat_style_receipt::StyleReceipt::for_state` exactly — a card that
    /// described a style the chip is not naming would be worse than none.
    pub(crate) fn for_state(state: &EditorState, now_ms: u64) -> Option<Self> {
        if !due(&state.editor_ui, now_ms) {
            return None;
        }
        if let Some(spec) = state.doc.design_md.as_ref() {
            return Some(Self::from_design_md(spec));
        }
        let pinned = state.editor_ui.pinned_style_guide.as_deref()?;
        let card = op_ai_skills::style_guide::style_guide_card(pinned)?;
        Some(Self::from_guide(&card))
    }

    fn from_guide(card: &StyleGuideCard) -> Self {
        Self {
            name: card.name.clone(),
            source: if card.is_user {
                StyleCardSource::Imported
            } else {
                StyleCardSource::Builtin
            },
            swatches: card.swatches.iter().filter_map(|hex| swatch(hex)).collect(),
            fonts: join_fonts(card.display_font.as_deref(), card.body_font.as_deref()),
            description: card.description.clone(),
        }
    }

    /// The document's own design.md, which arrives already structured — its
    /// colours and typography are parsed fields, not markdown this layer would
    /// have to read.
    fn from_design_md(spec: &jian_ops_schema::DesignMdSpec) -> Self {
        let typography = spec.typography.as_ref();
        Self {
            // The brief names itself when it can. "design.md" is the file, not
            // the style, and the chip already says the file.
            name: non_empty(spec.project_name.as_deref())
                .unwrap_or_else(|| "design.md".to_string()),
            source: StyleCardSource::DocumentDesignMd,
            swatches: spec
                .color_palette
                .iter()
                .flatten()
                .take(op_ai_skills::style_guide::STYLE_CARD_SWATCH_CAP)
                .filter_map(|color| swatch(&color.hex))
                .collect(),
            fonts: typography.and_then(|type_| {
                join_fonts(
                    type_.font_family.as_deref().or(type_.headings.as_deref()),
                    type_.body.as_deref(),
                )
            }),
            description: non_empty(spec.visual_theme.as_deref()).map(|theme| first_line(&theme)),
        }
    }
}

/// Whether a card is due to be on screen at `now_ms`.
fn due(ui: &EditorUiState, now_ms: u64) -> bool {
    ui.chat_style_chip_hover_since_ms
        .is_some_and(|since| now_ms >= since.saturating_add(STYLE_CARD_DWELL_MS))
}

/// The instant a pending card becomes due, for the hosts' animation
/// scheduler. `None` once it is showing (or was never coming) — the card needs
/// exactly one wake-up, not a running clock.
pub fn next_deadline_ms(ui: &EditorUiState, now_ms: u64) -> Option<u64> {
    let due = ui
        .chat_style_chip_hover_since_ms?
        .saturating_add(STYLE_CARD_DWELL_MS);
    (now_ms < due).then_some(due)
}

fn swatch(raw: &str) -> Option<StyleCardSwatch> {
    let [r, g, b, a] = hex_color::parse_hex_rgba8(raw, hex_color::HexOptions::LENIENT)?;
    Some(StyleCardSwatch {
        color: Color::rgba_u8(r, g, b, a as f32 / 255.0),
        hex: raw.trim().to_ascii_uppercase(),
    })
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// The first line of a multi-line field, so a whole section body does not
/// arrive on a two-line caption.
fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

/// `display / body`, or whichever single family was stated. Identical
/// families collapse: "Inter / Inter" says less than "Inter".
fn join_fonts(display: Option<&str>, body: Option<&str>) -> Option<String> {
    let display = non_empty(display);
    let body = non_empty(body);
    match (display, body) {
        (Some(display), Some(body)) if display.eq_ignore_ascii_case(&body) => Some(display),
        (Some(display), Some(body)) => Some(format!("{display} / {body}")),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

// ─── Layout ────────────────────────────────────────────────────────────

/// The card resolved against a chip and a panel: every rect and every string
/// exactly as they will be painted.
///
/// Paint and the placement tests read this same value, so an assertion about
/// where the card sits is an assertion about where it is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StyleCardLayout {
    pub(crate) rect: Rect,
    pub(crate) title: String,
    pub(crate) badge: Rect,
    pub(crate) badge_label: String,
    /// `(cell, colour, hex label)` per painted swatch.
    pub(crate) swatches: Vec<(Rect, Color, String)>,
    pub(crate) fonts: Option<String>,
    pub(crate) description: Vec<String>,
}

/// Place `card` above `chip`, inside `panel`.
///
/// Anchored to the chip's left edge rather than centred on it: the chip starts
/// at the input block's left edge, so a centred card would hang off the panel
/// on every narrow panel and be clamped back to the same place anyway.
///
/// The card never grows downward past the chip. When the space above cannot
/// hold everything, rows are dropped from the least load-bearing end — extra
/// swatch rows, then the description, then the families — so what survives on
/// a short panel is always the provenance the card exists to state.
pub(crate) fn layout_style_card(
    cx: &mut PaintCx<'_>,
    card: &StyleCard,
    chip: Rect,
    panel: Rect,
    locale: op_editor_core::Locale,
) -> StyleCardLayout {
    let width = (panel.size.x - PANEL_INSET * 2.0).clamp(CARD_MIN_W, CARD_MAX_W);
    let inner = (width - CARD_PAD * 2.0).max(0.0);

    let badge_label = op_i18n::translate(locale, card.source.label_key()).to_string();
    let badge_w = (text_metrics::measure_chrome(cx.backend, &badge_label, BADGE_FONT)
        + BADGE_PAD_X * 2.0)
        .min(inner);
    let title = text_metrics::fit_chrome(cx.backend, &card.name, inner, TITLE_FONT);

    // Everything the card would like to show, before the height budget.
    let mut swatch_rows = card.swatches.len().div_ceil(SWATCHES_PER_ROW);
    let mut description = card
        .description
        .as_deref()
        .map(|text| wrap_lines(cx, text, inner, BODY_FONT, DESCRIPTION_LINES))
        .unwrap_or_default();
    let mut fonts = card.fonts.clone();

    // Space above the chip, which is the ceiling: the card must not cover the
    // one control on the chip row.
    let ceiling = (chip.origin.y - TOOLTIP_ANCHOR_GAP - (panel.origin.y + PANEL_INSET)).max(0.0);
    let height = |rows: usize, description: &[String], fonts: &Option<String>| {
        content_height(rows, description.len(), fonts.is_some())
    };
    if height(swatch_rows, &description, &fonts) > ceiling && swatch_rows > 1 {
        swatch_rows = 1;
    }
    if height(swatch_rows, &description, &fonts) > ceiling {
        description.clear();
    }
    if height(swatch_rows, &description, &fonts) > ceiling {
        fonts = None;
    }
    if height(swatch_rows, &description, &fonts) > ceiling {
        swatch_rows = 0;
    }
    let card_h = height(swatch_rows, &description, &fonts).min(ceiling.max(0.0));

    let left = chip.origin.x.clamp(
        panel.origin.x + PANEL_INSET,
        (panel.origin.x + panel.size.x - PANEL_INSET - width).max(panel.origin.x + PANEL_INSET),
    );
    let rect = Rect::xywh(
        left,
        chip.origin.y - TOOLTIP_ANCHOR_GAP - card_h,
        width,
        card_h,
    );

    // Walk the blocks down the card in paint order.
    let x = rect.origin.x + CARD_PAD;
    let mut y = rect.origin.y + CARD_PAD;
    y += TITLE_LINE_H;
    let badge = Rect::xywh(x, y, badge_w, BADGE_H);
    y += BADGE_H;

    let shown = (swatch_rows * SWATCHES_PER_ROW).min(card.swatches.len());
    let mut swatches = Vec::with_capacity(shown);
    if shown > 0 {
        y += BLOCK_GAP;
        let per_row = SWATCHES_PER_ROW.min(shown);
        let cell_w = (inner - SWATCH_GAP * (per_row - 1) as f32) / per_row as f32;
        for (index, swatch) in card.swatches.iter().take(shown).enumerate() {
            let row = index / SWATCHES_PER_ROW;
            let column = index % SWATCHES_PER_ROW;
            let cell = Rect::xywh(
                x + column as f32 * (cell_w + SWATCH_GAP),
                y + row as f32 * (SWATCH_SIZE + HEX_LINE_H),
                cell_w,
                SWATCH_SIZE + HEX_LINE_H,
            );
            let hex = text_metrics::fit_chrome(cx.backend, &swatch.hex, cell_w, HEX_FONT);
            swatches.push((cell, swatch.color, hex));
        }
    }
    let fonts = fonts.map(|fonts| text_metrics::fit_chrome(cx.backend, &fonts, inner, BODY_FONT));

    StyleCardLayout {
        rect,
        title,
        badge,
        badge_label,
        swatches,
        fonts,
        description,
    }
}

/// Height of a card carrying these blocks, padding included.
fn content_height(swatch_rows: usize, description_lines: usize, fonts: bool) -> f32 {
    let mut height = CARD_PAD * 2.0 + TITLE_LINE_H + BADGE_H;
    if swatch_rows > 0 {
        height += BLOCK_GAP + swatch_rows as f32 * (SWATCH_SIZE + HEX_LINE_H);
    }
    if fonts {
        height += BLOCK_GAP + BODY_LINE_H;
    }
    if description_lines > 0 {
        height += BLOCK_GAP + description_lines as f32 * BODY_LINE_H;
    }
    height
}

/// Greedily break `text` into at most `max_lines` lines of `max_w`.
///
/// Breaks at whitespace when there is one and mid-run when there is not, which
/// is what makes it work for a CJK description as well as a Latin one. The last
/// line is ellipsized rather than dropped: a description cut without a mark
/// reads as a complete but oddly terse sentence.
fn wrap_lines(
    cx: &mut PaintCx<'_>,
    text: &str,
    max_w: f32,
    font_size: f32,
    max_lines: usize,
) -> Vec<String> {
    if max_lines == 0 || max_w <= 0.0 {
        return Vec::new();
    }
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() && lines.len() < max_lines {
        if lines.len() + 1 == max_lines
            || text_metrics::measure_chrome(cx.backend, rest, font_size) <= max_w
        {
            lines.push(text_metrics::fit_chrome(cx.backend, rest, max_w, font_size));
            return lines;
        }
        let split = break_at(cx, rest, max_w, font_size);
        let (head, tail) = rest.split_at(split);
        let head = head.trim_end();
        if head.is_empty() {
            // Not one character fits; showing an empty line would be worse
            // than showing the first character clipped.
            lines.push(text_metrics::fit_chrome(cx.backend, rest, max_w, font_size));
            return lines;
        }
        lines.push(head.to_string());
        rest = tail.trim_start();
    }
    lines
}

/// Byte offset of the widest prefix of `text` that fits `max_w`, preferring
/// the last whitespace inside it.
fn break_at(cx: &mut PaintCx<'_>, text: &str, max_w: f32, font_size: f32) -> usize {
    let mut fits = 0;
    let mut last_space = None;
    for (offset, ch) in text.char_indices() {
        let end = offset + ch.len_utf8();
        if text_metrics::measure_chrome(cx.backend, &text[..end], font_size) > max_w {
            break;
        }
        fits = end;
        if ch.is_whitespace() {
            last_space = Some(end);
        }
    }
    last_space.unwrap_or(fits)
}

// ─── Paint ─────────────────────────────────────────────────────────────

/// Paint the card above `chip`. Hosts reach this through the chat panel's
/// paint, last, so it hangs over the input block.
///
/// Returns the painted rect, for tests.
pub(crate) fn paint_style_card(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    card: &StyleCard,
    chip: Rect,
    panel: Rect,
    locale: op_editor_core::Locale,
) -> Rect {
    let layout = layout_style_card(cx, card, chip, panel, locale);
    cx.backend
        .fill_round_rect(layout.rect, TOOLTIP_RADIUS, theme.popover);
    cx.backend
        .stroke_round_rect(layout.rect, TOOLTIP_RADIUS, theme.border, 1.0);
    // Nothing may spill past the rounded edge on a panel too short for the
    // whole card — the drop ladder in `layout_style_card` sizes for the space,
    // and this is the floor under it.
    cx.backend.save();
    cx.backend.clip_rect(layout.rect);

    let x = layout.rect.origin.x + CARD_PAD;
    let mut y = layout.rect.origin.y + CARD_PAD;
    draw_line(
        cx,
        &layout.title,
        TITLE_FONT,
        theme.popover_foreground,
        x,
        y,
        TITLE_LINE_H,
    );
    y += TITLE_LINE_H;

    // Provenance, in a pill so it reads as a stamp on the name rather than a
    // second line of it.
    cx.backend
        .fill_round_rect(layout.badge, BADGE_H / 2.0, theme.muted);
    draw_line(
        cx,
        &layout.badge_label,
        BADGE_FONT,
        theme.muted_foreground,
        layout.badge.origin.x + BADGE_PAD_X,
        layout.badge.origin.y,
        BADGE_H,
    );
    y += BADGE_H;

    if !layout.swatches.is_empty() {
        y += BLOCK_GAP;
        for (cell, color, hex) in &layout.swatches {
            let swatch = Rect::xywh(
                cell.origin.x + (cell.size.x - SWATCH_SIZE).max(0.0) / 2.0,
                cell.origin.y,
                SWATCH_SIZE.min(cell.size.x),
                SWATCH_SIZE,
            );
            cx.backend.fill_round_rect(swatch, SWATCH_RADIUS, *color);
            // A near-background swatch would otherwise be an invisible hole in
            // the band; the hairline states its edge without stating a colour.
            cx.backend
                .stroke_round_rect(swatch, SWATCH_RADIUS, theme.border, 1.0);
            let label_x = text_metrics::centered_text_x(
                cx.backend,
                hex,
                HEX_FONT,
                Rect::xywh(cell.origin.x, cell.origin.y, cell.size.x, HEX_LINE_H),
            );
            draw_line(
                cx,
                hex,
                HEX_FONT,
                theme.muted_foreground,
                label_x,
                cell.origin.y + SWATCH_SIZE,
                HEX_LINE_H,
            );
        }
        y = layout
            .swatches
            .last()
            .map(|(cell, _, _)| cell.origin.y + cell.size.y)
            .unwrap_or(y);
    }

    if let Some(fonts) = layout.fonts.as_deref() {
        y += BLOCK_GAP;
        draw_line(
            cx,
            fonts,
            BODY_FONT,
            theme.popover_foreground,
            x,
            y,
            BODY_LINE_H,
        );
        y += BODY_LINE_H;
    }
    if !layout.description.is_empty() {
        y += BLOCK_GAP;
        for line in &layout.description {
            draw_line(
                cx,
                line,
                BODY_FONT,
                theme.muted_foreground,
                x,
                y,
                BODY_LINE_H,
            );
            y += BODY_LINE_H;
        }
    }
    cx.backend.restore();
    layout.rect
}

/// One run, vertically centred in a `line_h` band starting at `y`.
fn draw_line(
    cx: &mut PaintCx<'_>,
    text: &str,
    font_size: f32,
    color: Color,
    x: f32,
    y: f32,
    line_h: f32,
) {
    if text.is_empty() {
        return;
    }
    let layout = TextLayout::single_run(
        text,
        text_metrics::CHROME_FONT_FAMILY,
        font_size,
        color.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    let band = Rect::xywh(x, y, 0.0, line_h);
    cx.backend.draw_text(
        &layout,
        Point2D::new(x, jian_widgets::centered_text_baseline_y(band, font_size)),
    );
}

#[cfg(test)]
#[path = "ai_chat_style_card_tests.rs"]
mod ai_chat_style_card_tests;
