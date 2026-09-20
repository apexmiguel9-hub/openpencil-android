---
name: jian-components
description: Interactive widget family — emit first-class native controls with explicit state and design-system styling
phase: [generation]
trigger: null
priority: 5
budget: 1200
category: base
---

INTERACTIVE WIDGETS (jian component family) — FIRST-CLASS OUTPUT:

Emit the native node directly through `I(parent, {...})` or canonical JSON.
Do not assemble a visual imitation from frame/rectangle/text children, and do
not author a role-marked frame expecting a later promotion pass. All controls
except `tabs` are leaves; `tabs.children[i]` is the panel for `tabs[i]`.

REQUIRED SEMANTIC PROPS (never omit these in generated designs):

- `text_input`, `text_area`: `value` plus an intentional `placeholder`.
- `select`, `radio_group`: `options: [{value,label}]` plus selected `value`.
- `switch`, `checkbox`: explicit `checked`; checkbox may also carry `label`.
- `slider`: numeric `min`, `max`, `step`, and `value`.
- `number_input`: numeric `min`, `max`, `step`, and `value`.
- `progress`: numeric `max` and `value` (`indeterminate` only when intended).
- `tabs`: `tabs: [{value,label}]`, active `value`, and one child panel per tab.

DESIGN-SYSTEM STYLE CONTRACT:

- Every native control MUST explicitly carry `fill`, `stroke`, and
  `cornerRadius` values taken from the active design system; keep width/height
  intentional too. Never rely on renderer defaults.
- `fill` is the active/accent paint (or the field/control surface where
  applicable). `stroke.fill` is the inactive track/border paint. Use palette
  tokens consistently so switches, sliders, progress, selections, and fields
  belong to the same product instead of falling back to generic white/grey.
- `fill` is an array; `stroke` is `{thickness, fill:[...]}`.

Example native controls (the same objects work in JSONL):

`I(parent,{type:"select",value:"north",options:[{value:"north",label:"North"}],width:240,height:44,fill:[{type:"solid",color:"#211238"}],stroke:{thickness:1,fill:[{type:"solid",color:"#7C5A9E"}]},cornerRadius:12})`

`I(parent,{type:"slider",min:0,max:100,step:5,value:40,width:280,height:44,fill:[{type:"solid",color:"#A855F7"}],stroke:{thickness:1,fill:[{type:"solid",color:"#4B3A5F"}]},cornerRadius:22})`

LEGACY COMPATIBILITY ONLY: old documents may still contain frames whose roles
are promoted: `input`/`form-input`, `textarea`/`text-area`, `select`/`dropdown`,
`switch`/`toggle`, `checkbox`, `slider`, `radio-group`/`radio`, `number-input`,
or `progress`/`progress-bar` (and `semantics.role: "input"`). Keep accepting
those inputs, but NEVER choose that representation for new generation.
