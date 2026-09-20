---
name: 'sounding-navy'
tags: [light-mode, high-contrast, corporate, enterprise, data-focused, cjk-type, monospace, blue-accent, sharp-corners, flat, stroke-based, austere]
platform: slides
---

## Style Scope

This guide is self-contained and written for **Chinese consulting / strategy decks** — high formality, medium density, a **mixed** ground structure where dark pages bookend a light body. Apply its palette, type pairing, layout grammar, and motifs only when this exact guide is selected; do not borrow dashboard, landing-page, or social-card patterns into it. The generic deck laws (overflow splits into a new page rather than shrinking type, the density-slot table, the locked-vs-free split, narrative arc, the ghost-deck test, the single-accent rule) live in the deck contract and are **not** restated here. What follows is only what makes this deck *this* deck.

Treat unnamed layout frames as structural by default: no fill, no stroke, no cornerRadius, no shadow. Separation comes from a 1px rule and from whitespace, never from a container shell.

## Style Summary

The anchor is a **nautical sounding chart** — the warm off-white of chart paper, the steel blue of depth contours, the ink navy of deep water, superscript numerals at every sounding point, and an ochre warning tint over the shallows. What is being anchored is not boats or anchors or compasses; it is one fact: **every number printed on a chart is a number somebody went down and measured.** That is exactly what a strategy deck has to transmit. A conclusion here is not an opinion, it is a reading.

Three constraints define the surface. **The dark ground is not black** — it is `#091E32`, navy carrying real chroma, so the steel accent on top of it reads as "the deep end of the same family" rather than "a blue thing stuck on black". **The light ground runs warm and the dark ground runs cold** — paper is warm, water is cold, and that split gives the mixed structure a genuine difference of material instead of two brightness steps of one colour. **The ochre is dark enough to be read** — it sits at 4.00:1 on paper, because a warning colour that cannot be read is a contradiction in terms.

Key aesthetics:

- **Chart paper, not white**: `#F7F4EE`, an anti-glare warm off-white; every body page sits on it
- **Navy with chroma as the dark ground**: cover, section, and closing pages only — never mid-deck
- **Zero corner radius, everywhere**: a sounding chart has no rounded corners
- **Three stroke weights only**: hair 1 / rule 2 / heavy 3 — there is no fourth
- **Numerals are a separate voice**: every figure is set in a mono face, 48-72px, and never in the body face
- **Conclusion-shaped titles**: a full sentence that states the finding, never a topic label
- **One warning per deck**: `#A46D1A` appears exactly once across all pages
- **Depth reads downward**: the signature chart hangs from a baseline instead of floating on one
- **No source, no number**: any page carrying a figure carries its provenance line

## Color System

### Grounds

| Token | Value | Usage |
| --- | --- | --- |
| chart.paper | #F7F4EE | Body-page ground. Every content page sits on this |
| chart.paper.deep | #EBE7DF | Zone surface / table zebra rows. Two authorised uses in the whole deck |
| deep.navy | #091E32 | Cover, section, and closing page ground |
| deep.navy.low | #031223 | A deeper block placed on a dark page |

The two grounds are deliberately on opposite thermal sides — paper carries a warm cast, navy a cold one. They are two materials, not two levels of one colour, and that is what lets the mixed structure work without reading as a theme toggle.

### Text Colors

| Token | Value | Usage |
| --- | --- | --- |
| ink.sounding | #1D2228 | Primary text on light pages. 14.58:1 on paper |
| ink.soft | #565B61 | Secondary text on light pages. 6.24:1 |
| ink.faint | #7C8186 | Footnotes, sources, page numbers. 3.58:1 — **legal at 24px and above only** |
| paper.on.navy | #F0F4F7 | Primary text on a dark page. 15.27:1 on navy |
| paper.dim.on.navy | #B3B8BE | Secondary text on a dark page. 8.46:1 |

`ink.faint` is the one token in this palette with a size condition attached. It exists for the source line and the page number and nothing else; using it at body size is a contrast defect, not a stylistic choice.

### Accent Colors

| Token | Value | Usage |
| --- | --- | --- |
| contour.steel | #266EA4 | **Primary accent**: depth contours, the key series, conclusion rules. 4.97:1 on paper |
| contour.deep | #044A7D | The steel block that carries white text. 8.39:1 with white |
| contour.lift | #83B0D7 | The accent form used on dark pages. 7.36:1 on navy |
| shoal.ochre | #A46D1A | Second colour — risk and warning. **Once per deck, total.** 4.00:1 |

Steel has three states and they are not interchangeable: `contour.steel` is for marks and type sitting on paper, `contour.deep` is for a filled block that carries white text, `contour.lift` is the only steel that reads on navy. Picking the wrong one is the fastest way to break this style — `contour.steel` on the navy ground barely separates from it.

