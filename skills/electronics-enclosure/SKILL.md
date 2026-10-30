---
name: electronics-enclosure
description: Designs ParametricDesign cases for circuit boards and electronics - board standoffs from the mounting-hole pattern, cut-outs for ports, buttons and displays, ventilation, cable entries and snap or screwed lids, with sizes for common boards. Use when the user wants a case, housing or enclosure for a PCB, Raspberry Pi, Arduino, ESP32 or other electronics.
---

# Electronics enclosures

## Structure

1. **Base**: `hollow_box` with `inner` = board size + 2 × `gap` (2 mm) in X/Y, and inside height = standoff height + board thickness (1.6 mm) + tallest component + 3-5 mm.
2. **Standoffs**: `standoff` features placed `on: "inside_bottom"` with the board's hole pattern as a `grid` repeat, `offset` by the pattern's shift from the board centre.
3. **Port cut-outs**: `box` (rectangular) or `slot` (rounded) cuts placed `on` the wall face (`front`, `back`, `left`, `right`) with an `offset` to the port's position. The cutter's local X runs along the wall and local Y points up, so `"size": [port_width, port_height, wall + 1]`.
4. **Lid**: `lid` with `style: "snap"` (no screws) or `style: "flat"`/`"lip"` with screw holes into corner standoffs (connector-screws skill).
5. **Ventilation**: a `linear` repeat of `slot` cuts on the lid's `top` or on a wall.

## Heights and positions

Work out the vertical position of everything from the floor:

```json
"derived": {
  "board_top": "wall + standoff_h + 1.6",
  "outer_h": "height + wall",
  "port_z": "board_top + port_h / 2 - outer_h / 2"
}
```

A side face's anchor (`front`, `left`...) is at **half the outer height**, so a cut-out's vertical `offset` is "wanted centre height − outer_h / 2". Horizontal offsets are along the world axis of that wall: X for the front/back walls, Y for left/right.

Ports usually sit flush with the board edge. Measure from the board's datasheet or photo, and make every port position and size a param in a `Ports` group.

## Common boards

| board | size (mm) | mounting holes | notes |
|---|---|---|---|
| Raspberry Pi 4 / 5 | 85 × 56 | M2.5, 58 × 49 apart, 3.5 from left/top/bottom edges (pattern shifted −10 in X from the board centre) | USB-C and micro-HDMI on a long side, USB/Ethernet on the right end |
| Raspberry Pi 3 B/B+ | 85 × 56 | same as Pi 4 | micro-USB power, full HDMI |
| Raspberry Pi Zero / Zero 2 W | 65 × 30 | M2.5, 58 × 23 apart, 3.5 from the edges | mini-HDMI and 2 micro-USB on one long side |
| Arduino Uno | 68.6 × 53.4 | 4 × M3 (3.2 mm) at (14, 2.5), (15.3, 50.7), (66.1, 7.6), (66.1, 35.5) from the bottom-left corner | USB-B and barrel jack on the left end |
| Arduino Nano | 18 × 45 | none (use a slot/cradle) | mini/micro/USB-C on one end |
| ESP32 DevKit (30-pin) | ~28 × 51-55 | usually none (cradle) | sizes vary: measure |

For irregular hole patterns, use separate standoff features with their own offsets instead of a grid. Holes are at (x, y) from the board's bottom-left corner, so offset = (x − board_w/2 + shift, y − board_d/2).

## Rules

- **Clamp dependent sizes** in `derived` so standoffs never leave the board when the sliders move (`"hx": "min(hole_x, board_w - 7)"`).
- Walls 2 mm; standoffs 4-6 mm tall (room for solder joints underneath); `M2.5` tap holes for Raspberry Pi, `M3` for most others.
- Leave 1-2 mm around connectors in cut-outs (`port_w` = connector width + 2).
- Buttons and LEDs: round holes (`cylinder` cuts) on the lid `top`, positioned with offsets from the board centre.
- Cables: a `slot` cut at the bottom of a wall, or a round hole with a strain-relief bump.
- Heat: add vents above hot chips (Raspberry Pi CPU), at least 20 % open area for boards that run warm.

## Example

`examples/board-enclosure.pd.json`: a case for a Raspberry Pi 4/5-size board with standoffs, a front port and a vented snap lid.
