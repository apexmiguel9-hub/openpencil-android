---
name: 'mingsha-mineral-dark'
tags: [social-card, card-series, cjk-type, vertical-portrait, dark-mode, warm-tones, editorial, serif, display, textured, sophisticated, high-contrast]
platform: card
---

## Style Scope

This guide is self-contained and written for **Chinese social cards** — a series of 6-10 fixed-canvas pages read by swiping on a phone, not a scrolling page and not an app screen. Apply its palette, texture language, spacing, and type treatments only when this exact guide is selected; do not borrow dashboard, terminal, luxury, food, or landing-page patterns into it. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is deliberately a plaster panel, a mineral block, a caption rail, or a foil-ruled corner mark. Hierarchy comes from the mineral colours and the size jump, not from wrapper shells.

## Style Summary

A cave-wall ground with pigment sitting on top of it. The page is `#21120B` — the ochre mud of a plaster wall after the paint has come off, not a black and not a brown card. Everything above it is one of five mineral pigments ground from actual stone: cinnabar red, azurite blue, malachite green, ochre, and shell white. The relationship is physical rather than decorative — **opaque pigment laid over bare plaster**, so a colour block always reads as something applied to the wall, never as the wall changing colour.

The signature device is **flaking**. Three to five irregular polygons in the next plaster tone up sit on the ground, edges deliberately off-grid, as if the top layer had come away. They are the only shape in the system allowed to ignore the column grid, and no two of them may be the same path — a repeated flake stops being erosion and becomes wallpaper.

Type splits cleanly. A旧字形 Ming display face carries every headline at 88px and up; a thin screen-optimised sans carries everything below it. The display face is what a cave inscription panel would look like if it were set rather than brushed: square-shouldered, high-contrast strokes, no calligraphic gesture at all. Body copy is shell white at three brightnesses rather than three greys, because the warm ochre ground turns a cold grey secondary tone muddy within one step.

Colour is rationed hard. **Three pigments exist; at most two appear on any one page, and each appears at most twice.** Gold leaf is not a pigment — it is a 1px rule around a corner mark, and it never touches text below 64px.

Key aesthetics:

- **Plaster ground**: `#21120B`, ochre mud at chroma 0.028 — a paper-weight surface, never a card colour
- **Flaking as the only texture**: 3-5 irregular polygons, off-grid, never repeated
- **Opaque pigment, never wash**: mineral colour covers; it does not bleed, blur, or fade into the ground
- **Two pigments per page, twice each**: the third pigment on a page is a bug
- **Shell white at three brightnesses**: `#EFEBE2` / `#BBB7AD` / `#8A867D`, warm-tinted so they sit on ochre
- **Ming display over thin sans**: 88px and up is the Ming face; below it, nothing but the sans
- **Gold as a rule, never as ink**: 1px foil frames on corner marks only
- **Grit at the edges**: 8-14 four-pixel squares of pigment scattered within 40px of a colour block's edge
- **No brush, anywhere**: no ink bleed, no dry-brush, no calligraphic stroke — this is applied pigment, not painting

## Color System

### Core Backgrounds

| Token           | Value   | Usage                                                           |
| --------------- | ------- | --------------------------------------------------------------- |
| Page Background | #21120B | The plaster wall. Every card's ground; never a panel colour       |
| Card Surface    | #312018 | Plaster panel — the standard raised block                        |
| Raised Surface  | #412E25 | A panel on a panel, and the fill of every flake polygon           |
| Wash Surface    | #4F3E37 | The quietest step up, for a resting row inside a panel            |

Four steps, all at hue 45 and chroma 0.030. The ramp is lightness only — a shift in hue between surfaces would read as two different walls.

### Text Colors

| Token          | Value   | Usage                                                              |
| -------------- | ------- | ------------------------------------------------------------------ |
| Primary Text   | #EFEBE2 | Headlines and body copy. 15.27:1 on the ground                     |
| Secondary Text | #BBB7AD | Sub-copy, deck lines, list annotations. 9.07:1                     |
| Muted Text     | #8A867D | Captions, sources, page numbers, corner marks. 5.01:1              |
| On Pigment     | #EFEBE2 | All text on a deep-pigment block                                    |

Shell white carries a +85° warm tint at chroma 0.012. On this ochre ground a neutral grey secondary tone goes green within one step; the tint is what keeps the three brightnesses reading as one material.

### Border Colors

| Token           | Value   | Usage                                                        |
| --------------- | ------- | ------------------------------------------------------------ |
| Default Border  | #4F3E37 | The rare hairline between two panels of the same tone        |
| Foil Rule       | #D5AA55 | 1px gold rule, corner marks and page-number frames only      |