The two chromatic hues sit 173° apart, far past the 60° separation floor, which is why one warning mark next to a field of steel reads instantly. Spending the ochre twice destroys that: a warning that occurs twice is no longer a warning.

### Rules

| Token | Value | Usage |
| --- | --- | --- |
| rule.chart | #D3D1CD | Column rules and table rules. **Never text** |

One rule colour, three weights, nothing else. If two regions need to be told apart, they are told apart by a `rule.chart` hairline and by space.

## Typography

### Font Families

| Role | Family | Usage |
| --- | --- | --- |
| Display | Noto Sans SC 700 | Cover title, page titles, closing line |
| Body | Noto Sans SC 400 | Lead-in sentences, body copy, table body, annotations |
| Numerals | Inter 500 | Every figure, readout, and index number |

These are the **production floor** — the layer that renders today, and the layer every shipped page uses. The preferred layer, for when the faces are available, is 思源宋体 Semibold for display (a conclusion-shaped title wants the settled weight of a serif), 霞鹜新晰黑 (OFL) for body, and IBM Plex Mono for figures; the Latin companions are `Source Serif 4` and `Geist`.

Fallback chains, Latin face written first in every one:

- Display — `"Source Serif 4","思源宋体","Noto Serif SC",serif`
- Body — `"Geist","LXGW Neo XiHei","Noto Sans SC",sans-serif`
- Numerals — `"IBM Plex Mono",monospace`

Latin faces carry no Han codepoints, so Chinese characters fall through to the CJK face per character automatically. Written the other way round, the CJK face's own mediocre Latin swallows every digit and the paired Latin face never appears.

### Type Scale

| Level | Size | Font | Usage |
| --- | --- | --- | --- |
| Cover title | 100-116px | Display | Cover only, 2 lines maximum |
| Closing line | 72px | Display | The closing page's settling sentence, 2 lines maximum |
| Page title | 64px | Display | The conclusion sentence on every body page, 2 lines maximum |
| Argument title | 40px | Display | The heading of a numbered argument |
| Lead-in | 36px | Body | The sentence under a page title; also annotation-column headings |
| Body | 32px | Body | Body copy, argument detail, action lines, readout conclusion |
| Table body | 30px | Body | Table cells; also annotation-column detail |
| Source | 24px | Body | Footnotes, sources, page numbers — **the only place 24px is permitted** |

### Numerals

Figures are set in the mono face at **48-72px** and are the second-loudest thing on any page after the title. They are never set in the body face, never set at body size, and never wrapped in a coloured pill. The readout numeral at the foot of a page is 48px; a page whose whole point is one number may take it to 72px.

## Layout Grammar

