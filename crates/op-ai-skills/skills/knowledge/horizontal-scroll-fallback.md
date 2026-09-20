---
name: horizontal-scroll-fallback
description: Hand-built JSON structure for horizontal scroll rows (cards/chips/tiles) when MCP row tools are unavailable
phase: [generation]
trigger:
  keywords: [horizontal scroll, scrolling cards, swipeable row, chip row, card row, metric tiles, 横向滚动, 滑动列表]
priority: 25
budget: 1200
category: knowledge
---

HORIZONTAL SCROLL ROW — HAND-BUILT JSON FALLBACK

ONLY use this when the MCP row tools (`add_card_row_v0` / `add_metric_row_v0` / `add_nav_chip_row_v0`, taught by the `overflow` skill) are unavailable — embedded AI flow / JSON-only output. Generate EXACTLY this structure; do NOT just emit 6 cards inside a horizontal layout, the children will spill outside the page frame.

Structure:

- A wrapper frame with `width="fill_container"`, `height="fit_content"`, `layout="vertical"`, `clipContent=true`.
- Inside it, a row frame with `width="fit_content"`, `height="fit_content"`, `layout="horizontal"`, `gap=12`, `padding=[0,20]`.
- The row frame holds the actual cards.

Every **content / product / workout card** in the row MUST:

- Have a FIXED numeric `width` (typically 120-160 for mobile, 200-260 for desktop). Never `fill_container`, never `fit_content` - fixed pixels.
- Share identical width with its siblings for visual rhythm.

**EXCEPTION — nav chips / category chips / filter tags** (icon + short label like "All" / "Pizza" / "Videos"): use `width="fit_content"`, NEVER a fixed 120-160. That fixed width is content-card sizing; a 6-chip category row at 132px each becomes ~800px and scrolls off-screen for what should comfortably fit on one screen. With `fit_content`, a handful of short chips sit on one row (no scroll), and only a genuinely long list scrolls. Keep the same clipContent wrapper + fit_content row — just let each chip hug its content (icon + label + small horizontal padding).

**COUNT CAP for a no-scroll chip row (mobile 375px):** even at `fit_content`, only ~4-5 icon+label chips fit one phone width. For primary mobile category navigation, prefer the top 4 fully visible chips or wrap/grid them — do NOT show a half-clipped fifth chip as decoration. If the design is meant to fit on screen WITHOUT horizontal scrolling, emit only the chips that fit — do NOT pack 6+ chips into the row, the extras render off the right edge of the device. If you genuinely need all categories, you MUST place the row inside the `clipContent` wrapper above so the overflow clips at the screen edge (scroll row) instead of spilling past the phone frame. A bare horizontal frame with 6+ chips and no `clipContent` ancestor is the #1 mobile overflow bug — never emit it.

Example - 6 workout cards inside a 375px-wide mobile page:

```json
{
  "id": "cards-scroll",
  "type": "frame",
  "name": "Workouts Scroll",
  "width": "fill_container",
  "height": "fit_content",
  "layout": "vertical",
  "clipContent": true,
  "children": [
    {
      "id": "cards-row",
      "type": "frame",
      "name": "Workouts Row",
      "width": "fit_content",
      "height": "fit_content",
      "layout": "horizontal",
      "gap": 12,
      "padding": [0, 20],
      "children": [
        {
          "id": "card-hiit",
          "type": "frame",
          "width": 140,
          "height": 160,
          "cornerRadius": 20,
          "layout": "vertical",
          "gap": 8,
          "padding": 16,
          "fill": [{ "type": "solid", "color": "#1a1a1a" }],
          "children": []
        },
        {
          "id": "card-strength",
          "type": "frame",
          "width": 140,
          "height": 160,
          "cornerRadius": 20,
          "layout": "vertical",
          "gap": 8,
          "padding": 16,
          "fill": [{ "type": "solid", "color": "#1a1a1a" }],
          "children": []
        }
      ]
    }
  ]
}
```

Anti-patterns (do NOT emit any of these):

- Putting 5+ cards directly inside a `layout="horizontal"` page-root frame (they overflow the phone width).
- Using `fill_container` on cards in a horizontal row (they squish down to invisibility).
- Using `width="fit_content"` on **content/product cards** - text-driven widths are unpredictable and break rhythm. (Nav / category chips are the EXCEPTION above — those SHOULD use fit_content so a short row fits one screen.)
- Skipping the `clipContent=true` wrapper and relying on Skia to clip (it doesn't — only `clipContent:true` enables clipping).
