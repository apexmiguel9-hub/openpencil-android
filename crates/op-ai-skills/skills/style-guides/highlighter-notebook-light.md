---
name: 'highlighter-notebook-light'
tags: [social-card, card-series, cjk-type, vertical-portrait, light-mode, education, friendly, clean, display, soft-corners, crisp, data-focused]
platform: card
---

## Style Scope

This guide is self-contained and written for **Chinese social cards** — a swipeable series of fixed-canvas pages read on a phone, not a scrolling page and not an app screen. Apply its palette, paper language, spacing, and type treatments only when this exact guide is selected; do not borrow SaaS, dashboard, wellness, or e-commerce patterns into it. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is deliberately a highlight band, a sticky note, a grid panel, or a margin rule.

## Style Summary

Somebody's revision notes, photographed well. The page is `#FAFAF8` — copier white with a trace of warmth — carried by a faint cross-dot grid and a single red margin rule down the left. The writing colour is `#212939`, a blue-black, because a ballpoint's ink is blue-black and that is the tell that separates "handwritten notes" from "a printed handout". Nothing in the system is pure black and nothing is pure white.

The signature device is the **highlight band**, and it is defined by two constraints that most treatments get wrong. First, the fluorescent colours are pulled *down* to chroma 0.06-0.145 rather than up to screen-fluorescent saturation, because a real highlighter is translucent and the paper dilutes it — an undiluted highlight reads as a colour block, not as a pen stroke. Second, **the band is 55% of the line height, sitting from 45% above the baseline**, and it wraps only the highlighted characters plus 8px each side. A band that fills the full line height and the full column width has stopped being a highlighter and become a background.

Everything else is study-desk furniture used sparingly: a sticky note with one curled corner, a red-pen circle, a cross-dot grid at 48px pitch. The style is built for the highest-save-rate content — checklists, frameworks, step sequences, numbers — so its density ceiling is higher than the other light themes, and its ranking device is size and highlight rather than colour variety.

Key aesthetics:

- **Copier white, not pure white**: `#FAFAF8`, with a cross-dot grid at 48px pitch
- **Blue-black ink, not black**: `#212939` — the ballpoint tell
- **Diluted fluorescents**: chroma 0.06-0.145, because paper dilutes a translucent marker
- **The band is 55% of line height**, from 45% above baseline, hugging the words plus 8px
- **Two highlight colours per card, maximum**
- **Red pen circles and annotates, never sets text**: 8 characters or fewer
- **One sticky note per card**, with one curled corner and the only shadow in the system
- **Margin rule at one column in from the left**, the full height of the card
- **Rank by size and highlight, never by adding colours**

## Color System

### Core Backgrounds

| Token           | Value   | Usage                                                             |
| --------------- | ------- | ----------------------------------------------------------------- |
| Page Background | #FAFAF8 | The copier sheet. Every card's ground; never a panel colour         |
| Card Surface    | #EFF2F4 | Grid panel — the standard block for a grouped set of rows           |
| Grid Line       | #D6DFE6 | The cross-dot grid and table rules                                  |
| Sticky Surface  | #F7E8AF | The sticky note. One per card                                       |

### Text Colors

| Token          | Value   | Usage                                                              |
| -------------- | ------- | ------------------------------------------------------------------ |
| Primary Text   | #212939 | Headlines and body copy. 13.95:1 on the sheet                      |
| Secondary Text | #545B6A | Deck lines, annotations, list sub-copy. 6.52:1                     |
| Muted Text     | #69707F | Sources, page numbers, corner marks. 4.76:1                        |
| On Highlight   | #212939 | Text sitting on a highlight band — 9.5:1 or better on all three    |

The ink ramp carries a +265° blue cast at chroma 0.022-0.032. Flattening it to neutral grey turns the whole style into a printed worksheet, which is the opposite of what it is for.

### Border Colors

| Token           | Value   | Usage                                                                    |
| --------------- | ------- | ------------------------------------------------------------------------ |
| Default Border  | #D6DFE6 | Grid lines, table rules, panel edges. **Never carries text** (1.6:1)      |
| Margin Rule     | #E9B5B4 | The single red rule down the left margin, 2px, full card height           |

### Accent Colors