- **Margins**: top 96 / bottom 120 / left 120 / right 120. The bottom is deliberately larger than the top — in a projected room the lower edge of the screen is the part heads block. This is a physical constraint, not a taste.
- **Grid**: 12 columns × 118 + 11 gutters × 24 = **1680px content width**, which with the 120px side margins fills the stage exactly.
- **Corner radius**: **0**, on every node, without exception. There is no radius scale in this style.
- **Stroke weights**: hair **1** (table rules, profile connectors, hollow index dots at 1.5) / rule **2** (track line, table-header underline) / heavy **3** (the readout's top rule). Nothing takes a fourth weight.
- **Gap**: multiples of **24** only.
- **Asymmetry**: the two-column body page splits **7:3** — evidence left, judgement right — divided by a 1px `rule.chart` seam, with the narrow column filled `chart.paper.deep`.
- **Annotation column**: a fixed **480px** rail on the right of a chart page, carrying at most two heading + detail pairs.

## Signature Motifs

These three carry the deck's recognisability. Each is described so it can be built out of primitives.

### 1. Depth profile `depth-profile`

The original expression for "we started here, ended there, and here is what ate the difference".

Draw a horizontal 2px `rule.chart` baseline across the plot area at the **top**. Every bar is a rectangle whose top edge sits on that baseline and which extends **downward** — the bars hang, they do not float. Under the lower end of each bar, place its value as a mono numeral. Fill by role: decrements `ink.faint` neutral grey, increments `contour.steel`, the opening and closing totals `contour.deep` solid. Decrements do NOT take the ochre: a warning colour that shows up three times on an evidence page is no longer a warning, and the deck's single ochre is spent on the trade-off page. Draw no connector between bar tops — the bars' own height difference already states the relationship, and a connector at that height runs straight through the value labels.

*The divergence from the conventional floating waterfall is deliberate.* A floating bar reads as a **position**; a hanging bar reads as a **depth**, which runs in the same direction as loss and consumption. It also leaves the baseline as the single alignment anchor on the page.

### 2. Track index `track-index`

A section page opens with a 2px `rule.chart` line running the full content width near the top — the track. On it, place N dots of **12px** at equal intervals. The dot for the current section is filled `contour.steel` and carries the section's Chinese name **24px below it**; every other dot is hollow, 1.5px stroke, with no label at all.

The track is redrawn on **every** section page with only the filled dot moving. That repetition is what turns it from an ornament into a cross-page progress anchor.

### 3. Sounding readout `sounding-readout`

Above the footer, draw a **3px** `contour.steel` rule spanning the content width. Directly under it sits one row: a mono numeral at 48px on the left, and one conclusion sentence at 32px in `ink.sounding` on the right. **At most one per page** — it is that page's takeaway landing point.

It goes in the footer rather than in a callout box on purpose. A callout box competes with the body for visual weight; the footer position is, structurally, where "after you have read it" lives.

## Page Inventory

Seven page types. Density labels reference the contract's slot table; the numbers below are this style's assignments.

| # | Page type | Density | Text slots | Structure |
| --- | --- | --- | --- | --- |
| 01 | Cover · dark | low | 4 | `deep.navy` ground; mono project code above the title, title 100-116 (≤2 lines), a 120×3 `contour.lift` bar, client + date on one mono line |
| 02 | Agenda track · dark | low | 6 | Section overview: the track index fully expanded, all 5 nodes labelled, no body copy |
| 03 | Conclusion · light | medium | 6 | Title 64 + lead-in 36 + three numbered arguments (title 40 + detail 32, ≤2 lines each) + readout |
| 04 | Evidence · light | medium | 6 | Title 64 + **depth profile** + the 480px annotation rail (2 pairs of 36 + 30) + source line |
| 05 | Data · light | medium-high | 10 | Title 64 + table (header underlined 2px `contour.steel`; body rows 1px `rule.chart`; **last row unruled**) + source line |
| 06 | Trade-off · light | medium | 6 | Title 64 + the 7:3 split, 1px seam, right column on `chart.paper.deep`; the deck's single `shoal.ochre` appearance happens here |
| 07 | Action · dark | low | 5 | `deep.navy` ground; closing line 72 (≤2 lines) + three imperative actions (mono index + 32px copy) + mono sign-off |

## Strictly Avoid

1. **No corner radius.** The moment a radius appears, this deck slides from "sounding chart" to "SaaS landing page".
2. **No shadow, no gradient, no glassmorphism, no simulated depth of any kind.** The only depth in this system is the semantic depth the profile chart expresses.
3. **No content in cards.** Regions are separated by a 1px rule and by whitespace, not by rounded containers. `chart.paper.deep` surfaces are permitted in exactly two places: table zebra rows and the narrow column of the trade-off page.
4. **`shoal.ochre` once per deck.** The second occurrence voids the first.
5. **No topic titles.** Never "Market Overview"; always a complete conclusion sentence — "Only one of the three segments is still expanding". And never draw a rule under a title.
6. **No legend, no gridlines, no y-axis.** Values are labelled directly on the shapes; every non-key series is neutral grey.
7. **Dark and light pages never alternate.** Dark is permitted on the first one or two pages and the last page — a bookend — and every page between them is light.
8. **Nothing that implies interactivity.** No button states, no tabs, no pills, no badges, no navigation bars.
9. **No unsourced figures.** Any page containing a number carries a source line. If the source cannot be written, delete the number.
10. **No third chromatic hue.** Everything outside steel and ochre runs on the neutral ramp.

## Anti-Patterns

- **Black as the dark ground.** Pure black behind white type is the factory default of every AI-generated deck. Chroma in the navy is what makes the steel accent belong to the ground instead of sitting on it.
- **Matching the two grounds' hue.** Making the paper cold so it matches the navy collapses the mixed structure into a light/dark toggle of one colour and throws away the paper-versus-water distinction that justifies having two grounds at all.
- **A lighter, prettier ochre.** The warning colour was pushed down specifically to buy readable contrast on paper. Lifting it for the sake of a nicer swatch produces a warning nobody can read, which is worse than no warning.
- **Floating the profile bars.** Once the bars float, the baseline stops being an anchor and the chart becomes a generic waterfall — the single most recognisable thing in the style, spent.
- **A second readout on one page.** Two takeaways per page means neither is the takeaway.
- **Figurative nautical props.** No ships, anchors, compasses, helms, waves, or lighthouses. The anchor of this style is a measurement practice, not a maritime theme; the first literal boat turns it into a travel brochure.
