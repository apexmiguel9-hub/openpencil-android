---
name: 'gridpaper-graphite'
tags: [light-mode, education, data-focused, cjk-type, monospace, dual-font, swiss, minimal, sharp-corners, austere, crisp, blue-accent]
platform: slides
---

## Style Scope

This guide is self-contained and written for the **academic defence deck** — a presented slide sequence whose job is to let an audience check an argument step by step. Every page carries one claim, one exhibit, and one so-what. It is not a pitch deck, not a keynote, not a report laid out sideways. Apply its palette, grid, and motifs only when this guide is selected; do not borrow dashboard, landing-page, or marketing-deck patterns into it. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is deliberately an index tab, a note panel, or a table band — hierarchy here comes from the lattice and from type size, never from a wrapper shell. General deck law — overflow splits pages instead of shrinking type, the density-slot budget, the locked/variable split, the narrative arc, the ghost-deck test, the AI-slop checklist — lives in the deck contract and is **not** restated here.

Formality is high, density is medium-high, ground is light.

## Style Summary

The anchor is a **lab notebook**: an almost-invisible printed grid, the cool grey of a 2H pencil, and three coloured index tabs stuck along the page edge. What is being anchored is not a discipline — no beakers, no molecular diagrams, no gears — it is the single idea that **the process was written down, therefore it can be re-checked**.

Three arguments fix the system:

**One — the grid line sits at L0.925, only 0.05 below the L0.975 ground.** The grid's function is to make the audience believe the elements are aligned. The moment it can be actively read it stops doing that job and becomes decorative noise. It must be there and lose every competition.

**Two — the three index colours are semantic slots, not a colour scheme.** Blue means *definition / method*. Green means *example / result*. Amber means *limitation / open question*. A slide's colour is decided by what the slide is claiming, never by what looks good next to the previous slide. Amber is pushed down to L0.585 to reach **3.97** contrast, because these labels routinely appear at 20–24px and have to hold at that size.

**Three — body text is a chroma-carrying cool grey, not pure black.** `#1F2329` at chroma 0.012, hue 250, so it reads as graphite catching light rather than ink. That single move is the whole distance between this style and a white-background black-text conference template — and it is enough.

Key aesthetics:

- **Paper ground with a printed grid**: `#F4F8F8`, grid lines `#E0E8E9` on a 48px square lattice
- **Everything lands on the 48 grid**: alignment here is meant to be *visible* — that is where the credibility comes from
- **Zero corner radius everywhere**: nothing is rounded, nothing is soft
- **Graphite, not black**: a cool neutral ramp at hue 250 against a hue-200 ground
- **Three index colours as fixed semantics**: blue = definition, green = result, amber = limitation
- **Every exhibit is paired with its so-what**: an unpaired figure does not get on a page
- **Every page with data, a figure, or someone else's conclusion carries a citation gutter**: an empty gutter is a defect
- **Numerals and tabs replace bullets**: there is no bullet glyph in this system

## Color System

### Core Backgrounds

| Token      | Value   | Usage                                                          |
| ---------- | ------- | -------------------------------------------------------------- |
| grid.paper | #F4F8F8 | Page ground. oklch 0.975 0.004 200 — the notebook sheet          |
| grid.line  | #E0E8E9 | The printed grid, 48px pitch. oklch 0.925 0.008 210              |
| panel.note | #E8EEEF | Exhibit ground and pull-quote blocks. Graphite on it reads 13.46 |

Three steps and no more. `panel.note` is the only raised surface in the system; a panel on a panel does not exist here, because a notebook has one sheet.

### Border Colors

| Token     | Value   | Usage                                                       |
| --------- | ------- | ----------------------------------------------------------- |
| rule.hair | #CBD0D1 | Dividers, table row rules, the citation-gutter hairline      |

One border colour, drawn at 1px. The only 2px rule in the system is the `index.blue` line under a table header or above a so-what line — and it is an accent, not a border.

### Text Colors

| Token          | Value   | Usage                                                     |
| -------------- | ------- | --------------------------------------------------------- |
| graphite       | #1F2329 | Claim titles and body copy. 14.75:1 on the ground          |
| graphite.soft  | #575B60 | Secondary copy, exhibit sub-labels. 6.39:1                 |
| graphite.faint | #797E83 | Citations and page numbers. 3.83:1 — **≥24px only**        |

All three sit at hue 250 with chroma 0.010–0.012 against a hue-200 ground. The neutrals lean colder than the paper on purpose: that is the pencil reading against the sheet.