| Token            | Value   | Usage                                                             |
| ---------------- | ------- | ----------------------------------------------------------------- |
| Primary Accent   | #FBE76B | Highlight yellow. Carries ink at 11.62:1                          |
| Highlight Mint   | #A5F3C0 | Highlight mint. Carries ink at 11.23:1                            |
| Highlight Pink   | #F8C5CD | Highlight pink. Carries ink at 9.62:1                             |
| Red Pen          | #C33939 | Circles, underlines, annotations of 8 characters or fewer (5.07:1) |

**How to use the highlights.** A band is a `rect` behind a text run: height = 55% of the line height, top edge = 45% of the line height above the baseline, width = the run's measured width plus 8px on each side, radius 4px. It never spans the full line, never spans the full column, and never sits behind a whole paragraph.

**Two highlight colours per card, maximum**, and each one marks at most two runs. Three colours on one page turns a note into a rainbow and the reader stops knowing which mark matters.

Red pen is a different instrument, not a fourth highlight. It draws circles and underlines and writes short annotations; it never sets a headline, never sets body copy, and never fills anything.

**Gradients**: none. Highlighter ink on paper is flat; the only depth in the whole system is the sticky note's shadow.

## Typography

### Font Families

| Role             | Family                                        | Range         | Usage                                                   |
| ---------------- | --------------------------------------------- | ------------- | ------------------------------------------------------- |
| Display          | Familjen Grotesk, Noto Sans SC Heavy          | 88px and up   | Every headline, big numeral, and framework label         |
| Body / Interface | Geist, LXGW Neo XiHei, Noto Sans SC           | 64px and down | Body, deck lines, list rows, captions, page numbers      |
| Data / Numerals  | Geist                                         | any           | Every figure, with tabular-nums on                       |

LXGW Neo XiHei is the body face rather than 思源黑体 on purpose: it is lighter and more open on screen, and this style's content is list-dense, so the body face is doing more work than usual. All faces named are OFL.

The Latin face leads every fallback chain so digits and Latin words are set by Geist rather than by the Chinese face's own Latin. Because this style carries checklists and metrics, `tabular-nums` is mandatory on every numeric run — without it 1 and 8 have different widths and a column of figures visibly shivers.

### Type Scale

| Level      | Size  | Font    | Weight | Tracking | Line Height | Usage                                     |
| ---------- | ----- | ------- | ------ | -------- | ----------- | ----------------------------------------- |
| Display XL | 168px | Display | 900    | -0.01em  | 1.05        | Cover headline, 2-6 characters            |
| Display L  | 120px | Display | 900    | -0.01em  | 1.10        | Cover headline, big numeral               |
| Display    | 88px  | Display | 900    | 0        | 1.15        | Interior headline, framework label        |
| Title 1    | 64px  | Body    | 700    | 0        | 1.25        | Page title                                |
| Title 2    | 48px  | Body    | 600    | 0        | 1.30        | List-item title, table header             |
| Body L     | 40px  | Body    | 400    | 0.02em   | 1.70        | **Default body copy**                     |
| Body       | 36px  | Body    | 400    | 0.02em   | 1.70        | Body floor — never below                  |
| Caption    | 32px  | Body    | 400    | 0.02em   | 1.50        | Sources, page numbers, annotations        |

**At most four of these eight on one card.** This style's content is the densest of the four card themes, which makes the four-tier cap more important here, not less — density is carried by the list structure, never by adding type sizes.

### Font Weights

| Weight   | Value | Usage                                                       |
| -------- | ----- | ----------------------------------------------------------- |
| Regular  | 400   | All body copy and captions                                   |
| Semibold | 600   | List-item titles and table headers                           |
| Bold     | 700   | Page titles                                                  |
| Heavy    | 900   | Display lines only — the whole style's contrast lives here    |

The display tier at 900 against body at 400 is the primary hierarchy device, and it costs nothing to load in a variable face. No italic on Han characters; emphasis is weight, a highlight band, or a red-pen underline.

### Line Height

- Display (88-168px): 1.05-1.15
- Titles (48-64px): 1.25-1.30
- Body (36-40px): **1.70** — Chinese needs roughly 0.2 more leading than the Latin default, and here it also leaves room for the highlight band to sit without touching the line above
- Captions: 1.50

