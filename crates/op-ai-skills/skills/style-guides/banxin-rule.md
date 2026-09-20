---
name: 'banxin-rule'
tags: [cjk-type, light-mode, classical, editorial, serif, dual-font, warm-tones, red-accent, sharp-corners, flat, austere, refined]
platform: slides
---

## Style Scope

This guide is self-contained and written for **Chinese-native presentation decks** — a 1920×1080 fixed stage where the Chinese body text itself is the protagonist, not a caption under a Latin layout. Apply its palette, grid, rules, and folio marks only when this exact guide is selected; do not borrow dashboard, landing-page, or Latin-keynote patterns into it. Treat unnamed layout frames as structural by default: no fill, stroke, radius, or shadow unless the node is deliberately a 界行 rule, a quote block, a table zebra row, or a dark case page. Hierarchy comes from the head/foot asymmetry, the rules, and the size jump — never from wrapper cards.

## Style Summary

Anchored on the **page geometry of a woodblock-printed book**: 版心 (banxin, the type area bounded by the head and foot margins), 界行 (jiehang, the thin vertical rules that separate columns), 天头/地脚 (head margin far taller than foot margin), 鱼尾 (the geometric folio mark at the foot), and 函套 (the dark protective case the volumes sit in). What is anchored is the **page structure**, not any ornament. There is no brush, no wash, no seal, no cloud pattern anywhere in this style.

Sampling ran over the yellow of xuan paper, the brown cast of ink, cinnabar collation marks, and the dark brown of a book case. It converged on three facts: the light ground is xuan white at L0.955 C0.010 H88; the dark ground is case ink at L0.215 C0.014 H60; the only chromatic colour is cinnabar at H32 C0.140, and every neutral sits warm at H60.

Three arguments carry the whole style:

- **Ink is taken at L0.265 with 0.012 of warm chroma, never pure black.** Paper reflects warm; pure black on a warm white sheet floats a layer above it. This is the exact line between this deck and a black-on-white slide.
- **Head margin > foot margin, about 1.35 : 1 (144 : 108).** Books did this so the head could carry annotations. We keep it for two contemporary reasons: a projected slide's lower edge is routinely blocked by the heads in the room, and the asymmetry instantly breaks the "everything centred, all four margins equal" AI fingerprint. One decision solving a physical problem and an aesthetic one at once.
- **Cinnabar appears only as the folio mark and one emphasis per page, never as area.** The standard collapse of a Chinese palette is red spread wide — the moment it spreads, the page stops being a book leaf and becomes a festival poster.

Key aesthetics:

- **Two grounds, used as bookends**: xuan paper for the content pages, case ink for the cover, contents, and closing
- **界行 as the structural device**: a 1px vertical rule, full type-area height, marking the page rather than boxing the content
- **Warm neutral ramp only**: every grey is H60 warm; a cold grey goes green against this paper within one step
- **Cinnabar rationed to a folio mark**: one accent colour, two sanctioned uses, no third
- **Head-note as a third text layer**: title / body / 注疏 — annotation living in the head margin, not a header
- **Zero radius, zero shadow, zero gradient**: a printed leaf has none of the three
- **Two rule weights only**: 1px 界行, 2px divider — nothing between and nothing above
- **Numerals leave the Chinese family**: figures and Latin take mono or the paired Latin serif

## Color System

### Grounds

| Token         | Value   | Usage                                                                 |
| ------------- | ------- | --------------------------------------------------------------------- |
| Xuan White    | #F3F0E9 | Content-page ground. The paper; never a block colour                  |
| Xuan Deep     | #E5E1D7 | Quote blocks and table zebra rows. 11.73:1 under ink text             |
| Case Ink      | #1E1813 | Cover / divider / closing ground — the 函套 case                       |
| Case Ink Low  | #130E0A | A deeper block laid on a case-ink page                                |

Four steps, all warm (H60–H88). A hue shift between grounds would read as two different papers.

### Text Colors

| Token             | Value   | Usage                                                            |
| ----------------- | ------- | ---------------------------------------------------------------- |
| Ink Text          | #2A241F | Body copy and titles on paper. 13.46:1                           |
| Ink Soft          | #5F5A55 | Secondary text, side notes, table sources. 5.99:1                |
| Ink Faint         | #807A76 | Head-notes and folio numerals. 3.72:1 — **legal at ≥24px only**   |
| Xuan on Case      | #F0ECE7 | Primary text on a case-ink page. 14.94:1                         |
| Xuan Dim on Case  | #B4B0AA | Secondary text on a case-ink page. 8.14:1                        |

Ink Faint is the one tone in this palette that fails at small sizes by design — it exists for the two elements that are supposed to recede, and both of them are set at 24px or above. Never move it onto body copy to make a page "quieter".

### Rule and Accent

| Token            | Value   | Usage                                                                          |
| ---------------- | ------- | ------------------------------------------------------------------------------ |
| Jie Rule         | #D0CBC5 | The 界行 rules. **Non-text only** — never carries a glyph                       |
| Cinnabar Folio   | #AA4331 | **The sole accent**: folio mark, folio numeral, one emphasis per page. 5.19:1  |
| Cinnabar Wash    | #F9D7CE | Pale cinnabar ground that carries ink text. 11.41:1                            |

