---
name: core-parametric
description: Writes ParametricDesign (pd/0.1) JSON designs - parametric 3D-printable models built from components placed by anchors, with sliders the user can adjust. Use whenever the user wants an object designed, resized, changed or fixed for 3D printing, or asks for a "parametric design", "pd.json" or a model for the ParametricDesign app.
---

# ParametricDesign: writing a parametric design

You write one JSON document in the `pd/0.1` format. The ParametricDesign app turns it into a 3D model, shows every param as a slider next to the viewer, and exports STL/3MF. The user fine-tunes the sliders afterwards, so your job is a **correct, well-parameterised design on the first try**.

## Workflow

1. **Read the request completely.** List the object's function, every size the user gave, what it must fit or hold, and how it will be printed. If the user says they prepared the request in the ParametricDesign app and you have its tools, call `get_design_request` first: it returns their description, photos, printer and the skills they picked.
2. **Changing the open design?** If the user says "this box", "make it taller", "add holes to it" and you have the ParametricDesign tools, call `get_open_design` first and edit *that* JSON: keep its param names, part ids and the slider values they chose (use them as the new defaults when it makes sense). Send it back with `open_in_app` `mode: "update"` and a one-sentence `message` saying what changed; the user sees the old and new versions side by side and accepts or rejects. If they mean a design that isn't open, find it with `list_designs` and update it by its `design_id`; never create a second design with the same name. For a *different* object, use `mode: "new"`: it goes into their library.
3. **Decide instead of asking.** Ask a question only when the design is impossible without the answer (for example, a case for a device whose size is unknown and not in a photo). Otherwise pick sensible defaults (see the printability skill) and make every uncertain size a param. Say what you assumed in `meta.description`.
4. **Sketch the layout in your head**: which parts, which base shape for each, which features are added or cut, and where they sit. Use the coordinate rules below.
5. **Write the JSON.**
6. **Check it.**
   - If you have the ParametricDesign tools (MCP): call `validate_design` and fix **every** error, then call `render_preview` and *look at the image*: is every part where it belongs, the right size and the right way up? Use `layout: "exploded"` for multi-part designs and `section` to see inside. Use `measure` for fits (for example the gap between a lid and its box). Repeat until it is valid and looks right, then send it with `open_in_app`.
   - Without tools: go through the self-check list at the end of this skill line by line.
7. **Answer** with a short summary and the main sliders. If you sent it with `open_in_app`, say it is in the app (as a new design, or as an update waiting for them to accept) and don't repeat the whole JSON; otherwise give the complete JSON in a single ```json block. Never send a partial design or "...".

## Coordinates and units

- Millimetres and degrees everywhere. Never put units in values (`"12.5"`, not `"12.5mm"`; use `inch(2)` for inches).
- **Z is up.** The print bed is the plane z = 0. **Front is -Y**, back is +Y, left is -X, right is +X.
- Every shape is built in its own frame: **centred on X and Y, resting on z = 0**. A `box` with `"size": [40, 20, 10]` spans x -20..20, y -10..10, z 0..10. A `cylinder` stands on its base. A `sphere` touches z = 0.
- A part without `place` stays at the origin, so the first part of a design normally has no `place`.

## Document structure

```json
{
  "format": "pd/0.1",
  "meta": { "name": "...", "description": "what it is, what it fits, assumptions", "category": "storage/boxes", "tags": ["..."], "license": "CC-BY-4.0" },
  "params": {
    "width": { "value": 80, "min": 20, "max": 200, "step": 1, "unit": "mm", "label": "Inside width", "group": "Size" },
    "wall": { "value": 2, "min": 1.2, "max": 4, "step": 0.1, "unit": "mm", "label": "Wall", "group": "Fit", "advanced": true }
  },
  "derived": { "outer_w": "width + 2 * wall" },
  "parts": [ { "id": "box", "shape": { "type": "hollow_box", "inner": ["width", 50, 30], "wall": "wall" }, "add": [], "cut": [], "color": "#1d6fd8" } ],
  "splits": [],
  "print": { "layer_height": 0.2, "infill": 15, "supports": false }
}
```