### Letter Spacing

Body 0.02em; titles 0; display -0.01em. Latin all-caps labels +0.10em. Never apply Latin display tracking of -0.05em to Han text.

### Line Length

Body lines cap at **22 Han characters**. At 40px on the 920px column that falls out naturally; at 36px, narrow the measure to 10 of 12 columns.

## Spacing System

### Gap Scale

| Value | Usage                                                                     |
| ----- | ------------------------------------------------------------------------- |
| 8px   | The baseline unit; also the padding each side of a highlight band          |
| 16px  | Checkbox to label, icon to label                                          |
| 24px  | Between a list number and its text                                        |
| 32px  | Between the lines *within* one list item                                   |
| 72px  | Between list items — **must be at least 2× the intra-item gap**, or the list reads as one block |
| 48px  | Between a headline and its deck line                                      |
| 120px | Between the headline block and the content block                          |

### Padding Scale

| Value     | Usage                                                            |
| --------- | ---------------------------------------------------------------- |
| [12, 20]  | Small tags and checkbox cells                                    |
| 32px      | Table cells                                                      |
| 40px      | Grid panels                                                      |
| 48px      | The sticky note                                                  |
| [96, 80]  | The card — 96 top, 80 sides                                      |
| 128px     | Card bottom, larger than the top so feed chrome misses the last line |

### Layout Pattern

- Canvas 1080 × 1440 (3:4). Content column 920: 12 columns of 62 with 16 gutters
- Margin rule: 2px in Margin Rule colour, one column in from the left edge, full card height
- Grid: 4×4 dots in Grid Line at 48px pitch, behind everything, never over text
- Cover: heavy display headline, one highlighted run inside it, deck line, one sticky note
- Lists: 3-7 items; one marker type per card — numbers **or** checkboxes **or** timeline nodes, never a mixture
- Sticky note: one per card, 2° rotation or less, one curled corner path, a 0/4/12 shadow at 8% — the only shadow in the system
- Page number: bottom-right, Caption size, Muted Text

## Corner Radius

| Value | Usage                                           | Rationale                                          |
| ----- | ----------------------------------------------- | -------------------------------------------------- |
| 4px   | Highlight bands, grid dots, micro tags           | A marker stroke has a barely-rounded end            |
| 8px   | Checkbox squares, table cells                    | Nearly square                                       |
| 16px  | Button radius                                    | The interactive floor                               |
| 20px  | Card radius — grid panels and the sticky note    | The signature container radius                      |

Nothing is a full capsule. A pill in a notebook is a UI element that wandered in from another product.

## Icons

### Icon Font

- **Family**: Lucide
- **Style**: Outline, 2px stroke, round caps

### Commonly Used Icons

check, square, circle, arrow-right, arrow-up-right, chevron-right, pencil, bookmark, sticky-note, list, hash, trending-up, trending-down, alert-triangle, star

### Icon Sizes

| Size | Usage                                    |
| ---- | ---------------------------------------- |
| 32px | Beside Caption text                      |
| 40px | Beside Body text, checkboxes, list markers |
| 56px | Section markers                          |

### Icon Color States

| State     | Color   | Usage                                    |
| --------- | ------- | ---------------------------------------- |
| Primary   | #212939 | Default on the sheet and on panels       |
| Muted     | #69707F | Decorative and inactive marks            |
| Accent    | #C33939 | The single red-pen marker per card       |

## Anti-Patterns

- **The band never fills the line.** Full-line-height or full-column-width highlight bands are the single most common failure of this style; they turn a pen mark into a background wash.
- **Never saturate the fluorescents.** Screen-fluorescent chroma above 0.20 makes the palette read as a candy UI, not as marker on paper.
- **No handwriting face for body copy.** A script face may annotate at 32px or below; it may never set a paragraph.
- **Red pen never sets text longer than 8 characters** and never fills a shape.
- **One marker type per card.** Numbers, checkboxes, and timeline nodes are three different systems; mixing two on one page destroys the scan pattern that makes this style worth saving.
- **No pure black, no pure white.**
- **No shadow anywhere except the one sticky note.**
- **No third highlight colour**, and no more than two marked runs per colour.
