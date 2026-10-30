---
name: printability
description: Rules for designing parts that print well on FDM 3D printers - wall thickness, clearances and tolerances, overhangs, bridges, print orientation, strength and bed size - with default values to use when the user gives none. Use together with core-parametric for every ParametricDesign design.
---

# Designing for 3D printing (FDM)

Most users print on a hobby FDM printer: 0.4 mm nozzle, 0.2 mm layers, PLA or PETG, a bed of about 220-256 mm. Design for that unless told otherwise, and put the values that depend on the printer into **advanced params** so the user can tune them.

## Default numbers

| what | default | range / notes |
|---|---|---|
| wall (box sides, shells) | 2 mm | 1.2 minimum (3 lines); 2.4-3 for strong parts |
| floor / lid plate | 2 mm | at least 1.2 (6 layers) |
| smallest feature (rib, pin, text stroke) | 1.2 mm | never below 0.8 (2 × nozzle); the validator warns below 2 × nozzle |
| clearance between moving / removable parts | **0.2 mm per side** | 0.1 tight/press fit, 0.15-0.25 sliding, 0.3-0.5 loose |
| hole for a rod / pin / screw shank | nominal + 0.2-0.4 mm | holes print slightly small |
| sizes that must fit something | inside size = object + 2 × 0.5 mm | add more for things that vary (cables, food, hands) |
| corner rounding for boxes | 2-5 mm | rounded vertical edges print and look better |
| layer height | 0.2 mm | |
| infill | 15-20 % (decor, boxes), 30-50 % (brackets, load) | |

Make clearances and wall thicknesses params with `"advanced": true` (e.g. `fit`, `wall`) and use them in every expression that depends on them.

## Overhangs, bridges, supports

- The printer builds each layer on top of the previous one. A surface facing down at more than **45° from vertical** needs support. The validator warns about large ones.
- Short horizontal **bridges** (a closed roof over a hole, the top of a window) are fine up to about 10-20 mm. Small horizontal holes in walls (screw holes, cable holes up to about 8 mm) print fine.
- Avoid supports by design: put the flat side down, slope undersides at 45° (use a `wedge` or `cylinder` with `d_top` as a cone), split the object, or change `print_rotate`.
- Only set `"print": {"supports": true}` when supports really are unavoidable, and say so in `meta.description`.

## Orientation and strength

- Choose `print_rotate` so the **largest flat face is on the bed** and nothing hangs over air. Parts are laid on the bed at their lowest point automatically.
- Layers are weakest when pulled apart. Orient load-bearing parts so the load runs **along** the layers: an L bracket prints lying on its side, not standing up. A hook prints flat. A peg that gets bent prints lying down.
- Ribs and gussets that would hang in mid-air when the part is lying down should sit flush with the face that goes on the bed.
- Lids are printed upside down (plate on the bed) automatically.

## Size and the bed

- Default bed 256 × 256 × 256 mm (`print.bed` to change it). If a part is bigger, split it (multi-part skill). Don't shrink it silently.
- Keep parts under about 200 mm in the longest direction unless needed: they warp less and fit every printer.

## Holes and fasteners

- Use `screw_hole` (`fit: "clearance"` for screws that pass through, `"tap"` for screws threading into plastic, `"insert"` for heat-set inserts), `nut_trap` and `magnet_pocket` instead of hand-sizing cylinders. They already include sensible clearances.
- Leave at least 1.5 × the hole diameter of material around a screw hole, and at least 2 × d of depth for screws that thread into plastic.

## Parametric robustness

- Every param must work across its whole range. Ask yourself, for each param at its min and max, whether a wall disappears, a hole breaks out of the material, a count reaches 0 or parts collide. Clamp in `derived` (`min`, `max`, `clamp`) where they interact.
- Derive dependent sizes instead of making them independent params: the outer size from the inner size plus walls, hole spacing from the board size, and so on.
