---
name: 'broadsheet-lime-light'
tags: [editorial, magazine, clean, light-mode, minimal, serif, lime-accent, crisp, data-focused, tech]
platform: webapp
---

## Style Scope

This guide is self-contained. Apply its palette, radius scale, spacing, shadows, and component treatments only when this exact guide is selected; do not borrow food, social, dashboard, luxury, terminal, or other guide-specific patterns into another style. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is intentionally a card, input, button, badge, media mask, navigation surface, or other visible component. Avoid decorative wrapper shells around components; hierarchy should come from spacing, typography, and this guide's explicit surfaces.

## Style Summary

A paper-white product page that borrows its manners from a broadsheet newspaper: near-black ink on white, hairline rules instead of shadows, and one italic serif word dropped into an otherwise sans-serif headline to carry the emphasis. Surfaces are #FFFFFF and #FAFAFA, separated by #E5E5E5 hairlines rather than by elevation, so the page stays flat and the reader's eye is led by type size and rule placement alone.

The single accent is an electric lime, #A3E635, and it is deliberately rationed: the primary button, the "most popular" pricing badge, one bar in a seven-bar chart, one success state. Against a page this quiet, a lime chip reads as a highlighter mark on newsprint — which is exactly the intent, and why a second accent would ruin it.

Key aesthetics:

- **Hairline structure, no shadows**: 1px #E5E5E5 rules define every card, table row and section boundary
- **Serif emphasis inside a sans headline**: one word per headline set in Fraunces italic, the rest in Geist — the page's whole voice lives in that contrast
- **Rationed lime**: #A3E635 on the primary CTA, the featured pricing tier, and single data points; never as a background wash
- **Tight radii**: 4px and 8px only, with full capsules for pills — a soft, generous corner would undo the printed feel
- **Live-product panels**: hero and comparison sections show a real answer card (query, response, cited sources) rather than an abstract illustration
- **Data as body copy**: analytics blocks use the same type scale as the prose around them, with numerals at 36-48px doing the shouting

## Color System

### Core Backgrounds

| Token           | Value   | Usage                                            |
| --------------- | ------- | ------------------------------------------------ |
| Page Background | #FFFFFF | Root page ground                                 |
| Page Alternate  | #FAFAFA | Alternating sections, table headers, quiet bands |
| Card Surface    | #FFFFFF | Cards and panels, distinguished by their rule    |
| Inset Surface   | #F5F5F5 | Inputs, chips, code wells                        |
| Ink Panel       | #171717 | The one inverted block per page (code, avatars)  |
| Ink Panel Soft  | #303030 | Rows inside an inverted block                    |

### Text Colors

| Token            | Value   | Usage                                          |
| ---------------- | ------- | ---------------------------------------------- |
| Ink              | #262626 | Headlines, body copy, table values             |
| Ink Secondary    | #525252 | Supporting copy, descriptions                  |
| Ink Muted        | #737373 | Metadata, timestamps, column headers           |
| Ink Faint        | #A3A3A3 | Placeholders, disabled labels                  |
| On Ink           | #FFFFFF | Text on an inverted block                      |
| On Accent        | #171717 | Text on a lime fill — lime is too bright for white |

### Border Colors

| Token           | Value   | Usage                                        |
| --------------- | ------- | -------------------------------------------- |
| Hairline        | #E5E5E5 | Card edges, table rows, section rules        |
| Hairline Subtle | #F0F0F0 | Dividers inside a card                       |
| Hairline Strong | #D4D4D4 | Input outlines, focused edges                |
| Ink Border      | #171717 | The emphasised card (featured pricing tier)  |

### Accent Colors

| Token         | Value   | Usage                                          |
| ------------- | ------- | ---------------------------------------------- |
| Primary Lime  | #A3E635 | Primary buttons, featured badge, hero bar      |
| Lime Soft     | #ECFCCB | The one tinted background lime is allowed      |
| Mint          | #7EE2B8 | The secondary data series, status "healthy"    |
| Success       | #16A34A | Verified marks, positive deltas                |
| Warning       | #F59E0B | "Needs attention" chips in a data table        |
| Danger        | #DC2626 | Failures, negative deltas                      |

**How to use the accent**: one lime element per viewport height. If two want it, the more actionable one keeps it and the other becomes an ink-outlined variant. Lime always carries #171717 text — white on lime fails contrast and looks washed out.

## Typography

### Font Families

| Role            | Family   | Usage                                                  |
| --------------- | -------- | ------------------------------------------------------ |
| Display / Body  | Geist    | Everything by default — headlines, copy, labels, data  |
| Serif Emphasis  | Fraunces | One italic word per headline, pull-quote attributions  |
| UI / Data       | Inter    | Dense tabular readouts, chart labels, code-adjacent UI |

### Type Scale

