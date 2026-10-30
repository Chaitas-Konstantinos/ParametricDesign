---
name: multi-part
description: Designing ParametricDesign models made of several separately printed pieces - splitting objects that are too big for the print bed or unprintable in one piece, assemblies of parts that fit together, clearances between parts, and checking them in exploded view. Use when the object is larger than the bed, has parts in different colours, moves, or must be assembled.
---

# Multi-part models

## When to use several parts

- **Too big for the bed** (default 256 × 256 × 256 mm): split it into pieces that each fit, joined with a connector.
- **Unprintable in one piece**: an overhang or enclosed cavity that would need heavy supports. Split along a plane that gives every piece a flat side on the bed.
- **Different colours or materials**: each colour is its own part. 3MF keeps the colour per part.
- **Moving or removable pieces**: lids, drawers, sliders, hinges, pegs.

## Two ways to build it

### A. Design the whole thing, then split it (best for oversized objects)

Model the object as **one part** at full size, then cut it with `splits`. The engine cuts it and adds the connector geometry to both sides:

```json
"parts": [ { "id": "tray", "shape": { "type": "hollow_box", "outer": [320, 90, 30], "wall": 2.4, "floor": 5 } } ],
"splits": [
  { "part": "tray", "axis": "x", "at": 0, "names": ["tray_left", "tray_right"],
    "joint": { "type": "dovetail", "count": 2 } }
]
```

- `axis` is the direction the cut plane is **perpendicular** to. `"x"` cuts into a left piece (`names[0]`, x < at) and a right piece (`names[1]`, x > at). `"z"` cuts into bottom and top.
- `at` is a world coordinate. Parts are centred on X/Y, so `at: 0` is the middle. For 3 pieces of a 450 mm object use two splits at -75 and 75. The second split names the piece it cuts: split `"part": "shelf_b"` from the first split.
- Pieces keep the part's colour and `print_rotate`.
- Make the length a param and compute the cut positions from it (`"at": "length / 6"`). The validator checks the split at the param's min and max too.
- Choose the joint with the connector skill the user selected (`dovetail`, `pin`, `magnet`), or `none` for glue-only joints.

**Where to cut**: through thick, solid material, away from holes and thin walls. Dovetails need at least 3 mm of material height (thicken a tray's floor to 4-5 mm). Pins and magnets need their diameter plus about 2.5 mm of material all round. If the joint has no room, the validator says so: thicken the part at the seam, move the cut, or pick a smaller connector.

### B. Model separate parts in their assembled positions

For things that fit together (lid + box, peg + base, two halves of a case), model each part and place it relative to the others:

```json
{ "id": "base", "shape": { "type": "box", "size": [60, 60, 8] } },
{ "id": "peg", "shape": { "type": "cylinder", "d": "peg_d", "h": 30 }, "place": { "on": "base.top" } }
```

Rules:

- Parts must **not intersect** when assembled. A peg that sits in a hole needs the hole `2 × fit` bigger than the peg (`"d": "peg_d + 2 * fit"`). Overlapping parts are an error.
- Use a shared `fit` param (advanced, default 0.2 mm) for every clearance between parts.
- Use anchors and feature ids so parts follow each other when sizes change: `"on": "box.top"`, `"on": "base.socket.top"`.
- Give parts different colours so they are easy to tell apart in the viewer.

## Checking (with tools)

- `render_preview` with `"layout": "exploded"` shows the pieces pulled apart, with connectors visible. `"layout": "print"` shows how each piece lies on the bed.
- `measure` with `{"type": "distance", "a": "lid", "b": "box"}` gives the gap (or the overlap) between two parts.
- The validator warns when a piece is still too big for the bed.

## Tell the user

In the reply, say how many pieces there are, how they go together (slide the dovetails down, glue the pins, insert the magnets with matching polarity), and what to glue or buy (magnets, screws, inserts with sizes).
