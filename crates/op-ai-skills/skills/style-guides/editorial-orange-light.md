---
name: 'editorial-orange-light'
tags: [editorial, clean, high-contrast, light-mode, bold-typography, orange-accent, confident, tech, corporate]
platform: webapp
---

## Style Scope

This guide is self-contained. Apply its palette, radius scale, spacing, shadows, and component treatments only when this exact guide is selected; do not borrow food, social, dashboard, luxury, terminal, or other guide-specific patterns into another style. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is intentionally a card, input, button, badge, media mask, navigation surface, or other visible component. Avoid decorative wrapper shells around components; hierarchy should come from spacing, typography, and this guide's explicit surfaces.

## Style Summary

A near-white marketing page that gets its structure from solid near-black blocks rather than from borders. The page ground is #FAFAFA; whole sections — the product mock-up, the feature cards, the closing call to action — drop onto #18181B or #09090B panels with generous internal padding, so the reader travels through alternating light and dark bands instead of a uniform scroll. One saturated orange, #FF5A00, does every job an accent can do: the primary button, the active state, the progress fill, the inline link arrow. Nothing else competes with it.

Type is one grotesque, Inter, worked hard across a wide weight range. Headlines run 48-64px at 700 with negative tracking; body sits at 14-15px/1.5 in zinc greys; and small uppercase labels at 11-12px with +1.5 tracking mark every section so the page reads as an organised document rather than a stack of paragraphs.

Key aesthetics:

- **Alternating bands**: light page ground broken by full-bleed near-black sections; the contrast IS the layout
- **One accent, used decisively**: #FF5A00 on primary buttons, active toolbar cells, progress fills and link arrows — never as a tint or wash
- **Zinc neutral ramp**: every grey comes from one ramp (#09090B → #FAFAFA), which is what keeps dark panels from looking like different materials
- **Uppercase eyebrow labels**: 11-12px, +1.5 tracking, above every section title
- **Soft-square radii**: 10-14px on cards and panels, 8px on buttons and inputs, full capsules reserved for pills and badges
- **Product mock-ups as hero art**: a windowed application frame with a title bar and traffic-light dots, drawn with real UI inside it rather than a screenshot placeholder

## Color System

### Core Backgrounds

| Token           | Value   | Usage                                                |
| --------------- | ------- | ---------------------------------------------------- |
| Page Background | #FAFAFA | Root page ground, the light half of every band pair  |
| Page Alternate  | #F4F4F5 | Quiet strips between two light sections              |
| Panel Ink       | #18181B | Full-bleed dark sections, feature cards, CTA block   |
| Panel Deep      | #09090B | Application-mock chrome, the darkest inset available |
| Panel Raised    | #27272A | Cards and rows sitting on a dark panel               |
| Panel Inset     | #3F3F46 | Inputs, tracks and wells inside a dark panel         |

### Text Colors

| Token             | Value   | Usage                                            |
| ----------------- | ------- | ------------------------------------------------ |
| Ink               | #18181B | Headlines and body on the light ground           |
| Ink Secondary     | #52525B | Supporting copy on the light ground              |
| Ink Muted         | #71717A | Captions, metadata, disabled labels              |
| On Panel Primary  | #FFFFFF | Headlines and values on a dark panel             |
| On Panel Muted    | #A1A1AA | Body and labels on a dark panel                  |
| Accent Text       | #FF5A00 | Inline links, active labels, emphasised numerals |

### Border Colors

| Token           | Value   | Usage                                        |
| --------------- | ------- | -------------------------------------------- |
| Hairline        | #E4E4E7 | Section rules and dividers on light          |
| Hairline Strong | #D4D4D8 | Input outlines, card edges that must be seen |
| On Panel Border | #27272A | Dividers inside a dark panel                 |
| Accent Border   | #FF5A00 | Focus rings, selected tabs, active cells     |

### Accent Colors

| Token           | Value   | Usage                                        |
| --------------- | ------- | -------------------------------------------- |
| Primary         | #FF5A00 | Primary buttons, active states, progress fill |
| Primary Pressed | #E14F00 | Pressed and hovered primary                   |
| Signal Blue     | #2563EB | One full-bleed section that must break the two-tone rhythm (testimonials, community) |
| Success         | #27C93F | Live/synced indicators, positive status dots |
| Warning         | #FFBD2E | Pending status dots                          |
| Danger          | #FF5F56 | Error status dots, destructive actions       |

**How to use the accent**: give it to exactly one element per view — the primary CTA. Everything else that wants attention gets weight, size, or a dark panel instead. When an accent surface needs a second level (a hovered row, a selected chip), reach for the zinc ramp rather than a lighter orange; a tinted orange reads as a different brand.

## Typography

### Font Families

| Role     | Family        | Usage                                                    |
| -------- | ------------- | -------------------------------------------------------- |
| Display  | Inter         | Hero headline, section titles, metric values             |
| Body     | Inter         | Paragraphs, card copy, labels                            |
| CJK      | Noto Sans SC  | Any Chinese run — pairs with Inter at the same weights   |
| Monospace| Cosmica       | Code listings, file names, token values                  |

### Type Scale

| Level        | Size | Weight | Usage                                             |
| ------------ | ---- | ------ | ------------------------------------------------- |
| Display      | 64px | 700    | Hero headline (two lines, tracking -1.5)          |
| Title 1      | 40px | 700    | Section headlines                                 |
| Title 2      | 24px | 600    | Card and panel titles                             |
| Title 3      | 20px | 600    | Sub-headings, callout titles                      |
| Body Large   | 16px | 400    | Hero sub-copy, section intros                     |
| Body         | 14px | 400    | Card descriptions, list copy                      |
| Label        | 14px | 500    | Navigation links, button labels                   |
| Label Strong | 14px | 600    | Active navigation, table headers                  |
| Caption      | 12px | 400    | Metadata, helper text                             |
| Eyebrow      | 11px | 600    | Uppercase section labels (+1.5 tracking)          |
| Code         | 12px | 400    | Monospace listings                                |

### Font Weights

| Weight   | Value | Usage                                     |
| -------- | ----- | ----------------------------------------- |
| Regular  | 400   | Body copy, captions                       |
| Medium   | 500   | Navigation, secondary labels              |
| Semibold | 600   | Card titles, eyebrows, emphasised numbers |
| Bold     | 700   | Display and section headlines             |

### Letter Spacing

- Display (48-64px): -1.5px to -0.5px
- Section headlines (32-40px): -0.5px
- Uppercase eyebrows: +1.5px
- Body and labels: 0px

### Line Height

- Display: 1.05
- Headlines (20-40px): 1.2
- Body (14-16px): 1.5
- Captions and eyebrows: 1.4

## Spacing System

### Gap Scale

| Value | Usage                                     |
| ----- | ----------------------------------------- |
| 4px   | Icon and label pairs, tight numeral stacks |
| 6px   | Code lines, dense status rows              |
| 8px   | Badge groups, tag rows                     |
| 12px  | Button groups, list items                 |
| 16px  | Card internals, form fields               |
| 20px  | Nested panel sections                     |
| 24px  | Cards within a grid                       |
| 32px  | Section sub-blocks                        |
| 48px  | Between page sections                     |
| 80px  | Between full-bleed bands                  |

### Padding Scale

| Value     | Usage                                 |
| --------- | ------------------------------------- |
| [4, 12]   | Small pills, status chips             |
| [6, 12]   | Tabs, inline tags                     |
| [12, 20]  | Buttons                               |
| [14, 16]  | Inputs                                |
| 20px      | Compact cards on a dark panel         |
| 28px      | Feature cards                         |
| 32px      | Panel interiors                       |
| [80, 80]  | Full-bleed section blocks             |

### Layout Pattern

- Page width: 1200px, content inset 80px on each side
- Hero: two columns — copy on the left (max 520px), a windowed product mock-up on the right
- Feature grid: three equal cards on a dark band, gap 24
- Logo wall: a single uppercase eyebrow over a horizontal row of icon+wordmark pairs, gap 32
- Footer: brand column (240px fixed) + link columns that hug their labels + a newsletter column that takes the remainder. Do not make the link rail and the newsletter both `fill_container` — they will split the row evenly and the rail's columns will spill.

## Corner Radius

| Value | Usage                                | Rationale                                 |
| ----- | ------------------------------------ | ----------------------------------------- |
| 4px   | Progress tracks, thin indicators     | Barely softened, reads as a bar not a pill |
| 8px   | Buttons, inputs, small tags          | The interactive default                    |
| 10px  | Cards inside a panel                 | One step under the panel that holds them  |
| 14px  | Panels, feature cards, mock-up frame | The signature container radius            |
| 24px  | Full-bleed band blocks               | Large surfaces need a larger corner to read as soft |
| Full  | Avatars, status dots, capsule pills  | Set the radius to the element's own size  |

Design rationale: radii step up with the size of the thing they belong to, so nesting always reads correctly — a 10px card inside a 14px panel looks contained, whereas equal radii look like a mistake.

## Icons

### Icon Font

- **Family**: Lucide
- **Style**: Outline, 2px stroke, rounded caps

### Commonly Used Icons

zap, shield, bar-chart-2, panels-top-left, layers, box, triangle, credit-card, sliders-horizontal, arrow-right, play, check, circle-check, globe, mail, message-circle, sparkles, activity, git-pull-request, cpu, link

### Icon Sizes

| Size | Usage                                     |
| ---- | ----------------------------------------- |
| 14px | Inline with 12-14px text, status markers  |
| 16px | Button icons, list bullets                |
| 18px | Card header icons                         |
| 20px | Logo-wall marks, section markers          |
| 24px | Feature-card badges (inside a 44px tile)  |

### Icon Color States

| State      | Color   | Usage                                   |
| ---------- | ------- | --------------------------------------- |
| Accent     | #FF5A00 | Primary action icons, active states     |
| On Light   | #52525B | Default icons on the page ground        |
| On Panel   | #A1A1AA | Default icons inside a dark panel       |
| On Accent  | #FFFFFF | Icons sitting on an orange fill         |
| Muted      | #71717A | Disabled and decorative marks           |
