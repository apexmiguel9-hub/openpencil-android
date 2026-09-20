---
name: 'arcade-neon-dark'
tags: [social-card, card-series, cjk-type, vertical-portrait, dark-mode, night, neon, electric, bold-typography, display, high-contrast, urban, textured]
platform: card
---

## Style Scope

This guide is self-contained and written for **Chinese social cards** — a swipeable series of fixed-canvas pages read on a phone, not a scrolling page and not an app screen. Apply its palette, tube language, spacing, and type treatments only when this exact guide is selected; do not import crypto, gaming, cyberpunk, or SaaS-dark patterns into it. Treat unnamed layout frames as structural by default: no fill, stroke, cornerRadius, or shadow unless the node is deliberately a signage panel, a neon tube, a vertical signboard, or a caption rail.

## Style Summary

A shophouse arcade at night. The ground is `#0F1225` — not black, but the ink-blue-violet a city sky turns when its own light bounces off low cloud. Everything else in the system depends on that choice: neon laid on true black is a sticker, and neon laid on a sky that has already been stained by light has air between the tube and the wall.

Neon appears **only as tube**. A colour in this style is a 2px rounded-rect stroke with nothing inside it, plus a soft radial of the same hue at 12% opacity sitting underneath at two and a half times the tube's width. It never fills a shape, never becomes a text colour on a light ground, never becomes a gradient background. That single rule is the whole difference between this style and the cheap-AI-tech look it superficially resembles.

There are three tubes: magenta, cyan, amber. **Two per card, and each appears at most twice.** A fourth colour (jade) exists and is capped at one appearance across the entire deck — the one sign on the street that's a different colour from all the others.

Type is loud and Chinese. The display face is a genuine oblique Chinese sans — one of the very few that exists — and it carries every headline. It never sets body copy; its personality becomes reading friction past two lines. Body is a geometric screen sans in acrylic white at three brightnesses. The other signature device is the **vertical signboard**: a narrow 120-160px frame of stacked characters hard against the right edge of the cover, which is what shophouse signage actually does and which Latin typography structurally cannot do.

Key aesthetics:

- **Light-stained sky, not black**: `#0F1225` at chroma 0.038 — the tube needs air to glow into
- **Neon is a stroke, never a fill**: 2px tube + a 12% radial bloom underneath, and nothing else
- **Two tubes per card, twice each**: a third hue on one page is a bug
- **Vertical signboard**: stacked characters in a 120-160px rail, right edge, cover only
- **Oblique Chinese display over geometric sans body**: headline face never touches body copy
- **Acrylic white at three brightnesses**: `#E3E8EE` / `#ABB2BA` / `#757E88`
- **Wet-street fade**: one bottom-edge linear to the deepest night, 200px tall, once per deck
- **Panels are matte**: signage panels are flat fills with no glow — only tubes glow
- **Denser leading on dark**: body at 4+ lines moves up a size and out to 1.80 line height

## Color System

### Core Backgrounds

| Token           | Value   | Usage                                                                 |
| --------------- | ------- | --------------------------------------------------------------------- |
| Page Background | #0F1225 | The night sky. Every card's ground; never a panel colour                |
| Deep Night      | #060816 | The deepest step — inside a tube frame, and the end of the wet-street fade |
| Card Surface    | #1B2036 | Signage panel — the standard matte block                                |
| Raised Surface  | #282F47 | A panel on a panel, and the resting row inside one                      |

All four sit at hue ~273 with chroma 0.038-0.044. Only lightness moves between them; a hue shift would read as two different nights.

### Text Colors

| Token          | Value   | Usage                                                            |
| -------------- | ------- | ---------------------------------------------------------------- |
| Primary Text   | #E3E8EE | Headlines and body copy. 15.04:1 on the ground                   |
| Secondary Text | #ABB2BA | Deck lines, annotations, list sub-copy. 8.65:1                   |
| Muted Text     | #757E88 | Captions, sources, page numbers. 4.50:1                          |
| On Tube        | #060816 | Text set inside a filled tube frame — used sparingly              |

Acrylic white is cooled at hue 250 to match the sky rather than fight it. A warm white on this ground reads as a lit bulb rather than an acrylic sign face.

### Border Colors

| Token           | Value   | Usage                                                              |
| --------------- | ------- | ------------------------------------------------------------------ |
| Default Border  | #282F47 | The hairline between two panels of the same tone                    |
| Tube Stroke     | #FA618E | The 2px neon stroke — see the accent table for the other two hues   |