Two cinnabar tones, not interchangeable: the strong tone is a mark, the wash is a ground. There is no third chromatic colour in this style, and no second accent may be introduced for a chart series, a status tag, or a highlight — the neutral ramp carries all of that.

## Typography

### Font Families

| Role              | Preferred                                  | Fallback (ship this)  | Usage                                    |
| ----------------- | ------------------------------------------ | --------------------- | ---------------------------------------- |
| Display           | 源流明體 / 思源宋體 Semibold                 | Noto Sans SC 700      | Cover title, page titles, closing line   |
| Body              | 霞鶩文楷 (LXGW WenKai, OFL)                  | Noto Sans SC 400      | Body copy, side notes, table cells       |
| Numerals / Latin  | IBM Plex Mono + Source Serif 4              | Inter 400             | Folio numerals, contents numbering, data |

霞鶩文楷 is the preferred body face because it stays readable across long Chinese passages, which is the load this deck is built to carry. The Latin face is written **first** in every fallback chain: font matching is per-character, Latin faces carry no Han codepoints, so Han falls through to the Chinese face automatically. Written the other way round, the Chinese face's own weak Latin swallows every digit and the paired face never appears.

**Never mix more than two Chinese families.** 宋 + 黑 is the ceiling, and 宋 is display-only.

### Type Scale

| Level          | Size | Font     | Line Height | Usage                                                  |
| -------------- | ---- | -------- | ----------- | ------------------------------------------------------ |
| Cover Title    | 96px | Display  | —           | Cover only, ≤2 lines                                   |
| Closing Line   | 64px | Display  | —           | Closing page only, ≤2 lines                            |
| Page Title     | 56px | Display  | —           | Every interior page                                    |
| Section Title  | 40px | Display  | —           | Contents entries, in-page section heads                |
| Quote Body     | 36px | Body     | 1.8         | Quote-page block only, ≤4 lines                        |
| Body           | 32px | Body     | **1.75**    | **Default body copy**; also the cover subtitle         |
| Note           | 26px | Body     | —           | Head-notes, side notes, quote sources                  |
| Folio          | 24px | Numerals | —           | Folio numeral under the fishtail mark                  |

96px is the cover ceiling **because Han strokes are dense** — at the same visual weight a Chinese title sits one step below its Latin equivalent, so 96 here does the work 120 does in a Latin deck. Line-height and letter-spacing bands come from `cjk-typography`; the only value this style pins beyond them is body 32/1.75, which is deliberately looser than the band's floor because the body block is the protagonist.

### Font Weights

| Weight   | Value | Usage                                                                    |
| -------- | ----- | ------------------------------------------------------------------------ |
| Regular  | 400   | All body copy, side notes, table cells, folio numerals                   |
| Semibold | 600   | The preferred display face (思源宋體 Semibold) at every display level      |
| Bold     | 700   | The fallback display face (Noto Sans SC) — it needs 700 to hold the same weight |

Chinese has no italic. Never set `font-style: italic` on Han glyphs; the synthesised skew deforms the strokes. Emphasis moves to weight, to a `Cinnabar Wash` ground, or to a change of face — never to a colour change on the text itself, because the only chromatic colour is reserved.

### The measure, and why 界行 exists

The body block is **never allowed to fill the 1680px content width** — at 32px that is 54 characters to the line, far past the ceiling in `cjk-typography`. The body block therefore narrows to **8 columns (≈1128px)** or splits into two columns. That constraint is the entire functional reason the 界行 motif exists: the rules are what make a narrowed or split measure read as a designed page instead of a short paragraph floating in white space. They are structure earning its keep, not decoration.

## Layout Grammar

- Stage 1920×1080, one top-level frame per page.
- Margins **top 144 / bottom 108 / left and right 120** — 天头 > 地脚, and this asymmetry is not negotiable.
- Content width **1680 = 12 columns × 118 + 11 gutters × 24**.
- Corner radius **0**, everywhere, on everything.
- Rule weights: **1px** for 界行, **2px** for a divider. There is no third weight.
- Gap base **24**; every vertical rhythm is a multiple of it.
- Draw every rule as a `rectangle` (a 1–3px-high or -wide rect), never as a `line` node.

## Spacing System

Every value below is fixed by the page geometry; none of them is a free parameter.

| Value    | Usage                                                                        |
| -------- | ---------------------------------------------------------------------------- |
| 4px      | The gap between the two fishtail tips — the smallest measure in the style     |
| 8px      | The side of each fishtail triangle                                            |
| 12px     | Fishtail mark to folio numeral                                                |
| 24px     | **The gap base.** Column gutter, and the unit every vertical rhythm multiplies |
| 32px     | Text clearance on each side of a 界行 rule                                     |
| 56px     | Page top edge to the head-note baseline row                                   |
| 108px    | Foot margin                                                                   |
| 120px    | Left and right margins                                                        |
| 144px    | Head margin — 1.35× the foot                                                  |