`graphite.faint` is below 4.5:1 and is therefore restricted by size, not by taste. It may carry citation lines and page numbers at 24px and nothing else. It never carries a sentence a reader has to parse.

### Index Colors

| Token       | Value   | Usage                                                                   |
| ----------- | ------- | ----------------------------------------------------------------------- |
| index.blue  | #1D6294 | Definition / method. Also the deck's single accent. 6.08:1 both ways     |
| index.green | #33724C | Example / result. 5.37:1                                                 |
| index.amber | #A17221 | Limitation / open question. 3.97:1 — labels at 20–24px                   |

`index.blue` is the only colour that also works as a general accent: the cover rule, the 2px table-header underline, the so-what top line. It is reversible — `grid.paper` type on an `index.blue` fill is the same 6.08:1 as the reverse, which is why the tab tongue can be filled solid and still carry a label.

Green and amber are **never** general accents. They appear only where their semantic slot applies: green on a results block, amber on a limitations block. A green heading on a methods page is a bug, not a variation.

## Typography

### Font Families

| Role            | Family                                       | Fallback         | Usage                                            |
| --------------- | -------------------------------------------- | ---------------- | ------------------------------------------------ |
| Display         | 思源黑体 Bold, Geist                          | Noto Sans SC 700 | Cover title, claim titles, exhibit titles         |
| Body            | 霞鹜新晰黑, Geist                              | Noto Sans SC 400 | Body copy, captions, sub-labels                   |
| Data / Citation | IBM Plex Mono                                 | Inter 400        | Formulas, figures, step numerals, citation lines  |
| Quotation       | Source Serif 4                                | —                | Latin pull quotes only                            |

The Latin face is written **first** in every fallback chain. Font matching is per-character: Latin faces carry no CJK codepoints, so Han characters fall through to the Chinese face automatically. Written the other way round, the Chinese face's own Latin swallows every digit and the paired Latin face never appears — which matters more here than in most styles, because this deck is full of numerals.

### Type Scale

| Level          | Size | Font    | Line Height | Usage                                            |
| -------------- | ---- | ------- | ----------- | ------------------------------------------------ |
| Cover Title    | 84px | Display | —           | Cover only, once per deck                        |
| Claim Title    | 60px | Display | —           | The page title, written as a claim. **≤2 lines** |
| Exhibit Title  | 36px | Display | —           | Exhibit headings, numbered step headings         |
| Body           | 30px | Body    | **1.7**     | **Default body copy — and the floor**            |
| Caption        | 26px | Body    | —           | Figure captions, table notes                     |
| Citation       | 24px | Mono    | —           | Citation-gutter lines, in-text superscripts       |
| Page Number    | 24px | Mono    | —           | Page numbers                                     |

The page title is a **claim sentence**, not a topic label. "Method" is a topic; "A two-stage sampler removes the bias without extra passes" is a claim. Titles that are topics defeat the entire deck.

Body at 30px is lower than most decks go, and it is deliberate: this style has to fit citations on the page. **30 is the floor, not the target.** If content does not fit at 30, delete content — do not go to 28.

Four roles, and each has one job. Display carries titles. Body carries prose. Mono carries anything that is a number, a step index, or a reference — that is what makes the citation gutter read as apparatus rather than as more text. Source Serif 4 appears for Latin pull quotes and nowhere else.

### Line Height

Body runs at **1.7**. Han glyphs fill the em box top to bottom, so Chinese needs roughly 0.2 more leading than the Latin equivalent; at 1.5 the citation-dense body of this deck reads as a solid grey block.

## Spacing System

### Layout Grid

- Margins: **top 96, bottom 112, left 112, right 128**. The bottom is heavier because the citation gutter lives there; the sides are deliberately unequal so the 1680 content width divides by the 48 lattice exactly (48 × 35), and a page that is not centred between equal margins cannot read as the centred-and-equidistant AI default
- Grid: **48px square**. Every element origin and every element edge lands on a multiple of 48
- Columns: **12 × 118 with 11 gutters of 24 = 1680** content width
- Base gap: **24**

The alignment in this style is meant to be seen. "Close enough" is not a tolerance — an element off the 48 lattice is a defect on the same footing as a typo, because the visible regularity of the grid is what makes the argument look checkable.

The base gap of 24 is exactly half the 48 lattice pitch and exactly the column gutter, so stacks built from 24 and 48 resolve back onto the lattice on their own. Anything that needs a gap the lattice cannot express is the wrong structure, not a spacing problem.

