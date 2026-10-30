---
name: connector-snap-fit
description: Makes ParametricDesign box lids that click shut - a lid with snap bumps on its lip and matching grooves cut automatically into the box walls - with rules for sizes, clearances and wall thickness. Use when the user wants a snap-on, click or tool-free lid for a box, case or container.
---

# Snap-fit lids

Use a `hollow_box` part and a `lid` part with `"style": "snap"`. The lid's lip drops into the box; small bumps on the lip click into grooves that the engine cuts into the box's inner walls.

```json
"parts": [
  { "id": "box", "shape": { "type": "hollow_box", "inner": ["width", "depth", "height"], "wall": "wall", "radius": 4 } },
  { "id": "lid", "shape": { "type": "lid", "fits": "box", "style": "snap", "clearance": "fit", "lip_height": 5, "snap_size": 0.6 } }
]
```

| lid field | default | notes |
|---|---|---|
| `fits` | — | id of the hollow_box part |
| `style` | `lip` | `snap` for clicking lids; `lip` for a friction-fit lip; `flat` for a plain plate |
| `clearance` | 0.2 | gap per side between lip and wall; use an advanced `fit` param |
| `thickness` | 2 | plate thickness |
| `lip_height` | 4 | how deep the lip goes into the box; 4-6 for snaps |
| `lip_wall` | 1.6 | lip thickness: thinner flexes more |
| `snap_size` | 0.6 | bump radius: 0.4 gentle, 0.6 firm, 0.8 very firm |

## Rules

- **Leave the lid's `place` out.** It sits on its box automatically, and the grooves are cut where the bumps land. It is printed upside down automatically.
- Box `wall` at least **1.6 mm** (2 mm recommended): the groove is `snap_size + 0.1` mm deep, so keep `snap_size` under about a third of the wall.
- `lip_height` must be less than the box's inside height, and bigger than 2 × `snap_size` + 1.
- PLA snaps are stiff: keep `snap_size` at 0.4-0.6. PETG tolerates 0.8.
- Make the clearance a param (`fit`, advanced, 0.1-0.4) and say in its description: "Increase if the lid is too tight".

## Checking

- `render_preview` with `"section": {"axis": "y", "at": 0}` and the front view shows the lip, the bumps and the grooves cut through the middle.
- `measure` `{"type": "distance", "a": "box", "b": "lid"}` should report a small gap and no overlap.

## Example

`examples/snap-lid-box.pd.json`.