There is almost no border work. Panels separate by lightness. A tube is not a border: it is a light source, and it never wraps a content card — only a headline, a numeral, or a signboard.

### Accent Colors

| Token          | Value   | Usage                                                          |
| -------------- | ------- | -------------------------------------------------------------- |
| Primary Accent | #FA618E | Magenta tube. 6.32:1 on the ground                              |
| Cyan Tube      | #44D6D5 | Cyan tube. 10.43:1                                              |
| Amber Tube     | #F0B04E | Amber tube. 9.73:1                                              |
| Jade Tube      | #4CBD88 | The odd sign out. **Exactly one appearance per deck**            |

**How to use the tubes.** Each is a 2px `stroke` on a rounded rect or a headline underscore, with a same-hue `radial` from 12% to transparent underneath it at 2.5× the stroke width. That is the entire vocabulary. A tube colour may also set display type at 88px and above directly on the ground — all three clear 6:1 — but it must not set body copy, must not fill a block, and must not appear on a light surface.

Two tubes per card. Each tube colour at most twice on that card. The bloom counts as part of its tube, not as a second appearance.

**Gradients** exist for exactly two things: the tube bloom (radial, same hue, 12% → 0) and the wet-street fade (linear 90°, transparent → Deep Night, 200px tall, bottom edge, once per deck). Never a background gradient, never a two-hue gradient, and never blue-to-purple — that combination is the single most recognisable fingerprint of generic AI tech design and this style must not be mistaken for it.

## Typography

### Font Families

| Role             | Family                                    | Range         | Usage                                                    |
| ---------------- | ----------------------------------------- | ------------- | -------------------------------------------------------- |
| Display          | Anton, 得意黑 Smiley Sans, Noto Sans SC     | 88px and up   | Every headline, signboard, and large numeral             |
| Body / Interface | Schibsted Grotesk, Glow Sans SC, Noto Sans SC | 64px and down | Body, deck lines, list rows, captions, page numbers |
| Data / Numerals  | Schibsted Grotesk                         | any           | Every figure, with tabular-nums on                       |

得意黑 is one of the very few Chinese faces with a true designed oblique rather than a synthesised skew, which is exactly why it is here — and exactly why it must never set body copy: two lines of it and the reader is fighting the letterforms. All faces named are OFL.

The Latin face leads every fallback chain, so digits and Latin words are set by Anton / Schibsted Grotesk instead of by the Chinese face's own Latin.

### Type Scale

| Level      | Size  | Font    | Weight | Tracking | Line Height | Usage                                     |
| ---------- | ----- | ------- | ------ | -------- | ----------- | ----------------------------------------- |
| Display XL | 168px | Display | 700    | -0.01em  | 1.05        | Cover headline, 2-6 characters            |
| Display L  | 120px | Display | 700    | -0.01em  | 1.10        | Cover headline, signboard, large numerals |
| Display    | 88px  | Display | 700    | 0        | 1.15        | Interior headline, pull quote             |
| Title 1    | 64px  | Body    | 700    | 0        | 1.25        | Page title                                |
| Title 2    | 48px  | Body    | 600    | 0        | 1.30        | Section title, card title                 |
| Body L     | 40px  | Body    | 400    | 0.02em   | 1.70        | **Default body copy**                     |
| Body       | 36px  | Body    | 400    | 0.02em   | 1.70        | Body floor — never below                  |
| Caption    | 32px  | Body    | 400    | 0.02em   | 1.50        | Sources, page numbers, corner marks       |

**At most four of these eight on one card.**

**Dark-ground correction**: when a card carries four or more lines of body copy, Body 36 is not allowed — use Body L 40 and raise line height to **1.80**. Light text on a dark ground blooms optically, and dense small copy on this palette closes up.

### Font Weights

| Weight   | Value | Usage                                                      |
| -------- | ----- | ---------------------------------------------------------- |
| Regular  | 400   | All body copy and captions                                  |
| Semibold | 600   | Section titles, the emphasised run inside a body line       |
| Bold     | 700   | Page titles and every display line                          |

No `font-style: italic` on Han characters, ever — 得意黑's oblique is drawn into the face, and asking the browser to skew on top of it deforms every stroke. Emphasis in body copy is weight, or a tube underscore.