Only `format` and `parts` are required, but always add `meta` and `params`. Keys starting with `_` (such as `"_comment"`) are ignored, so use them for notes. Full field-by-field reference: `references/format.md`. All components with every field and default: `references/components.md`.

## Params: the sliders

Params are what makes the design useful. For each one give `value`, `min`, `max`, a `label` in the user's words, a `unit` and a `group`.

- **Name params after what the user cares about**: `phone_width`, `cable_d`, `shelf_depth`, not `x1`.
- **3 to 8 main params** in Simple mode: the sizes that define the object (inside size, number of slots, hole spacing...).
- Mark tuning details `"advanced": true`: wall thickness, clearances/tolerances, fillets, small offsets. Beginners only see the non-advanced ones.
- **Ranges must be realistic, and the design must build at every value in them.** The validator builds each param at its `min` and at its `max` (one at a time). If a combination breaks (holes outside the board, walls thicker than the box), clamp it in `derived`: `"hx": "min(hole_x, board_w - 7)"`.
- Types: number (default), `"type": "integer"` (counts), `"type": "boolean"` (`"value": true`), choice (`"value": "round", "options": ["round", "square"]`), `"type": "color"` (`"value": "#ff7a00"`).
- Put what the object must **fit** as params of the **inside** size (`inner` of a `hollow_box`), and compute the outside from it.

## Expressions

Any number can be an expression string using params and derived values: `"width + 2 * wall"`, `"max(3, height / 4)"`, `"count > 1 ? (length - 10) / (count - 1) : 0"`, `"style == \"round\" ? 5 : 0"`. Functions: `min max clamp abs round floor ceil sqrt pow sin cos tan asin acos atan atan2 if inch` (angles in degrees). `derived` values are computed top to bottom, so define a value before using it.

## Parts, features and colours

- A **part** is one printed object (one body). It has a base `shape`, then `add` features (unioned) and then `cut` features (subtracted), always in that order.
- Each **feature** is `{ "shape": {...}, "place": {...} }` plus optional `id`, `repeat`, `mirror`, `enabled`.
- Every added feature must **touch** the body. A part that falls into several pieces is an error.
- Separate parts must **not overlap** when assembled. Leave a clearance gap, or make one an `add` feature of the other if they are really one piece.
- `color`: `"#rrggbb"`, a basic colour name, or a color param. Parts are exported to 3MF with their colours, so give parts that should print in different filaments different colours.
- `enabled`: a boolean or expression (`"lid_holes"`, `"count > 0"`) to switch a part or feature on and off.

## Placing things: anchors

Place features and parts relative to **named anchors** instead of computing coordinates. Every shape has the anchors of its bounding box: `top`, `bottom`, `left` (-X), `right` (+X), `front` (-Y), `back` (+Y), `center`, and combinations such as `top-left` or `bottom-front-right` (the first word is the face, the rest slide the point to that edge or corner). Some components have more anchors, such as `hollow_box.inside_bottom` and `lid.seat`.

`place` fields:

| field | meaning |
|---|---|
| `on` | Target anchor. For a feature: an anchor of its own part's base shape (`"top"`, `"inside_bottom"`) or of an earlier feature with an id (`"shelf.top"`). For a part: `"<part id>.<anchor>"`, e.g. `"box.top"`. |
| `anchor` | Which anchor of the thing being placed goes onto the target. Usually leave it out (defaults below). |
| `offset` | `[x, y, z]` added afterwards, always in the part's / world axes (not the face's). |
| `rotate` | `[x, y, z]` degrees, applied to the shape first (about X, then Y, then Z). |
| `at` | `[x, y, z]` coordinates instead of `on`: puts the shape's own origin (bottom centre) there. |
| `orient` | `"normal"` (default for features) or `"none"` (default for parts). See below. |

