---
name: connector-dovetail
description: Joins split ParametricDesign pieces with sliding dovetail keys - a key on one piece slides down into a matching slot in the other and locks sideways. Best for flat or tray-like objects split on a vertical seam (trays, plates, signs, shelves, long organisers). Use with multi-part when the user wants dovetail connectors.
---

# Dovetail connectors

A dovetail key grows out of the seam face of the first piece (`names[0]`) and flares wider towards its tip. The second piece gets a matching slot with clearance. Each key runs through the **full height of the material** where it sits, so the pieces join by sliding one **down** onto the other, and then can't be pulled apart sideways.

```json
"splits": [
  { "part": "tray", "axis": "x", "at": 0, "names": ["tray_left", "tray_right"],
    "joint": { "type": "dovetail", "count": 2, "width": 10, "depth": 8, "angle": 15, "clearance": "fit" } }
]
```

| field | default | notes |
|---|---|---|
| `count` | 2 | keys along the seam. 1 per 40-60 mm of seam length |
| `width` | 10 | neck width at the seam |
| `depth` | 8 | how far the key reaches into the other piece |
| `angle` | 15 | flare of the sides in degrees, 10-20 |
| `clearance` | 0.15 | gap per side. Use an advanced `fit` param, 0.1-0.25 |
| `positions` | automatic | positions along the seam (world coordinate on the horizontal axis lying in the cut plane: y for an x split, x for a y split) |

## Requirements

- **Vertical seams only**: `axis` must be `"x"` or `"y"`. For horizontal cuts (`"z"`) use pins or magnets.
- **At least 3 mm of material height** where each key sits, and room for the key's widest part plus 1.2 mm on each side. Thin walls (2 mm) can't hold a key; floors and solid plates can. When splitting a tray or organiser, make the **floor 4-6 mm** thick (an advanced param) so the keys sit in the floor.
- Keys are placed automatically on solid material spread along the seam. Give `positions` only if they must avoid something (a hole, a divider).

## Design tips

- Keep `depth` ≤ about the material's own length on each side of the seam minus 5 mm.
- The keys are strongest when the pieces are printed flat (seam vertical, keys in the layer plane). That is the default for trays and plates.
- Add a few drops of glue for a permanent join, or none for a removable one.
- In the reply, tell the user to slide the piece with the slots down over the keys.

## Example

`examples/dovetail-tray.pd.json`: a 320 mm desk tray split in the middle with two dovetails in its 5 mm floor.
