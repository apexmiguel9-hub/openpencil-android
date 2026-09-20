---
name: 'tidemark-slate'
tags: [light-mode, data-focused, enterprise, corporate, monospace, cjk-type, blue-accent, clean, crisp, flat, dual-font]
platform: slides
---

## Style Scope

This guide is self-contained and written for the **data-review deck**: medium-high formality × **high** density × light. It is the deck you present when a cycle has closed and the room needs to know what happened and where it sits relative to last cycle. Every device below exists to serve that one sentence — metrics carry a scale, statuses carry meaning, roadmaps run in lanes.

Apply its palette, grid, and motifs only when this exact guide is selected; do not borrow dashboard, landing-page, or keynote patterns into it. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is deliberately a tile, a status pill, or a lane bar — hierarchy comes from the grid and the type scale, not from wrapper shells. Read the general deck rules from the deck contract; they are not repeated here. This file covers only what is specific to tidemark-slate: its palette and the semantic sub-system inside it, its type pairing and scale, its page geometry, its three signature motifs, and the failure modes that destroy it.

## Style Summary

The anchor is a **tidemark stone** in a harbour basin — slate scoured by repeated rise and fall, tide marks cut into its face, three-colour channel buoys sitting off it. What is anchored is one idea: **a cut mark is where the water stood last time**. A review is not a recital of numbers; it is a report of position relative to the previous reading. Nothing figurative is anchored — no waves, no vessels, no lighthouses.

The ground is a cold white slate at oklch L 0.965, chroma 0.004, hue 240. That coldness is a decision, not a default: **any warm ground on a data page makes the numbers read as though they were packaged by an atmosphere.** A near-neutral cold ground is the only ground that does not interfere with numeric judgement.

Above it sits exactly one chromatic accent — tide blue at hue 235, chroma 0.115 — and a **separate semantic sub-system** of three buoy colours. The buoys are not part of the palette allocation. They do not compete for the "one primary, one secondary, one accent" budget; they are meaning, rendered as dots and pills, and nothing else.

Key aesthetics:

- **Cold slate ground**: `#F1F4F6`, hue 240 at chroma 0.004 — near-neutral by intent, never warm
- **One chromatic accent**: tide blue, for scale rules and the key chart series and nothing beyond
- **Semantics as a sub-system**: green / amber / red are status, never palette, never chart series
- **Every numeral is mono**: across tiles, tables, and axis labels, so columns align on their own
- **Scale over label**: distance to target is drawn as a position on a rule, not written as text
- **Shadow is a single hairline**, and only tiles are allowed to carry it
- **Radius 8, nowhere else**: the tile needs to detach from the ground without earning a border
- **Charts are one hue ramp**: no legend, no gridlines, no y-axis; values sit on the marks

## Color System

### Surfaces

| Token       | Value   | Usage                                                          |
| ----------- | ------- | -------------------------------------------------------------- |
| slate.wash  | #F1F4F6 | Page ground. oklch 0.965 0.004 240                              |
| slate.panel | #E4E8EB | Tile and card surface. oklch 0.930 0.006 240                    |
| slate.rule  | #CED1D4 | Dividers and table rules. oklch 0.860 0.006 240                 |

Three steps, all hue 240, chroma held at or below 0.006. The ramp is lightness only. A hue shift between two surfaces would read as two different papers on one page, and this deck's whole claim is that the page is one slab.

### Text Colors

| Token     | Value   | Usage                                                                    |
| --------- | ------- | ------------------------------------------------------------------------ |
| ink.slate | #19212A | Primary text. 14.71:1 on wash, 13.19:1 on panel                          |
| ink.soft  | #515961 | Secondary text. 6.44:1 on wash, 5.77:1 on panel                          |
| ink.faint | #737A81 | Labels, page numbers, quarter chips. 3.94:1 — **24px and above only**    |

ink.faint carries the same hue 250 as the other two and is legal only at 24px and up. Below that size it is not "quiet type", it is a contrast failure that a projector will finish off.

### Accent Colors

| Token           | Value   | Usage                                                                  |
| --------------- | ------- | ---------------------------------------------------------------------- |
| mark.tide       | #006C9B | **The single accent.** Scale rules, target marks, the key series. 5.25:1 |
| mark.tide.deep  | #004C77 | The blue block that carries white text. 8.27:1 with white               |
| mark.tide.wash  | #D6ECF9 | Non-key chart series and pale grounds. 13.33:1 with ink.slate           |

The three tide tones are one hue (235) at three lightnesses, and they are not interchangeable. `mark.tide` is for marks and rules sitting on the slate; `mark.tide.deep` is the only tone that may be filled behind white text; `mark.tide.wash` is what every non-key series in a chart becomes. A chart in this deck is a **single-hue ramp** — key bars in `mark.tide`, everything else in `mark.tide.wash`.

### Semantic Buoy Colors

| Token      | Value   | Usage                                    |
| ---------- | ------- | ---------------------------------------- |
| buoy.green | #35824B | Semantic: on track. 4.28:1               |
| buoy.amber | #AF7100 | Semantic: at risk. 3.67:1                |
| buoy.red   | #B24037 | Semantic: blocked. 5.16:1                |

