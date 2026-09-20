---
name: 'butter-serif-light'
tags: [editorial, serif, dual-font, display, light-mode, warm-tones, colorful, rounded, soft-corners, friendly]
platform: webapp
---

## Style Scope

This guide is self-contained. Apply its palette, radius scale, spacing, shadows, and component treatments only when this exact guide is selected; do not borrow food, social, dashboard, luxury, terminal, or other guide-specific patterns into another style. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is intentionally a card, input, button, badge, media mask, navigation surface, or other visible component. Avoid decorative wrapper shells around components; hierarchy should come from spacing, typography, and this guide's explicit surfaces.

## Style Summary

A butter-cream page with no lines on it anywhere. The ground is #FFFFEB — a warm, faintly yellow off-white, not a grey ivory — and structure comes entirely from big soft-cornered blocks of colour dropped onto it: a sand panel here, a deep-teal section there, one long near-black band in the middle of the page. There is not a single stroke in the whole system; where another style would draw a 1px rule, this one changes the fill.

The voice lives in one typographic move, used on every large headline: a roman serif phrase interrupted by an italic one of the same face. "Built around *how you work*", "4x faster *than typing*", "Don't type, *just speak.*" — EB Garamond carries the whole line at weight 400, and the italic run is the only thing that changes. Display type runs very large (64-120px) and very tight: -0.03em tracking and a line height at or just under 1.0, so a two-line headline reads as one block of shape.

The two faces do not share the headline; they split the page. **EB Garamond owns everything from 20px up and Figtree owns everything below it** — there is no sans-serif headline anywhere and no serif button label. Body copy is Figtree 16/500 in near-black, and secondary copy is the same ink at 70% opacity rather than a second grey. Colour arrives only as whole surfaces — a teal section, a lilac button, a mint stat card — never as tints, gradients, or outlines.

Key aesthetics:

- **Butter-cream ground**: #FFFFEB, warm and slightly yellow, is the page — not a card colour, not a section colour
- **Zero strokes**: no borders, no dividers, no hairlines; separation is fill against fill
- **Roman interrupted by italic**: one phrase per headline switches to EB Garamond italic — same face, same size, same baseline
- **A clean face split**: EB Garamond at 20px and above, Figtree at 20px and below; neither crosses
- **Oversized tight display**: 64-120px at -0.03em with line height 0.95-1.0
- **Ink at three opacities**: #1A1A1A at 100 / 70 / 50 % instead of a grey ramp
- **Big soft corners**: 32-48px on section blocks, full capsules on every button and tag
- **Crayon accents as whole surfaces**: mint, coral, lilac and amber appear as filled blocks and pills, never as text colour or outline
- **One drawn line**: a single thick organic black ribbon sweeping across a section is the only illustration device

## Color System

### Core Backgrounds

| Token            | Value   | Usage                                                    |
| ---------------- | ------- | -------------------------------------------------------- |
| Page Background  | #FFFFEB | The whole page. Sections sit on it; it is never a card    |
| Sand Surface     | #E4E4D0 | Cards, feature panels, the standard raised block          |
| Sand Quiet       | #EEEBE3 | A second sand a step closer to the page                   |
| Paper Panel      | #F5F4F0 | Inset panels inside a sand card                           |
| Paper Bright     | #FFFDF9 | The lightest surface, for a panel on a coloured ground    |
| Inverted Band    | #1A1A1A | One long full-bleed near-black section per page           |
| Deep Teal Band   | #034F46 | The second full-bleed section, and the tile colour it seeds |

### Text Colors

| Token           | Value     | Usage                                                  |
| --------------- | --------- | ------------------------------------------------------ |
| Ink             | #1A1A1A   | Headlines, body copy, everything primary                |
| Ink 70%         | #1A1A1AB3 | Secondary copy, descriptions, captions                  |
| Ink 50%         | #1A1A1A80 | Uppercase eyebrows, metadata, disabled labels           |
| Ink Soft        | #71716E   | The rare label that needs a solid grey rather than alpha |
| On Dark         | #FFFFEB   | All text on the inverted band, the teal band, and pills |
| On Dark Soft    | #FFFFEB1A | A whisper-level wash on dark, for a resting surface     |

Note the ink ramp is opacity, not separate greys. `#1A1A1A` at 70% over butter cream is warmer than any neutral grey would be, which is what keeps secondary copy from going cold on this ground.

