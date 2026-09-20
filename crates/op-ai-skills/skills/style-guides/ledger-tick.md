---
name: 'ledger-tick'
tags: [light-mode, warm-tones, data-focused, corporate, enterprise, monospace, cjk-type, crisp, flat, sharp-corners, stroke-based]
platform: slides
---

## Style Scope

This guide is self-contained and written for **one deck genre only: the competitive matrix** — an evaluation deck whose job is to lay out "what we can do and what they can do" as a ledger a reader can check cell by cell. Apply its palette, rules, and mark language only when this exact guide is selected; do not borrow dashboard, keynote, editorial, or pitch-deck patterns into it. Treat unnamed layout frames as structural by default: no fill, stroke, radius, or shadow unless the node is deliberately the paper ground, a zebra band, a ledger rule, the own-column wash, or one of the three tick marks. Hierarchy comes from the rules and the marks, not from card shells.

## Style Summary

The anchor is a **hand-kept account book**: warm paper, blue printed grid lines, red header and totals lines, a mark drawn into each cell, a double rule under the tally. What is anchored is not the look of the book — it is the *act of checking a row against a column*. Every decision below serves that act.

Three arguments define the tier and none of them are optional:

1. **"Not met" is an empty space, not a cross.** Unmet is a short pale dash. A red X is a value judgement about a competitor and turns the whole deck into an attack pitch instead of an assessment. A blank is a statement of fact. This also sidesteps red/green scoring, a known credibility killer.
2. **Only the own column is coloured.** Every competitor column is neutral, at identical column width, row height, and type size. Colour is a position, and a position gets stated exactly once.
3. **The two rule colours sit at 3.13 contrast on purpose.** They are *lines, not text*. Push their contrast up and they start competing with the marks for attention. A contrast checker must carry them on its exemption list; they never carry a glyph.

Key aesthetics:

- **Ledger paper ground**: `#F5F2E6` at oklch L0.960 C0.016 H92 — warm account-book stock, never a white slide
- **Two rule hues, blue and red**: blue rules the grid, red rules the header and the tally; neither ever sets type
- **Three marks that differ by shape**: a check, a half-filled square, a short dash — distinguishable with colour removed
- **One wash, one place**: `#DFF3E2` appears on the own column and nowhere else in the whole deck
- **Zebra bands, no boxes**: rows separate by 1px blue rules and alternating bands; there is no table border box
- **Mono for every figure**: all numerals and every mark column are forced to IBM Plex Mono
- **Radius 0 everywhere**: a ruled ledger has no rounded corners, no shadow, and no gradient
- **The tally line is a sentence**: the double rule carries a written verdict, never a weighted score

## Color System

### Ground

| Token         | Value   | Usage                                                              |
| ------------- | ------- | ------------------------------------------------------------------ |
| ledger.paper  | #F5F2E6 | Page ground on every slide; also the ink colour inside a green chip |
| ledger.band   | #EBE6D8 | Zebra row band, and the four segments of a quartile bar (13.16:1)  |

### Ink

| Token      | Value   | Usage                                              |
| ---------- | ------- | -------------------------------------------------- |
| ink.entry  | #221F1B | Primary text: titles, row labels, verdicts (14.63:1) |
| ink.soft   | #5A5652 | Secondary text: definitions, current-state lines (6.48:1) |
| ink.faint  | #78746F | Footnotes, sources, page numbers, dates (4.14:1)   |

### Rules — lines only, never type

| Token      | Value   | Usage                                                         |
| ---------- | ------- | ------------------------------------------------------------- |
| rule.blue  | #88A2B9 | The ledger grid: 1px cell rules and row separators (3.13:1)    |
| rule.red   | #BD7670 | The 2px header underline and the tally double rule (3.13:1)    |

Both are deliberately low-contrast and are exempt from text-contrast checks **because no glyph is ever painted in them**. Setting a label, a number, or a caption in `rule.blue` or `rule.red` is the single fastest way to break this style.

### Marks

| Token      | Value   | Usage                                                          |
| ---------- | ------- | -------------------------------------------------------------- |
| tick.green | #2F7442 | Met: the check mark, the own-column frame and chip (5.06:1; 4.88:1 on own.wash) |
| tick.grey  | #888682 | Partially met: the half-filled square (3.24:1 — a mark, not text) |
| tick.pale  | #C6C4C1 | Not met: the short dash standing in for an empty cell           |