| Level        | Size | Font     | Weight | Usage                                        |
| ------------ | ---- | -------- | ------ | -------------------------------------------- |
| Display      | 48px | Geist    | 600    | Hero headline (with one Fraunces italic word) |
| Title 1      | 36px | Geist    | 600    | Section headlines, big metric values          |
| Title 2      | 20px | Geist    | 600    | Card titles, pricing tier names               |
| Title 3      | 16px | Geist    | 600    | Sub-headings, row titles                      |
| Body Large   | 16px | Geist    | 400    | Hero sub-copy, section intros                 |
| Body         | 14px | Geist    | 400    | Card copy, feature lists, table cells         |
| Label        | 13px | Geist    | 600    | Buttons, active navigation                    |
| Caption      | 12px | Geist    | 400    | Metadata, helper text                         |
| Eyebrow      | 11px | Geist    | 600    | Uppercase section labels (+1.2 tracking)      |
| Data         | 12px | Inter    | 500    | Chart labels, chips, dense readouts           |

### Font Weights

| Weight   | Value | Usage                                    |
| -------- | ----- | ---------------------------------------- |
| Regular  | 400   | Body copy, table cells                   |
| Medium   | 500   | Chart and chip labels                    |
| Semibold | 600   | Headlines, titles, buttons, eyebrows     |

Italic is reserved: Fraunces italic for headline emphasis and pull quotes, and nothing else. An italicised label or caption breaks the device.

### Letter Spacing

- Display (48px): -1.5px
- Section headlines (36px): -1px
- Uppercase eyebrows: +1.2px
- Body, labels, data: 0px

### Line Height

- Display: 1.1
- Headlines (20-36px): 1.2
- Body (14-16px): 1.55
- Table cells and chips: 1.3

## Spacing System

### Gap Scale

| Value | Usage                                        |
| ----- | -------------------------------------------- |
| 4px   | Icon and label pairs, value+unit pairs       |
| 6px   | Chip rows, citation badges                   |
| 8px   | Feature list items, tight stacks             |
| 10px  | Table row internals                          |
| 12px  | Button groups, card sub-sections             |
| 16px  | Form fields, card internals                  |
| 24px  | Cards within a grid, panel sections          |
| 32px  | Section sub-blocks                           |
| 40px  | Between page sections                        |
| 96px  | Between major bands                          |

### Padding Scale

| Value     | Usage                                  |
| --------- | -------------------------------------- |
| [2, 8]    | Micro status chips                     |
| [4, 8]    | Table action buttons                   |
| [4, 10]   | Tags and citation badges               |
| [6, 12]   | Segmented-control segments             |
| [12, 16]  | Inputs, list rows                      |
| [20, 24]  | Cards                                  |
| 24px      | Panels and pricing tiers               |
| [96, 96]  | Section blocks                         |

### Layout Pattern

- Page width: 1200px, content inset 96px on each side
- Hero: copy column on the left (max 520px), a live answer-card panel on the right
- Feature block: one wide primary column plus stacked secondary cards, each with an eyebrow, a title, copy and a small live readout
- Analytics: three metric cards in a row above a two-column split — a chart card and a data table
- Pricing: three tiers, the middle one emphasised by an ink border and a lime badge, not by scale
- Charts: give bar columns and their axis labels the SAME explicit column width so bars and labels stay on shared centres, and so the row is not mistaken for a data table

## Corner Radius

| Value | Usage                                       | Rationale                                    |
| ----- | ------------------------------------------- | -------------------------------------------- |
| 0px   | Section rules, table dividers               | Printed rules have no corners                |
| 2px   | Chart bars                                  | Just enough to avoid a hard pixel edge       |
| 4px   | Chips, tags, small badges, inputs           | The dense default                            |
| 8px   | Cards, panels, buttons, pricing tiers       | The container default — and the largest used |
| Full  | Pills, avatars, toggles, status dots        | Set the radius to the element's own size     |

Design rationale: the ceiling of 8px is the rule that keeps this style from drifting into a generic rounded SaaS look. Softness here comes from white space and hairlines, not from corners.

## Icons

### Icon Font

- **Family**: Lucide
- **Style**: Outline, 1.5-2px stroke

### Commonly Used Icons

search, sparkles, check, circle-check, file-text, book-open, database, refresh-cw, activity, trending-up, trending-down, clock, shield-check, hexagon, layers, globe, arrow-right, chevron-down, copy, calendar, zap, git-pull-request, link-2

### Icon Sizes

| Size | Usage                                    |
| ---- | ---------------------------------------- |
| 12px | Inside chips and citation badges         |
| 14px | Inline with body copy, feature checks    |
| 16px | Buttons, card headers, table actions     |
| 20px | Logo-wall marks, section markers         |
| 24px | Hero panel header, tier icons            |

### Icon Color States

| State      | Color   | Usage                                    |
| ---------- | ------- | ---------------------------------------- |
| Ink        | #262626 | Default on white                         |
| Muted      | #737373 | Secondary and decorative marks           |
| Accent     | #A3E635 | Only where the lime element already is   |
| Success    | #16A34A | Verified sources, passing checks         |
| On Ink     | #FFFFFF | Icons on the inverted block              |
