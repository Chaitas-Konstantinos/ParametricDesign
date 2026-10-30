---
name: connector-screws
description: Metric screw holes, countersinks, counterbores, nut traps, heat-set insert holes and standoffs for ParametricDesign models - to bolt printed parts together or mount them to walls, furniture and devices. Use when the design is screwed, bolted or mounted.
---

# Screws, nuts and inserts

## Components

- `screw_hole` (use in `cut`): `size` M2-M8; `fit`: `clearance` (the screw passes through freely), `tap` (the screw cuts its own thread in plastic), `insert` (hole for a heat-set threaded insert); `head`: `none`, `cap` (counterbore), `countersunk` (90° cone); `depth` or `"through": true`.
- `nut_trap` (use in `cut`): hexagonal pocket for a metric nut, sized with clearance.
- `standoff` (use in `add`): round post with a `tap` or `insert` hole, for boards and lids.

The head recess is always at the entry face: a `screw_hole` placed `on` a face drills into it from there.

## Patterns

**Through-mount to a wall or board** (countersunk, flush):

```json
{ "shape": { "type": "screw_hole", "size": "M4", "through": true, "head": "countersunk" },
  "place": { "on": "front", "offset": [0, 0, 10] },
  "repeat": { "linear": 2, "step": [0, 0, "hole_span"] } }
```

`through` measures the material under the placement point and drills exactly through it. Use it for walls and plates. On a `hollow_box` it goes through **one** wall, not across the cavity.

**Bolting two printed parts together**: clearance holes in the top part, tap or insert holes in the bottom part, at the **same offsets**. In the lid (top part):

```json
{ "shape": { "type": "screw_hole", "size": "M3", "through": true, "head": "cap" }, "place": { "on": "top", "offset": ["sx", "sy", 0] }, "mirror": "xy" }
```

and in the base (bottom part), matching posts:

```json
{ "shape": { "type": "standoff", "screw": "M3", "h": "post_h", "fit": "insert" }, "place": { "on": "inside_bottom", "offset": ["sx", "sy", 0] }, "mirror": "xy" }
```

**Captive nut**: a `screw_hole` with `fit: "clearance"` through the part plus a `nut_trap` on the opposite face at the same position.

## Sizes

| size | clearance hole | tap hole | heat-set insert hole | cap head recess ⌀ × depth | countersink ⌀ |
|---|---|---|---|---|---|
| M2 | 2.4 | 1.7 | 3.2 | 4.4 × 2.2 | 4.2 |
| M2.5 | 2.9 | 2.2 | 3.6 | 5.0 × 2.7 | 5.0 |
| M3 | 3.4 | 2.6 | 4.0 | 6.2 × 3.2 | 6.2 |
| M4 | 4.5 | 3.5 | 5.6 | 8.0 × 4.2 | 8.2 |
| M5 | 5.5 | 4.4 | 6.4 | 9.5 × 5.2 | 10.2 |
| M6 | 6.6 | 5.2 | 8.0 | 11.0 × 6.2 | 12.2 |
| M8 | 9.0 | 7.0 | 10.0 | 14.5 × 8.2 | 16.4 |

## Rules

- Material around a hole: at least 1.5 × the screw diameter between the hole and an edge.
- Depth for `tap` holes: at least 2 × the screw diameter. `insert` holes default to the insert length + 1 mm.
- For wall mounting, use M4 or M5 countersunk screws (typical wood screws are 3.5-5 mm), 2+ screws, and orient the part so the screw load runs along the layers.
- Make the screw size a choice param (`"options": ["M3", "M4", "M5"]`) when the user may have different screws, and pass it straight to `size`.
- In the reply, list what to buy: screw size and length, nuts or inserts, and how many.