Borders are close to absent. Two panels are told apart by lightness, the way two coats of plaster are. If a rule appears, it is because two same-tone surfaces meet, and it is 1px.

### Accent Colors

| Token             | Value   | Usage                                                                  |
| ----------------- | ------- | ---------------------------------------------------------------------- |
| Primary Accent    | #C54F3B | Cinnabar. Display type ≥48px, thick underscores, flake highlights       |
| Cinnabar Deep     | #9D2E1C | The cinnabar block that carries shell-white text (6.21:1)               |
| Azurite           | #3577AB | The blue pigment: chart bars, markers, numerals                         |
| Azurite Deep      | #105482 | The azurite block that carries shell-white text (6.76:1)                |
| Malachite         | #479173 | The green pigment. **Once per deck**, never more                        |
| Malachite Deep    | #1B674C | Its text-bearing block (5.71:1)                                         |
| Gold Leaf         | #D5AA55 | Foil: 1px rules and corner marks. Never body ink                        |

**How to use the pigments.** Each pigment has a bright tone and a deep tone, and they are not interchangeable: the bright tone is for *type and marks sitting on the wall* (3.79-4.81:1 — legal at 48px and above, never for body copy), the deep tone is for *blocks that carry white text*. Picking the bright tone as a block fill is the single most common way to break this style — shell white on `#C54F3B` is only 3.87:1 and body copy inside it fails.

At most two pigments appear on one card, each at most twice. Malachite is capped at one appearance across the whole deck — it is the rarest of the three stones and the palette treats it that way.

**Gradients** exist for exactly one thing: the niche glow. A single `radial` from Muted Text at 12% to transparent, centred around (0.5, 0.28), used once on the cover page. No gradient on a block, a panel, a button, or a pigment.

## Typography

### Font Families

| Role             | Family                              | Range        | Usage                                                       |
| ---------------- | ----------------------------------- | ------------ | ----------------------------------------------------------- |
| Display          | Newsreader, 汇文明朝体, Noto Serif SC | 88px and up  | Every headline, pull quote, and large numeral                |
| Body / Interface | Geist, LXGW Neo XiHei, Noto Sans SC | 64px and down | Body, captions, list rows, annotations, page numbers       |
| Data / Numerals  | Geist                               | any          | All figures, with tabular-nums on                           |

The Latin face is written **first** in every fallback chain. Font-family matching is per-character: Latin faces carry no CJK codepoints, so Han characters fall through to the Chinese face automatically. Written the other way round, the Chinese face's own mediocre Latin swallows every digit and letter and the paired Latin face never appears.

汇文明朝体 is free for commercial use; Newsreader, Geist, LXGW Neo XiHei, and the Noto faces are OFL.

### Type Scale

| Level      | Size  | Font    | Weight | Tracking | Line Height | Usage                                     |
| ---------- | ----- | ------- | ------ | -------- | ----------- | ----------------------------------------- |
| Display XL | 168px | Display | 700    | -0.01em  | 1.05        | Cover headline, 2-6 characters            |
| Display L  | 120px | Display | 700    | -0.01em  | 1.10        | Cover headline, large numerals            |
| Display    | 88px  | Display | 700    | 0        | 1.15        | Interior headline, pull quote             |
| Title 1    | 64px  | Body    | 700    | 0        | 1.25        | Page title                                |
| Title 2    | 48px  | Body    | 600    | 0        | 1.30        | Section title, card title                 |
| Body L     | 40px  | Body    | 400    | 0.02em   | 1.70        | **Default body copy**                     |
| Body       | 36px  | Body    | 400    | 0.02em   | 1.70        | Body floor — never go below               |
| Caption    | 32px  | Body    | 400    | 0.02em   | 1.50        | Sources, page numbers, corner marks       |

**Use at most four of these eight on one card.** More than four and the levels stop meaning anything; the scale exists to force that restraint, not to offer variety.

32px is the floor for anything at all. A card renders 1080px wide and is read at roughly 390pt, so 32px lands near 11.6pt — already the bottom of comfortable. Below that is not "small type", it is a defect.

### Font Weights

| Weight   | Value | Usage                                                                |
| -------- | ----- | -------------------------------------------------------------------- |
| Regular  | 400   | All body copy and captions                                            |
| Semibold | 600   | Section titles, the emphasised run inside a body line                |
| Bold     | 700   | Page titles and every display line                                    |

Chinese has no italic. Never set `font-style: italic` on Han characters — the browser synthesises a skew that deforms the strokes. Emphasis moves to weight (400 → 600), to a pigment underscore, or to a change of face. Turn synthesis off globally.