### Own column

| Token     | Value   | Usage                                                     |
| --------- | ------- | --------------------------------------------------------- |
| own.wash  | #DFF3E2 | Full-height fill behind the own column only (14.11:1 against ink.entry — the table text stays readable inside it) |

`own.wash` is capped at one appearance per deck. A second washed column means two positions are being argued, and the matrix stops being an assessment.

## Typography

### Font Families

| Role            | Family                                   | Usage                                                  |
| --------------- | ---------------------------------------- | ------------------------------------------------------ |
| Display         | 思源黑体 Bold / Familjen Grotesk          | Cover title, page titles                               |
| Body            | 霞鹜新晰黑 / Familjen Grotesk             | Row labels, definitions, notes, verdict sentences      |
| Data / Marks    | **IBM Plex Mono**                        | **Mandatory** for every figure and every mark column   |

Fallback chain: display `Noto Sans SC` 700 / body `Noto Sans SC` 400 / mono `Inter` 500.

The mono rule is not decoration. Column headers, dates, page numbers, and every cell in a mark column are set in IBM Plex Mono so glyph advances are uniform and marks land on the same optical centre down a column. A proportional face makes an eight-row column visibly ragged, and a ragged column cannot be checked at a glance.

### Type Scale

| Level          | Size | Font    | Usage                                                   |
| -------------- | ---- | ------- | ------------------------------------------------------- |
| Cover title    | 88   | Display | Cover slide only                                        |
| Page title     | 56   | Display | Every interior slide                                    |
| Verdict        | 44   | Display | The single judgement sentence on the conclusion slide    |
| Mark glyph     | 32   | Mono    | The tri-tick cell mark, and the tally conclusion line    |
| Item title     | 32   | Body    | One gap / one advantage / one criterion heading          |
| Cover subtitle | 30   | Body    | Cover subtitle only                                      |
| Table header   | 28   | Mono    | Uppercase, letterSpacing **+2** on Latin, **0 on CJK**   |
| Row label      | 28   | Body    | The left-hand criterion label in the matrix              |
| Body note      | 28   | Body    | Definitions, current state, plan, reading notes          |
| Footnote       | 24   | Body    | Data cut-off date, sources, page numbers                 |
| Chip label     | 20   | Mono    | Uppercase text inside the own-column chip, in ledger.paper |

Table headers are the only uppercase run in the system, and +2 tracking is what makes an uppercase mono header read as a ledger column head rather than as shouting. Tracking follows the run's actual script: a Latin header takes +2, a Chinese one takes 0, because positive tracking on Han glyphs loosens an already full-width em box into gaps.

## Layout Grammar

Margins: **top 88 / bottom 104 / left and right 96**. Base gap: **16**.

The matrix grid:

- **Row-label column fixed at 360px.** Every remaining column splits the leftover width equally — competitor and own columns are always the same width.
- **Row height 72.** Uniform, including the header row. The mark sits centred horizontally and vertically in its cell.
- **6–8 rows, at most 5 columns** (the row-label column counts toward the 5; one of the remaining columns is the own column). Over the cap, split into two slides — never shrink the row height.
- On the 1920-wide slide frame the content band is 1728 wide; after the 360 label column, 1368 is shared equally by the comparison columns.

The rules:

- Cell / row rules: **1px `rule.blue`**
- Header underline: **2px `rule.red`**, directly under the header row
- Tally: a **1px + 2px double rule** in `rule.red`, **4px apart**, above the last line of the table
- No outer table border, no vertical box, no coloured header fill

## Corner Radius

| Value | Usage                                                              |
| ----- | ------------------------------------------------------------------ |
| 0px   | Everything: chips, bands, quartile segments, the own-column frame   |

There is no second row in this table on purpose. A ledger is ruled, not rounded; a radius anywhere reads as a UI card dropped onto an account page.

## Signature Motifs

### 1. `tri-tick` — the three-state cell mark