**Features placed `on` a face are oriented to it**: the feature's own +Z points out of that face. On the top face nothing rotates. On a side face the feature's local X runs horizontally along the face and its local Y points up. So:

- an **added** feature stands **on** the face, sticking out (a peg on the front, a rib on the floor);
- a **cut** feature sinks **into** the face from the surface, as deep as its height (`h`). A hole cut `on: "top"` with `h: 5` is a 5 mm deep pocket. A `screw_hole` with `"through": true` measures the material under it and drills exactly through.

Parts are **not** rotated when placed: `"place": {"on": "box.top"}` sets the part's bottom on the box's top, centred. With `"on": "box.top-left"` its bottom-left edge sits flush on the box's top-left edge.

Part anchors come from the part's **base shape** (before add/cut). A `lid` places itself on the box it `fits` when it has no `place`, so leave the lid's `place` out.

## Repeating and mirroring features

- `"repeat": {"grid": [3, 2], "spacing": [20, 15]}`: a grid centred on the placement point, laid out along the face (X and Y on the top face; horizontal and up on a side face).
- `"repeat": {"linear": 4, "step": [10, 0, 0]}`: copies `step` apart in part axes, centred on the placement point (`"center": false` starts at it).
- `"repeat": {"radial": 6, "radius": 15}`: copies around the face normal through the placement point, rotated with it (`"rotate": false` keeps them unrotated; `"start_angle"`).
- `"mirror": "x"`: also a copy mirrored across the part's centre plane x = 0 (`"y"`, or `"xy"` for 4 copies). Place the original at a positive offset.

Counts can be expressions (`"repeat": {"linear": "slots", ...}`). Use `"enabled": "slots > 0"` when a count may be 0, because counts must be at least 1.

## Printing

- Each part is printed in its modelled orientation unless you set `"print_rotate": [x, y, z]` (degrees). The exporter lays every part on the bed. Choose the orientation with the largest flat face down and no overhangs (see the printability skill). Lids flip themselves.
- `print`: `layer_height`, `infill`, `walls`, `supports`, `nozzle`, `material`, `bed: [x, y, z]`. These are hints written into the 3MF.
- Objects larger than the bed must be split: see the multi-part skill.

## What the validator tells you

Each problem has a JSON path and a fix. **Errors** mean the design is wrong and must be fixed: unknown fields or types, bad expressions, parts that fall apart, overlapping parts, sizes that collapse at a param's min or max. When an error is tagged `(when wall = 4 (max))`, fix the formula or narrow that param's range. **Warnings** are printability advice: thin walls, overhangs needing support, too big for the bed, a cut that removes nothing (usually a placement mistake). Fix warnings too unless they are intended.

## Self-check (always, and especially without tools)

1. `"format": "pd/0.1"`, `meta.name`, and a unique `id` on every part (letters, digits, `_`).
2. Every shape `type` and field exists in `references/components.md`. No invented fields.
3. Every param has value, min, max, label and unit. The value is inside the range. Tolerances and wall thicknesses are `advanced`.
4. Every expression only uses declared names. `derived` values are defined before use.
5. Mentally build each part at a param's min and max: are all sizes still positive? Do holes stay inside the material? Do counts stay at least 1?
6. Every `add` touches the body. Every `cut` is placed `on` the face it should enter and is long enough to go through what it must (or uses `through`).
7. Separate parts do not intersect. Fits have clearance (0.2 mm per side by default).
8. Parts face the right way for printing (`print_rotate`) and fit the bed.

## Examples

Fully worked, validated designs: `examples/snap-lid-box.pd.json` (box and snap lid with color params), `examples/wall-bracket.pd.json` (anchors on faces, feature ids, through screw holes, print orientation), `examples/vase.pd.json` (revolved profile). Copy their patterns.