### Border Colors

There are none. This style has no strokes: not on cards, not on inputs, not as dividers. If two things must be told apart, give one of them a surface colour. The only near-border in the system is a low-alpha wash used as a resting fill:

| Token       | Value     | Usage                                           |
| ----------- | --------- | ----------------------------------------------- |
| Ink Wash    | #1A1A1A26 | A pressed / selected state on a light surface   |
| Sand Wash   | #E4E4D01A | A resting row inside a coloured section         |

### Accent Colors

| Token       | Value   | Usage                                                    |
| ----------- | ------- | -------------------------------------------------------- |
| Teal        | #034F46 | The section ground, the primary dark pill, tile surfaces |
| Lilac       | #F0D7FF | The primary call-to-action pill (with ink text)          |
| Mint        | #34D399 | One stat or status block per page                        |
| Coral       | #FF6C4C | One emphasis block, opposite the mint one                |
| Amber       | #FFA946 | The third and last crayon, for a single highlight tile   |

**How to use the accents**: they are surfaces, not ink. A crayon colour fills a block, a pill, or a tile; it never colours text, never outlines anything, and never appears as a gradient. Use at most three of the four on one page, each exactly once — the effect depends on each colour reading as a single deliberate object against the cream, and a repeated crayon immediately reads as a palette instead.

**Gradients** are for image scrims only: a 180° `#FFFFFF00 → #000000B3` to seat white text over photography, or `#FFFFFF00 → #034F46` to fade an image into the teal band. Never on a button, a card, or a background.

## Typography

### Font Families

| Role             | Family      | Range     | Usage                                                       |
| ---------------- | ----------- | --------- | ----------------------------------------------------------- |
| Display          | EB Garamond | 20px and up | Every headline and pull quote, always weight 400; italic for the emphasis run |
| Interface / Body | Figtree     | 20px and down | Body, labels, buttons, navigation, captions, data          |

The split at 20px is the rule, and 20px is the one size both faces are allowed: Figtree 20/500 for hero sub-copy, EB Garamond 20/400 for a quiet serif section label. Everywhere else the size decides the face.

### Type Scale

| Level        | Size  | Font        | Weight | Tracking | Usage                                       |
| ------------ | ----- | ----------- | ------ | -------- | ------------------------------------------- |
| Display XL   | 120px | EB Garamond | 400    | -3.6px   | The closing statement, one line, once       |
| Display L    | 96px  | EB Garamond | 400    | -2.88px  | Hero headline                               |
| Display      | 75px  | EB Garamond | 400    | -2.25px  | Section openers on a full-bleed band        |
| Title 1      | 64px  | EB Garamond | 400    | -1.92px  | Major section headlines                     |
| Title 2      | 48px  | EB Garamond | 400    | -1.44px  | Big numbers, stat values, sub-headlines     |
| Title 3      | 32px  | EB Garamond | 400    | -0.96px  | **The workhorse** — card titles, feature headings, pull quotes |
| Serif Label  | 20px  | EB Garamond | 400    | 0        | A quiet serif label inside a card           |
| Lead         | 20px  | Figtree     | 500    | 0        | Hero sub-copy, section intros               |
| Body         | 16px  | Figtree     | 500    | 0        | The default body copy                       |
| Body Strong  | 16px  | Figtree     | 600    | 0        | Emphasis inside body, product-name labels   |
| Label        | 14px  | Figtree     | 500    | 0        | **Most-used size** — nav, buttons, list rows, footer links |
| Eyebrow      | 14px  | Figtree     | 500    | +1.12px  | Uppercase section labels, at Ink 50%        |
| Caption      | 13px  | Figtree     | 500    | 0        | Metadata, units, footnotes                  |
| Micro        | 12px  | Figtree     | 600    | -0.24px  | Input placeholders, badge text              |

Two rules make the scale work. **Display tracking is -0.03em at every size** — the pixel values above are that ratio, so a size not in the table computes rather than guesses. **Uppercase eyebrows are +0.08em** (+1.12px at 14px), the only positive tracking in the system.

### Font Weights

| Weight   | Value | Usage                                                    |
| -------- | ----- | -------------------------------------------------------- |
| Regular  | 400   | EB Garamond — every headline, no exceptions               |
| Medium   | 500   | Figtree default: body, labels, navigation, captions       |
| Semibold | 600   | Figtree emphasis: a strong run in body, a placeholder     |
| Bold     | 700   | Reserved. If a 600 is not enough, the size is wrong       |

