---
name: connector-magnet
description: Joins split ParametricDesign pieces with disc magnets in facing pockets on both sides of the seam, for connections that can be taken apart again. Works on any cut direction. Use with multi-part when the user wants magnetic or removable connectors.
---

# Magnet connectors

Each joint position gets a round pocket on **both** sides of the seam, flush with the seam faces. A magnet is glued into each pocket so they pull the pieces together.

```json
"splits": [
  { "part": "box", "axis": "z", "at": "height * 0.75", "names": ["box_body", "box_cap"],
    "joint": { "type": "magnet", "count": 4, "d": 6, "h": 3, "clearance": 0.1 } }
]
```

| field | default | notes |
|---|---|---|
| `count` | 2 | magnet pairs |
| `d` | 6 | magnet diameter |
| `h` | 3 | magnet thickness (each pocket is h + 0.2 deep) |
| `clearance` | 0.1 | gap per side: 0.05-0.1 press-fit, 0.15 with glue |
| `positions` | automatic | `[[u, v], ...]` points in the cut plane (world coordinates: (y, z) for an x split, (x, z) for a y split, (x, y) for a z split) |

Common neodymium disc sizes: 5×2, 6×2, 6×3, 8×2, 8×3, 10×2, 10×3, 12×3 mm. Make `d` and `h` params (advanced) so the user can match the magnets they own.

## Notes

- Each pocket needs the magnet diameter plus about 2.5 mm of material all round, and h + 0.7 mm of depth on both sides. Thin walls can't hold magnets: thicken them or cut where the material is thick.
- A single magnet pair lets the pieces rotate. Use 2 or more, or combine with a lip or pin for alignment.
- Tell the user to check **polarity** before gluing: put all magnets for one piece on a stack, mark the outward face, and glue the other piece's magnets so they attract.