**These three are a sub-system, not palette colours.** They take no part in the primary/secondary/accent allocation, they appear only as 12px dots or as pills, and they never become chart series colours. The reason is precise: the moment a green bar, an amber bar, and a red bar stand in one chart, the colours have been demoted from *meaning* to *category* — and categorical colour is the one thing this deck does not want, because its charts are single-ramp by design.

The amber was pulled down from L 0.660 to **L 0.600** to buy 3.67:1. A semantic colour that works on hue but not on lightness fails twice over: for colour-blind viewers, and for anyone in the room watching a washed-out projector.

## Typography

### Font Families

| Role             | Family                                          | Usage                                                   |
| ---------------- | ----------------------------------------------- | ------------------------------------------------------- |
| Display          | Familjen Grotesk, 思源黑体 Bold                   | Cover headline and page titles                          |
| Body             | Geist, 霞鹜新晰黑                                 | Body copy, table text, annotations, captions            |
| Numerals         | IBM Plex Mono                                   | **Every figure, everywhere, without exception**          |

Fallback stack: display `Noto Sans SC` 700 / body `Noto Sans SC` 400 / numerals `Inter` 500 with **tabular figures on**. If the render stack cannot switch on tabular figures, numeric column widths must be written as fixed values instead — an un-tabular numeral column is a broken column.

The Latin face is written **first** in every chain. Font matching is per-character: Latin faces carry no CJK codepoints, so Han characters fall through to the Chinese face automatically. Written the other way round, the Chinese face's own mediocre Latin swallows every digit and the paired Latin face never appears.

Forcing all numerals to mono is the cheapest and highest-yield credibility device in this deck. Figures line up across tiles, across table rows, and across axis labels without a single alignment rule being written.

### Type Scale

| Level             | Size     | Font     | Tracking | Usage                                            |
| ----------------- | -------- | -------- | -------- | ------------------------------------------------ |
| Cover headline    | 88px     | Display  | —        | Cover page only                                  |
| Page title        | 56px     | Display  | —        | Every interior page                              |
| Tile value        | 64px     | Numerals | —        | The actual value inside a tidemark tile          |
| Risk item title   | 36px     | Display  | —        | The heading of a risk entry                      |
| Resolution line   | 32px     | Body     | —        | One-sentence decision on the resolutions page    |
| Table header      | 28px     | Body     | —        | Column headers                                   |
| Table body        | 28px     | Body     | —        | Row text, impact lines, remediation lines        |
| Tile sub-row      | 26px     | Body     | —        | The line under a tile value                      |
| Annotation        | 26px     | Body     | —        | Notes, explanatory copy, source lines            |
| Tile label        | 24px     | Body     | +2 / 0   | Uppercase, above the tile value. +2 on Latin runs, **0 on CJK** |
| Page number       | 24px     | Numerals | —        | Page numbers and quarter chips                   |

**The page title is 56px, one step smaller than a comparable low-density deck sets it.** That is deliberate: at this density the title has to yield to the content. A 72px title on a 16-slot page is a title that has taken space away from the thing the page exists to show.

## Spacing System

### Page Margins

| Edge         | Value | Note                                                        |
| ------------ | ----- | ----------------------------------------------------------- |
| Top          | 88px  | Tightened for density                                       |
| Bottom       | 104px | Larger than the top; the source line lives here             |
| Left / Right | 96px  | Never below the 72px soft floor                             |

### Column Grid

**12 columns × 122 + 11 gutters × 24 = 1728.** That 1728 is the content width, and with 96px side margins it lands the deck on a 1920 canvas exactly. Every structural block — tile row, table, lane chart, annotation rail — snaps to this grid.

The 440px annotation rail on the trend page is a grid citizen: it sits at the right of the content width and the chart takes what remains.

### Gap Scale

The breathing unit for this deck is **16**, and every gap is an integer multiple of it. This is one step tighter than a low-density deck's unit, which is the whole point — high density is achieved by shrinking the rhythm, never by shrinking the type.

## Corner Radius

| Value | Usage                                                                  |
| ----- | ---------------------------------------------------------------------- |
| 8px   | Tidemark tiles, lane bars                                              |
| 14px  | Buoy pills only — half of the 28px pill height                          |
| 0px   | Everything else: tables, rules, dividers, charts, sections, page grounds |

8 is the **only** radius this deck introduces, and it exists for one reason: a tile has to separate from the ground without earning a border. Radius anywhere else is radius creep, and radius creep is what turns a review deck into a product page.

## Shadow

One shadow exists in the entire deck: `0 1px 2px rgba(20,32,45,.06)`, single layer, **tiles only**. Here a shadow is *readable lift*, not decoration. A second layer, a larger blur, or a shadow on any non-tile element is a defect.

## Signature Motifs

### 1. Tidemark tile — `tidemark-tile`

The core device, and what separates this deck from a generic KPI card. Four stacked layers inside one `slate.panel` block at radius 8:

1. **Label** — 24px, uppercase, letterSpacing +2 for Latin (**0 for a CJK label**), `ink.faint`
2. **Value** — mono 64px, `ink.slate`
3. **Scale rule** — a 2px horizontal rule spanning the full tile width, with a **3px vertical marker in `mark.tide`** placed at the target's position along that rule
4. **Delta chip** — a `buoy.*` dot, a mono figure, and a comparison word

Layer 3 is the whole idea. **The target is drawn as a position on a scale, not written as "Target: X".** Written as text, distance-to-target must be computed by the reader; drawn as a mark, it is simply seen. A tile that states its target in words has been rebuilt as a KPI card and no longer belongs to this deck.

Tile rows are **fixed at four tiles**. Three leaves an awkward gap; five squeezes the mono 64 value into a wrap.

### 2. Buoy dot — `buoy-dot`

A strictly semantic marker. Two forms only:

- **Dot** — 12px circle in `buoy.green` / `buoy.amber` / `buoy.red`
- **Pill** — height 36, radius 18, horizontal padding 16, same three fills. The height is set by the label, not the other way round: 24px CJK at 1.4 needs a ~34px line box, so a 28px pill could only fit by shrinking the type — which is the one thing a deck may never do

The hard rule, not a suggestion: **any row carrying a buoy dot must also carry an owner column and a due-date column.** A status with no owner and no date is an ownerless status, which is worse than no status at all. If either column cannot be filled, delete the dot.

### 3. Tide lane — `tide-lane`

The roadmap device.

- **Lane name column**: 200px fixed, on the left
- **Time region**: the remainder, with months separated by 1px dashed `slate.rule` lines
- **Bar**: height 32, radius 8, filled `mark.tide.wash`, with a **6px solid `mark.tide` segment at its left end**
- **Milestone**: a 16×16 square rotated 45°, filled `mark.tide.deep`

The milestone is a rotated square rather than a dot on purpose. **A milestone and a status marker must be distinguishable by shape, not by colour alone** — colour-only differentiation fails on a dim projector and fails for colour-blind viewers.

## Page Inventory

| #  | Page type       | Density     | Slot cap | Structure                                                                                          |
| -- | --------------- | ----------- | -------- | -------------------------------------------------------------------------------------------------- |
| 01 | Cover           | low         | 5        | 88px headline + mono period chip + one scope line + presenter/date                                 |
| 02 | Tile panorama   | high        | **16**   | 56px conclusion title + a row of 4 tidemark tiles (4 slots each) + one source line                 |
| 03 | Trend           | medium-high | 8        | 56px title + single-ramp bar chart (key `mark.tide`, rest `mark.tide.wash`) + 440px annotation rail, 2 groups |
| 04 | Detail table    | high        | **14**   | 56px title + 6–8 table rows (buoy column + owner column + due column) + source                     |
| 05 | Risks           | medium-high | 9        | 56px title + 3 risk entries (buoy dot + 36px title + 28px impact + 28px remediation), 1px `slate.rule` between entries |
| 06 | Lane roadmap    | medium-high | 10       | 56px title + tide lanes (3–4 lanes × one quarter) + one legend line                                |
| 07 | Resolutions     | medium      | 6        | 56px title + 3 pending decisions (32px decision + mono decider + mono deadline) + a closing line   |

## Strictly Avoid

1. **The three buoy colours never become chart series colours.** Dots and pills only; charts stay single-ramp.
2. **No gridlines, no legend, no y-axis ticks.** Values are labelled at the end of the bar.
3. **No pie charts, no donut charts, no 3D, no gradient bars, no shadowed bars.**
4. **Nothing but a tile carries a shadow.** In this deck shadow is readable lift, not ornament.
5. **A row with a buoy dot must have an owner and a due date.** Missing either — delete the dot.
6. **Page titles state a conclusion, not a topic.** "Adoption is up but two integrations are dragging", not "Usage update".
7. **No second non-semantic chromatic colour.** Beyond tide blue there are neutrals and the three semantic tones, nothing else.
8. **Numerals never use the body face.** One numeral set in a proportional face voids the column-alignment promise for the entire deck.
9. **No two-colour quarter-over-quarter bar comparison.** The comparison runs as a grey ghost bar, never as a second chromatic colour.
10. **Tile rows are fixed at four.** Not three, not five.

## Anti-Patterns

- **A warm or cream ground.** Even a slight warm cast reframes the numbers as something curated for mood. The ground is hue 240 at chroma 0.004 and stays there.
- **Rebuilding the tile as a KPI card.** Dropping the scale rule and writing "vs target" as a text line is the single most common way this deck collapses into a generic dashboard slide.
- **Radius creep.** Radius 8 is licensed for tiles and lane bars. Once table cells, section blocks, or chart bars pick it up, the deck stops reading as a cut slab.
- **Distinguishing a milestone from a status by colour.** They must differ in shape; a `mark.tide.deep` dot and a `buoy.*` dot are the same object to half the room.
- **A number without a source.** Any page carrying a figure carries a source line. If the source cannot be written, the figure should not be on the page.