- **Met**: a **24px** check in `tick.green` — two straight segments, **2.5px** stroke, square ends. It is a drawn polyline, not a glyph in a circle and not a dot.
- **Partially met**: a **24×24** square in `tick.grey` with its **left half filled** — a half cell, literally.
- **Not met**: a **20×2px** dash in `tick.pale`, centred in the cell.

The three states differ *by shape*, so the matrix survives greyscale printing and colour-blind readers. That is the accessibility floor and also what separates this from a red/green score sheet.

### 2. `own-column` — the one coloured column

- The full column is filled with `own.wash`.
- A **2px `tick.green`** frame runs around it, from the top of the header row down to the tally rule.
- The column head carries one solid `tick.green` chip: **height 32, radius 0**, containing **20px uppercase `ledger.paper`** text (the product name).
- Column width, row height, and type size are identical to every competitor column. Only the wash and the frame differ.

### 3. `tally-rule` — the double rule and its sentence

- Above the closing line: **1px + 2px `rule.red`**, 4px apart.
- Below it sits a single **32px `ink.entry`** sentence, left-aligned, spanning the full table width.
- It is a conclusion, not an arithmetic total. The ledger's tally convention is borrowed to carry the "so what", which is the one thing a matrix otherwise leaves unsaid.

## Page Types

| #  | Page             | Density     | Slot cap | Structure                                                                                         |
| -- | ---------------- | ----------- | -------- | ------------------------------------------------------------------------------------------------- |
| 01 | Cover            | low         | 5        | Title 88 + subtitle 30 + one line naming the evaluation scope + date in mono                       |
| 02 | Criteria         | medium      | 8        | Page title 56 + 4–5 criterion definitions (title 32 + note 28) + data cut-off footnote             |
| 03 | Main matrix      | **high**    | **16**   | Page title 56 + matrix (6–8 rows × up to 5 columns, own column included) + one legend row + tally double rule + conclusion line |
| 04 | Quartile scale   | medium-high | 9        | Page title 56 + 3 quartile bars (four equal `ledger.band` segments + 1px separators + a 6px `tick.green` position marker) + one reading note each |
| 05 | Gaps             | medium-high | 9        | Page title 56 + 3 gaps (`tick.grey` half-cell + title 32 + current 28 + plan 28), separated by 1px `rule.blue` |
| 06 | Advantages       | medium-high | 9        | Page title 56 + 3 advantages (same structure, but the mark is the `tick.green` check) + one verifiable proof line each |
| 07 | Conclusion       | medium      | 6        | Page title 56 + one verdict sentence 44 + 3 imperative actions + data-source footnote              |

## Strictly Avoid

1. **No red cross, and no red/green scoring.** Unmet is the pale dash.
2. No competitor logos, no competitor product screenshots, no visual that can be read as disparagement.
3. Only the own column is coloured. Competitor columns stay neutral, at identical width, row height, and size.
4. `rule.blue` and `rule.red` never carry text.
5. No rounded corners, no shadow, no gradient.
6. The three marks must be distinguishable by shape. Never render them as three dots in three colours.
7. One mark per cell, and nothing else — no explanatory text inside a cell. Explanations belong on slides 05 and 06.
8. Every criterion must be given a checkable definition on slide 02. An undefined criterion does not go into the matrix.
9. Matrix caps: 8 rows, 5 columns. Over the cap, split into two slides — **never reduce the row height**.
10. The tally line is a sentence, not a score. No weighted totals — a single number hides every trade-off the matrix exists to expose.

## Anti-Patterns

- **Scoring instead of recording.** The moment a column of numbers is summed, the deck stops being checkable and becomes a claim. Cells record state; the sentence under the tally carries the argument.
- **Decorating the competitor columns.** Tinting them, greying them out, or narrowing them all say the same thing — that the comparison was arranged rather than made.
- **Raising the rule contrast "for legibility".** The grid is meant to sit beneath the marks. If the blue lines are noticeable before the green checks are, the page has inverted.
- **A second washed column, or a green accent anywhere off the own column.** `tick.green` is a mark colour and the own-column frame; it is not a general highlight.
- **A tick glyph borrowed from an icon set.** The check is drawn as a two-segment 2.5px polyline at 24px so it matches the half-cell and dash in weight; an icon-font check arrives at a different optical weight and breaks the three-way shape reading.