### Line Height

- Display (88-168px): 1.05-1.15
- Titles (48-64px): 1.25-1.30
- Body (36-40px): **1.70**, and **1.80** on cards with four or more body lines
- Captions: 1.50

### Letter Spacing

Body 0.02em; titles 0; display -0.01em. Latin all-caps labels +0.10em. Never apply Latin display tracking of -0.05em to Han text.

### Vertical Setting

The signboard is set with `writing-mode: vertical-rl`. Latin words and multi-digit numbers inside a vertical run must be set upright (`text-orientation: upright`, and two-digit numbers combined) or they lie on their side and break the sign.

## Spacing System

### Gap Scale

| Value | Usage                                            |
| ----- | ------------------------------------------------ |
| 8px   | The baseline unit                                |
| 16px  | Icon to label, tube to its caption               |
| 24px  | Between a list number and its text               |
| 32px  | Between stacked list rows                        |
| 48px  | Between a headline and its deck line             |
| 72px  | Between blocks within a card                     |
| 120px | Between the headline block and the content block |

### Padding Scale

| Value     | Usage                                                    |
| --------- | -------------------------------------------------------- |
| [16, 28]  | Tube frames around a short phrase                        |
| 40px      | Signage panels                                           |
| 56px      | A panel carrying four or more lines of body copy         |
| [96, 80]  | The card — 96 top, 80 sides                              |
| 128px     | Card bottom, deliberately larger so feed chrome misses the last line |

### Layout Pattern

- Canvas 1080 × 1440 (3:4). Content column 920: 12 columns of 62 with 16 gutters
- Cover: vertical signboard hard against the right edge in a 120-160px rail; headline and deck line in the left 9 columns; one tube frame; the wet-street fade at the bottom
- Interior: one headline, one content structure. Two structures means two pages
- Tubes: never wrap a content card. They frame a headline, a numeral, or the signboard
- Panels are matte: signage panels are flat Card Surface fills with no glow and no stroke
- Wet-street fade: bottom 200px, once per deck
- Page number: bottom-right, Caption size, Muted Text

## Corner Radius

| Value | Usage                                            | Rationale                                          |
| ----- | ------------------------------------------------ | -------------------------------------------------- |
| 4px   | Grit marks and micro tags                            | Nearly square at the finest scale                 |
| 16px  | Button radius, tube frames around a short phrase  | A bent glass tube has a radius; a sharp corner lies |
| 24px  | Card radius — signage panels                      | The signature container radius                      |
| 40px  | The signboard rail                                | Long surfaces need a larger corner to read soft     |

Nothing is a full capsule. A pill in this style reads as a UI chip, and there are no UI chips on a card.

## Icons

### Icon Font

- **Family**: Lucide
- **Style**: Outline, 2px stroke, round caps

Round caps here, unlike the flat-print styles: they match the bent-glass logic of the tubes.

### Commonly Used Icons

arrow-right, arrow-up-right, chevron-right, chevron-down, zap, sparkles, moon, play, circle, square, bookmark, message-circle, hash

### Icon Sizes

| Size | Usage                          |
| ---- | ------------------------------ |
| 32px | Beside Caption text            |
| 40px | Beside Body text, list markers |
| 56px | Section markers                |

### Icon Color States

| State     | Color   | Usage                                       |
| --------- | ------- | ------------------------------------------- |
| Primary   | #E3E8EE | Default on the ground and on matte panels   |
| Muted     | #757E88 | Decorative and inactive marks               |
| Accent    | #FA618E | The single marker carrying emphasis         |

## Anti-Patterns

- **Never fill with a tube colour.** Neon is a stroke plus a bloom. A magenta block, a cyan background, or a tube colour behind body copy destroys the style instantly.
- **No blue-to-purple gradient.** It is the defining fingerprint of generic AI tech design; this style's whole premise is being read as a photographed street instead.
- **No glitch, no scanlines, no chrome, no circuit traces, no rain droplets.** This is a night street, not cyberpunk. Figurative sci-fi props pull it straight into the cheap register.
- **No third tube hue on a card**, and no more than two appearances per hue.
- **Panels never glow.** Only tubes do. A glowing card edge reads as a game UI.
- **得意黑 never sets body copy**, and never sets anything under 88px.
- **No pure black.** `#000000` anywhere removes the atmospheric layer the whole palette is built on.