A value that is not on this list and not a multiple of 24 is a value someone invented. The 4 / 8 / 12 trio exists solely inside the folio mark and appears nowhere else in the deck.

## Signature Motifs

**1 · 界行 `jie-rule`** — column separation is a 1px vertical `Jie Rule` line that **spans the full type-area height**, from the underside of the head margin to the top of the foot margin. It does not stretch or shrink with the text beside it. This is the root difference from card-based columns: a card boxes content, a 界行 marks the page. Text keeps **32px** clearance on each side of the rule. Horizontal 界行 rules follow the same weight and colour and are used between contents entries and between table rows.

**2 · 鱼尾页码 `folio-mark`** — centred in the foot margin: two **8px** isosceles triangles set point-to-point with a **4px** gap between the tips, filled `Cinnabar Folio`; **12px** below it, the folio numeral at mono **24px** in `Ink Faint`. This is an original geometric form — **draw no fish and no representational object of any kind**. It repeats on every page and is the deck's constant identity anchor across spreads.

**3 · 天头批注 `head-note`** — inside the head-margin band, **56px** from the top edge, a single line of **26px** `Ink Faint` annotation. It is not a title and not a running header: it carries meta-information about where this page sits in the argument, and it constitutes the third text layer of the style (title / body / 注疏). Head-notes are **optional**, but the moment one page uses one, **at least three pages in the deck must** — a single occurrence reads as a stray element.

## Page Types

| #  | Page              | Density     | Slot cap | Structure                                                                                                            |
| -- | ----------------- | ----------- | -------- | -------------------------------------------------------------------------------------------------------------------- |
| 01 | Cover · case      | low         | 4        | Case-ink ground; title 96 (≤2 lines) + a 96×2 cinnabar short rule + subtitle 32 + mono byline                         |
| 02 | Contents · case   | low         | 7        | Case-ink ground; 5–6 entries, each a mono numeral + a 40px entry name, 1px horizontal 界行 between entries             |
| 03 | Argument · paper  | medium-high | 8        | Head-note + page title 56 + body across 8 columns at 32/1.75 (≤6 lines) + a 3-column side note (26px) right of the rule |
| 04 | Parallel reading  | medium-high | 10       | Page title 56 + two 5-column columns split by a 1px 界行; left 「其说」 (their claim), right 「我见」 (my reading); the right column's opening sentence sits on `Cinnabar Wash` |
| 05 | Quote             | medium      | 5        | Page title 56 + a `Xuan Deep` quote block (36/1.8, ≤4 lines) + right-aligned source at 26 + 界行 bracketing the block left and right |
| 06 | Table             | medium-high | 12       | Page title 56 + table (2px ink rule under the header; 1px 界行 between rows; **no rule on the last row**; zebra in `Xuan Deep`) + source |
| 07 | Closing · case    | low         | 4        | Case-ink ground; closing line 64 (≤2 lines) + an enlarged cinnabar fishtail centred + byline                          |

**Dark and light pages are bookends only** — the first two pages and the last. The middle of the deck never alternates grounds.

## Strictly Avoid

- **No ink wash, brushwork, flying white, bleed, seals, key-fret or cloud patterns, ruyi motifs, or decorative vertical lettering.** The identity of this style is page geometry; the instant any of these appear it collapses into a generic "China-style template".
- **No corner radius, no shadow, no gradient.** Not on a quote block, not on a table, not on the cover.
- **No cinnabar as area.** Never a large ground, never a gradient, never a text colour — the two exceptions are the folio and the single per-page emphasis.
- **No 楷 or 宋 above 88px.** Stroke ink distributes unevenly at giant sizes and the glyphs shimmer; giant type goes to the sans face.
- **界行 rules carry no text, are never a background, and never exceed 2px.** A thickened rule stops being a page mark and becomes a border.
- **No third Chinese family.** 宋 + 黑 is the ceiling, and 宋 stays display-only.
- **Head-notes never carry a title, a page number, or "本页要点"-style meta commentary.** That is a running header, and this style does not have one.

## Anti-Patterns

- **The 界行 that hugs its text.** A rule sized to the paragraph beside it is a card border wearing a different name. It runs the full type area or it is deleted.
- **The symmetric page.** Equal head and foot margins undo the one decision that makes this style legible as a book leaf — and hand back the AI fingerprint the asymmetry was bought to break.
- **A fishtail redrawn per page.** The mark is the identity anchor precisely because it is byte-identical on all seven pages; a "variation" on page four reads as a mistake.
- **Cinnabar as a chart palette.** Series colour comes from the warm neutral ramp; the accent marks one thing on a page, and a chart with four cinnabar bars marks nothing.
- **A card wrapper around body copy on a paper page.** The paper is already the container. Rules and margins do the grouping.
- **Alternating grounds through the middle of the deck.** Case ink is a binding, not a rhythm device — pages 3 through 6 are paper, without exception.
- **A head-note on exactly one page.** Below three occurrences the layer does not exist; it just looks like something landed in the margin.
- **A cold grey borrowed from another guide.** Every neutral here is warm at H60; one cold grey dropped in reads green against this paper and gives away that the palette was assembled rather than derived.