The one sanctioned exception is the index tab, which bleeds 24px *outside* the left margin. That break is the point of the motif (see below).

## Corner Radius

| Value | Usage      |
| ----- | ---------- |
| 0px   | Everything |

Panels, tabs, tables, exhibit frames, the cover rule: all square. The tab tongue's one non-square edge is an **8px bevel**, a cut corner — not a radius. Nothing in this deck is a capsule, a pill, or a rounded card.

## Signature Motifs

**1. Index tab tongue — `index-tab`.** A filled rectangle pinned to a content block's **top-left corner and bled 24px to the left of it**. Height **32**. The left end is cut back by an **8px bevel** (a chamfer, never a radius). Fill is one of the three index colours; inside sits a **20px uppercase label** in `grid.paper`, and the tongue's width hugs that label. Semantics are fixed and non-negotiable: `index.blue` = definition / method, `index.green` = result / example, `index.amber` = limitation / open question.

The leftward bleed is the whole trick. It makes the tab read as something *stuck on* rather than *laid out into*, and it deliberately breaks the otherwise uniform left margin — which is exactly why it reads as a physical tab and not as a coloured heading.

**2. Exhibit couplet — `exhibit-couplet`.** An exhibit and its so-what are one unit, never separable. Directly beneath the exhibit runs a **2px `index.blue` top line**; beneath that line sits a **30px conclusion sentence** — not a figure caption, but a statement of *what this exhibit changes*.

Two rules govern it, and they cut both ways. An exhibit with no so-what does not get on a page. And if the sentence still stands with the exhibit covered up, the exhibit is redundant — delete the figure, keep the sentence.

**3. Citation gutter — `citation-gutter`.** At the page bottom: a **1px `rule.hair`** hairline, and below it a **40px band** carrying a **mono 24px** citation in the form `[3] Author, Year, Journal`. In-body references are superscript numerals pointing into it.

Any page carrying data, a figure, or somebody else's conclusion **must** have a filled gutter. An empty gutter on such a page is a defect, and "Source: internal" is not a citation.

## Page Plan

Eight pages, deliberately. Fifteen would force a section-navigation layer, and this deck is sized to never need one.

| #  | Page                     | Structure                                                                                             |
| -- | ------------------------ | ----------------------------------------------------------------------------------------------------- |
| 01 | Cover                    | Title 84 + subtitle 30 + author / institution / date in mono + one `index.blue` rule, 48 wide          |
| 02 | Problem and gap          | Claim title + three numbered parts (state of the art / gap / this work) at 36 + 30, plus gutter        |
| 03 | Method                   | Claim title + `index.blue` tab on the method block + 3–4 steps (mono numeral + 36 + 30), plus gutter   |
| 04 | Result, figure and read  | Claim title + **exhibit in the left 7 columns, reading in the right 5** + couplet + gutter             |
| 05 | Result, table            | Claim title + table (2px `index.blue` header underline; 1px `rule.hair` rows; no rule on the last row) |
| 06 | Limitations              | Claim title + `index.amber` tab on three limitations (36 + 30), plus gutter                            |
| 07 | Conclusions              | Title reads **"Conclusions", never "Thank you"** — three one-sentence conclusions at 36 + one 30 line  |
| 08 | References               | Full-page mono 24 reference list of 8–12 entries + contact line in the footer                          |

Evidence precedes interpretation on page 04 because the eye runs left to right; flipping the columns tells the audience the conclusion was decided before the data.

## Icons

This style has no icon layer. Hierarchy is carried by mono numerals, index tabs, and rules. Where another style would place a glyph, place a numbered step or a tab.

## Anti-Patterns

- **The closing page is never "Thank You" and never blank.** It stays on screen for the entire Q&A; that time belongs to the conclusions.
- **No exhibit without a so-what**, and no exhibit that survives its own removal.
- **No gradients, no shadows, no rounded corners, no decorative icons, no photographs, no script or novelty faces.**
- **Index colours are not a palette.** Never as chart series colours, never as a large background fill, never as text colour — the label inside a tab tongue is the only exception.
- **Never off the 48 lattice.** "Looks about right" is not accepted here.
- **Never below 30px body.** Cut content instead.
- **No bullet glyphs.** Levels come from mono numerals and tab tongues.
- **Charts for trend, comparison, and distribution; tables only for exact value comparison** — and never both on one page.
- **No unverifiable sourcing.** The citation gutter cannot be omitted and cannot be filled with "Source: internal data".
- **No section-navigation layer.** Past 15 pages a deck needs one; this style stays at 8 precisely so it never does.
