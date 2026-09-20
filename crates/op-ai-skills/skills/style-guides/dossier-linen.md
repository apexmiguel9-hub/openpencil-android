---
name: 'dossier-linen'
tags: [light-mode, monochrome, editorial, corporate, enterprise, serif, monospace, cjk-type, austere, quiet, sharp-corners, flat]
platform: slides
---

## Style Scope

This guide is self-contained and written for a **document memo deck** — a run of pages
that are read, not presented. Every page is a sheet that can be pulled out of the folder
and understood on its own; nothing on it depends on the page before it. Apply this
palette, grid, and type treatment only when this exact guide is selected; do not borrow
dashboard, keynote, poster, or landing-page patterns into it.
Treat unnamed layout frames as structural by default: no fill, no stroke, no cornerRadius,
no shadow, unless the node is deliberately the binding rail, a quote block, a decision
block, or a table band.
Hierarchy comes from clause numbering and from where a line starts on the grid — never
from a card shell.

## Style Summary

Linen paper with file ink on it. The ground is `#F3EEE4`, the warm-grey yellow of a linen
document envelope — chroma 0.014, which is what makes it read as *official stock* rather
than a cream journal page or a newsprint tint. Above it there is one colour and one only:
a rust thread at chroma 0.090, and it is allowed to be a binding line, a page number, and
a clause number. Nothing else.

**This is the "high density ≠ small type" reference deck.** Its body density is the
highest in the whole system, and its type floor is identical to every other deck's.
Density here is bought with **line length, paragraph structure, and where the whitespace
is spent** — never by shrinking the body. Body 30 at line-height 1.8 is the absolute
floor of this style. When a page will not fit, you delete a clause or add a page. You do
not go to 28.

Two derivations are load-bearing and should not be re-tuned:

- **One chromatic ink, on purpose.** A memo persuades through structure, not through
  colour. A second hue would send the reader hunting for "the highlight colour", and in
  this deck the emphasis lives inside the sentence.
- **Rust stops at chroma 0.090.** Pushed to 0.13 and above, the binding line stops being
  a *thread* and starts reading as a red annotation rule — a different meaning entirely.

Key aesthetics:

- **Linen ground**: `#F3EEE4` at chroma 0.014 — paper stock, never a panel colour
- **A single chromatic ink**: rust, on the binding rail, folio numbers, and clause numbers
- **Zero bullets anywhere**: numbered clauses and whole paragraphs, at most a ruled table
- **Asymmetric margins**: 216 left, 120 right — the binding takes the difference
- **Tables, never charts**: this deck reports; it does not present
- **Radius 0, no shadow, no gradient, no icon, no photo, no card**
- **The rail never moves**: same x on every page, and so is the body start

## Color System

### Ground

| Token | Value | Usage |
| --- | --- | --- |
| Linen | #F3EEE4 | Page ground. Every page, no exceptions |
| Linen Band | #EFE9DE | Table zebra rows only. 14.00:1 under file ink |
| Linen Deep | #E6DFD3 | Quote blocks and decision blocks. 12.77:1 under file ink |

Three steps at hue 80-82, separated by lightness alone (0.950 / 0.935 / 0.905). The steps
are deliberately close: they mark a change of *register*, not a change of surface. If a
block needs to be seen from across the room, this is the wrong deck.

### Ink

| Token | Value | Usage |
| --- | --- | --- |
| File Ink | #201C19 | Body copy, section titles, table text. 14.63:1 on linen |
| Soft Ink | #56524E | Secondary lines, source rows, table headers. 6.70:1 |
| Faint Ink | #746F6B | Folio labels and clause numbers when unaccented. 4.29:1 |

All three sit at chroma 0.008 hue 60 — a warm near-black rather than a true black. A
neutral grey secondary tone goes cold against linen within one step and reads as a
different document.

### Rule and Thread

| Token | Value | Usage |
| --- | --- | --- |
| Hairline | #C0BDB8 | 1px rules: table lines, question/answer separators, folio underscore. Never text |
| Bind Thread | #864737 | **The only chromatic ink**: binding rail, folio numbers, clause numbers. 6.11:1 |

Bind Thread is a line and a numeral. It is never a fill behind text, never a heading
colour, and never a second accent's excuse.

## Typography

### Font Families

| Role | Family | Usage |
| --- | --- | --- |
| Display | 思源宋体 Semibold, Source Serif 4 | Document title and section titles |
| Body | 霞鹜文楷, Source Serif 4 | Every paragraph, quote, and table cell |
| Mono | IBM Plex Mono | Clause numbers, folio tabs, dates, owners, deadlines |

Fallbacks: display `Noto Sans SC` 600, body `Noto Sans SC` 400, mono `Inter` 400. 霞鹜文楷
is the body face because long CJK paragraphs are what this deck is made of, and it holds
readability at paragraph length better than a screen sans. Write the Latin face **first**
in every fallback chain — matching is per-character, so Han falls through to the CJK face
automatically, whereas the reverse order lets the CJK face's own Latin swallow every
digit and page number.

### Type Scale

| Level | Size | Font | Line Height | Usage |
| --- | --- | --- | --- | --- |
| Document Title | 76 | Display | — | Cover page only, once in the whole deck. 76 rather than 72 because the largest size must be >= 2.5x the 30px body |
| Section Title | 44 | Display | — | Every interior page |
| Body | 30 | Body | 1.8 | Paragraphs, quotes, question/answer text. **The floor** |
| Table Body | 28 | Body | — | Inside ruled tables only |
| Clause Number | 26 | Mono | — | §2.1 markers in the hanging indent |
| Folio | 26 | Mono | — | Page tab and document code. Folded up from 24 so the cover runs 76/44/30/26 — four steps, not five |