### Line Height

- Display (88-168px): 1.05-1.15 — the larger the size, the tighter
- Titles (48-64px): 1.25-1.30
- Body (36-40px): **1.70** — Chinese runs about 0.2 higher than the Latin equivalent because Han glyphs fill the em box top to bottom, and 1.5 leaves them touching
- Captions: 1.50

### Letter Spacing

Body 0.02em; titles 24-64px at 0; display above 88px at -0.01em. **Never apply the Latin display convention of -0.05em to Han text** — Han glyphs are already full-width by design, and negative tracking at that scale collides strokes.

## Spacing System

### Gap Scale

| Value | Usage                                             |
| ----- | ------------------------------------------------- |
| 8px   | The baseline unit. Everything is a multiple        |
| 16px  | Inside a caption rail, icon to label               |
| 24px  | Between a list row's number and its text           |
| 32px  | Between stacked list rows                          |
| 48px  | Between a headline and its deck line               |
| 72px  | Between blocks inside a card                       |
| 120px | Between the headline block and the content block   |

### Padding Scale

| Value     | Usage                                                   |
| --------- | ------------------------------------------------------- |
| [16, 24]  | Corner marks and small pigment tags                     |
| 40px      | Plaster panels                                          |
| 56px      | A deep-pigment block carrying white text                |
| [96, 80]  | The card itself — 96 top, 80 sides                      |
| 128px     | Card bottom. Deliberately larger than the top: the feed overlays interaction chrome there |

### Layout Pattern

- Canvas 1080 × 1440 (3:4). Content column 920 wide: 12 columns of 62 with 16 gutters
- Cover: display headline in the top 38%, deck line under it, the flake cluster and one pigment block in the bottom 45%
- Interior: one headline, one content structure, nothing else. A page that carries two structures is two pages
- Flakes: 3-5 polygons in Raised Surface, none repeated, edges off-grid, never overlapping text
- Grit: 8-14 four-pixel squares in a pigment at 15-25% opacity, within 40px of a block's edge
- Niche glow: one radial per deck, cover page only
- Page number: bottom-right, Caption size, Muted Text, inside a 1px foil frame

## Corner Radius

| Value | Usage                                            | Rationale                                        |
| ----- | ------------------------------------------------ | ------------------------------------------------ |
| 0px   | Flake polygons, grit squares, foil rules          | Erosion and pigment have no radius               |
| 8px   | Pigment tags and corner marks                  | Just enough to read as applied rather than cut    |
| 16px  | Button radius, caption rails                      | The interactive floor                             |
| 24px  | Card radius — plaster panels and pigment blocks   | The signature container radius                    |

Nothing in this style is a capsule. A fully rounded pill reads as a UI control, and there are no UI controls on a card.

## Icons

### Icon Font

- **Family**: Lucide
- **Style**: Outline, 2px stroke, square caps

This style is not icon-led. It prefers a numeral, a foil rule, or a pigment block where another style would place a glyph. Icons appear in list markers and corner marks and nowhere else.

### Commonly Used Icons

arrow-right, arrow-up-right, chevron-right, check, minus, plus, circle, square, bookmark, quote, hash, corner-down-right

### Icon Sizes

| Size | Usage                              |
| ---- | ---------------------------------- |
| 32px | Beside Caption text                |
| 40px | Beside Body text, list markers     |
| 56px | Section markers                    |

### Icon Color States

| State     | Color   | Usage                                     |
| --------- | ------- | ----------------------------------------- |
| Primary   | #EFEBE2 | Default on the wall and on pigment blocks |
| Muted     | #8A867D | Decorative and inactive marks             |
| Accent    | #C54F3B | The single marker that carries emphasis   |

## Anti-Patterns

- **No figurative motifs.** No apsaras, no lotus, no caisson ceilings, no camel bells, no dunes. The moment a representational symbol appears, the theme collapses from "a pigment system" into "a tourism poster".
- **No brushwork.** No ink bleed, no dry-brush texture, no calligraphic stroke, no seal stamps. This style is opaque pigment applied to plaster — covering, not soaking in. That is exactly what separates it from every ink-and-rice-paper treatment.
- **No repeated flake.** Reusing one polygon path turns erosion into a pattern, and a pattern is cheap.
- **No gold ink.** Gold is a 1px rule. A gold headline is the fastest way to make this style look like a banquet menu.
- **No third pigment on a page**, and no bright pigment used as a block fill under body copy.
- **No gradient** except the single cover-page niche glow.