The display face never goes above 400. Its presence at 75-120px is what carries the weight, and a bolded Garamond loses exactly the quality it was chosen for.

### The emphasis run

A headline at 64px or above is one line of EB Garamond 400 roman with a single phrase swapped to the **italic** of the same face, at the same size and baseline. Put the italic on the phrase that carries the promise, not on a noun — the roman states the fact and the italic says how it feels. One run per headline, and only on the largest tier: a 32px card title stays roman throughout, and a second italic run in one line turns the device into decoration.

### Line Height

- Display (48-120px): 0.95, and 1.0 where a headline wraps to two lines
- Title 3 (32px): 1.0
- Lead and Body (16-20px): 1.19-1.21
- Eyebrows and captions: 1.2

Tight is the point. A display line height above 1.0 opens a gap that the tracking has just closed.

## Spacing System

### Gap Scale

| Value | Usage                                          |
| ----- | ---------------------------------------------- |
| 8px   | Icon and label inside a pill                   |
| 12px  | Tag rows, tight stacks                         |
| 16px  | Card internals, list items                     |
| 24px  | Between a headline and its sub-copy            |
| 32px  | Between cards in a row                         |
| 48px  | Between a section header and its content       |
| 80px  | Between blocks inside a section                |
| 160px | Between page sections on the cream ground      |
| 240px | Around a full-bleed band, above and below      |

### Padding Scale

| Value      | Usage                                            |
| ---------- | ------------------------------------------------ |
| [10, 20]   | Pills and buttons                                |
| [12, 24]   | Tags and status capsules                         |
| 24px       | Inset panels                                     |
| 40px       | Sand cards                                       |
| [80, 64]   | Section blocks                                   |
| [160, 120] | Full-bleed bands, top and bottom generous        |

### Layout Pattern

- Page width: 1920, content held to a centred ~1240 column
- Hero: centred stack — uppercase eyebrow, display headline over two lines, 20px sub-copy, one pill CTA, a small availability caption beneath it
- Full-bleed bands: two per page at most, one inverted (#1A1A1A) and one teal (#034F46), each with 32-48px corners so the cream shows at the edges
- Feature list: a tall sand or teal panel on the left, a stack of 32px title + 16px copy pairs on the right
- Comparison: two panels side by side inside one sand card — the teal half asks, the paper half answers
- Decoration: one thick black organic ribbon per page, drawn as a path that crosses a section edge. One. It is a signature, and two is a pattern.

## Corner Radius

| Value   | Usage                                          | Rationale                                     |
| ------- | ---------------------------------------------- | --------------------------------------------- |
| 4-8px   | Inline chips, small inset wells                | The smallest things stay nearly square         |
| 12-16px | Cards inside a panel, image masks              | One step under the block that holds them      |
| 32px    | Sand cards, section blocks                     | The signature container radius                 |
| 48px    | Full-bleed bands                               | Large surfaces need a large corner to read soft |
| Full    | Buttons, pills, tags, avatars, waveform capsules | Set the radius to the element's own height    |

Every interactive element is a capsule. There are no rounded-rectangle buttons in this style — a button with an 8px corner reads as a different product.

## Icons

### Icon Font

- **Family**: Lucide
- **Style**: Outline, 2px stroke, rounded caps

This style is not icon-led: it prefers a drawn ribbon, a waveform capsule, or a photograph where another style would place a glyph. Keep icons to the places that genuinely need one — a check inside a status pill, a chevron in a nav — and let the type and the colour blocks do the rest.

### Commonly Used Icons

check, circle-check, chevron-down, chevron-right, arrow-right, arrow-up-right, mic, audio-lines, languages, sparkles, keyboard, command, globe, play, x

### Icon Sizes

| Size | Usage                                  |
| ---- | -------------------------------------- |
| 14px | Inside a pill, beside 13-14px text     |
| 16px | Buttons, list rows                     |
| 20px | Card headers                           |
| 24px | Section markers                        |

### Icon Color States

| State     | Color     | Usage                                    |
| --------- | --------- | ---------------------------------------- |
| Ink       | #1A1A1A   | Default on cream and on a crayon surface |
| Ink 50%   | #1A1A1A80 | Decorative and inactive marks            |
| On Dark   | #FFFFEB   | On the inverted band, the teal band, pills |