Section title is **44 — the smallest interior title in the system**, and that is the
point: it is a document's section heading, not a slide's page title. It announces where
you are in the file; it does not perform.

### Line Height

Body is **1.8**, and it goes *up* rather than down as density rises. This is the
counter-intuitive rule of the deck: once line length grows and paragraph count grows,
leading is the only variable still holding readability, so it is the last thing to be
spent. Titles and mono runs set solid enough to keep the page compact; the paragraph is
where the air goes.

### Letter Spacing

The document code in the top-left folio position sets uppercase at **+2** tracking. Body,
titles, and table text take no tracking adjustment — a memo's job is to be transparent.

## Layout Grammar

Margins are **96 top, 96 bottom, 216 left, 120 right**. The left margin is not a taste
decision: it is the space the binding takes. Body copy starts 96px to the right of the
binding rail, and the clause numbers hang in the gap to its left. That asymmetry is the
strongest identity signal in the deck, and it is also what immunises it against the
"every page anchored at the same spot" tell.

The content band is **10 columns × 144 with 9 gutters × 16 = 1584 wide** — the gutter is 16 here rather than 24 because that is the only value that divides this band into whole columns. Running text
narrows further, to **eight of those ten columns**, purely to hold line length — the
content band is the table's width, not the paragraph's.

Corner radius is **0** everywhere. Zero shadow, zero gradient. Nothing in this deck is a
card, so nothing needs a corner.

## Signature Motifs

**1. Binding rail (`binding-rail`).** A 2px Bind Thread vertical line at **x = 120** from
the left page edge, running the full page height. Three 12×12 squares filled in Linen and
stroked 1px Hairline sit on the line at equal spacing — the binding holes. The rail
repeats on every page at exactly the same x, with zero drift.

**2. Numbered clause (`numbered-clause`).** The clause number is set in mono at 26 and
hangs in the margin **to the left of the rail**, right-aligned to rail − 24. The clause
body starts at rail + 96 and runs as ordinary prose at 30/1.8. There are **no bullet
glyphs anywhere in the deck** — depth is carried entirely by the number (§2 / §2.1 /
§2.1.1), and it stops at three levels.

**3. Folio tab (`folio-tab`).** Top-right: mono 24 reading `p. 3 / 8`, with a 48 × 1px
Hairline rule directly beneath it. Top-left, at the mirrored position: the document code
in mono, uppercase, +2 tracking. Together they tell the reader this is sheet N of a file,
which is the whole premise of the deck.

## Page Inventory (8 pages)

| # | Page | Structure |
| --- | --- | --- |
| 01 | Cover sheet | Document code + title 76 + one abstract paragraph at 30/1.8 (≤5 lines) + three mono lines: recipient, date, classification |
| 02 | Background | Section title 44 + clauses §1.1–§1.3, each ≤6 lines |
| 03 | Current data | Section title 44 + a ruled table of 6-8 rows + a source line. **Data appears as a table; never as a chart** |
| 04 | Analysis | Section title 44 + clauses §3.1–§3.2 + one Linen Deep quote block |
| 05 | Options | Section title 44 + a comparison table (rows = criteria, columns = options); the final row is the recommendation, set bold, with a 2px Bind Thread rule above it |
| 06 | Retrospective narrative | Section title 44 + one past-tense "assume this succeeded — here is what it looked like" passage at 30/1.8 (≤8 lines) + a single attributed line, indented italic 30 |
| 07 | Open questions | Section title 44 + 4-5 question/answer pairs (question mono bold 30; answer 30/1.8, ≤3 lines) separated by 1px Hairline |
| 08 | Resolutions | Section title 44 + a Linen Deep decision block holding 3 items, each one sentence at 30 plus a mono owner and a mono deadline + a sign-off slot |

## Strictly Avoid

1. **No bullet glyph, anywhere in the deck.** Numbered clauses, whole paragraphs, ruled
   tables. That is the complete inventory of list forms.
2. **No charts.** A chart in this deck is a presentation artefact, and this is not a
   presentation. Data goes into a table.
3. **No second chromatic ink.** Outside the rust thread, everything is neutral.
4. **No radius, shadow, gradient, icon, photograph, or card.**
5. **No rail drift.** The binding rail and the body start sit on the same x on every
   single page.
6. **Clause depth stops at three.** A fourth level means the section should be split into
   its own page or merged upward.
7. **Density never touches the type floor.** Body 30 is absolute. If it does not fit,
   remove a clause or add a page — shrinking the body is the failure mode this deck
   exists to demonstrate you do not need.
8. **Folio must be a fraction.** `p. 3 / 8`, never `p. 3`. A one-sided page number tells
   an asynchronous reader nothing.
9. **No order-dependent phrasing.** "As shown on the previous page" is banned; every page
   must survive being pulled out of the folder alone.
10. **Tables use horizontal rules and zebra bands only.** No coloured header row, no full
    box border, no vertical rules.

## Anti-Patterns

- **Small type standing in for editing.** The moment a page is set at 28 to make it fit,
  the deck has lost the argument it was built to make. Density is a structural result,
  not a font-size result.
- **The rust thread as a highlight colour.** Rust behind a heading, rust as an emphasis
  fill, rust on a keyword — each turns the binding into decoration and immediately reads
  as a template.
- **A card wrapper around a clause.** A filled, rounded, shadowed container around body
  copy converts a memo into a slide. Linen Deep blocks exist for quotes and decisions
  only, and they carry no radius.
- **A symmetric left margin.** Setting 120/120 destroys the hanging indent, strands the
  clause numbers, and removes the one thing that makes these pages look like a bound file.
- **Chart-shaped tables.** Coloured header bands, alternating column fills, and boxed
  cells are a chart wearing a table's clothes; keep to rules and bands.
