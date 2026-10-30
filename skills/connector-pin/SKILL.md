---
name: connector-pin
description: Joins split ParametricDesign pieces with alignment pins - matching holes on both sides of the seam plus separately printed pins (or pieces of filament, dowels or nails), usually glued. Works on any cut direction, including horizontal cuts through tall objects. Use with multi-part when the user wants pins, dowels or glued joints.
---

# Pin connectors

Both pieces get matching holes across the seam. Pins go into them, align the pieces exactly and carry sideways load; glue holds them together.

```json
"splits": [
  { "part": "statue", "axis": "z", "at": 80, "names": ["lower", "upper"],
    "joint": { "type": "pin", "count": 3, "d": 5, "length": 14, "clearance": 0.15 } }
]
```

| field | default | notes |
|---|---|---|
| `count` | 2 | 2 aligns; 3+ also stops rotation on round seams |
| `d` | 4 | pin diameter: 3-6 mm for printed pins |
| `length` | 12 | total pin length; half goes into each piece. About 3 × d |
| `clearance` | 0.15 | gap per side in the holes (0.1 for a snug fit, 0.2 for glue) |
| `make_pins` | true | also output the pins as printable parts (named `<part>_pin1`, ...) |
| `positions` | automatic | `[[u, v], ...]` points in the cut plane (world coordinates: (y, z) for an x split, (x, z) for a y split, (x, y) for a z split) |

## Notes

- Any axis works. For tall objects cut horizontally (`"z"`), pins are the natural choice.
- Holes need the pin diameter plus about 2.5 mm of material all around, and length/2 + 0.5 mm of depth on each side. If they don't fit, the validator says so: use thinner pins, fewer pins or a thicker seam.
- **Filament pins**: `"d": 1.75, "length": 10, "clearance": 0.1, "make_pins": false`, and tell the user to cut pieces of 1.75 mm filament. **Wooden dowels**: set `d` to the dowel diameter and `make_pins: false`.
- Printed pins are output standing up, which is the strongest way to print them.
- In the reply: "Glue the pins into the lower piece first, then press the upper piece on."
