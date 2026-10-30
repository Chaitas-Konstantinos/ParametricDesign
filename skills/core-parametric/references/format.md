# pd/0.1 format reference

Complete field reference for ParametricDesign designs. Unknown keys are errors, except keys starting with `_` or `$`, which are comments.

## Top level

| key | required | type | notes |
|---|---|---|---|
| `format` | yes | `"pd/0.1"` | exactly this string |
| `meta` | recommended | object | see below |
| `params` | recommended | object of params | the user's sliders, in display order |
| `derived` | no | object of expressions | computed values, evaluated top to bottom |
| `parts` | yes | array of parts | at least one |
| `splits` | no | array of splits | cut a part into printable pieces (multi-part skill) |
| `print` | no | object | print hints for the 3MF |

### meta

`name` (text, shown in the app and catalog), `description` (what it is, what it fits, assumptions), `category` (path like `storage/boxes`, `household/mounting`, `electronics/enclosures`, `tools/holders`, `home/decor`, `toys/games`), `tags` (array of text), `license` (`CC-BY-4.0`, `CC0-1.0`, ...), `author`, `skills` (ids of the skills used), `version`.

## params

```json
"params": {
  "width":  { "value": 80, "min": 20, "max": 200, "step": 1, "unit": "mm", "label": "Inside width", "group": "Size" },
  "count":  { "type": "integer", "value": 3, "min": 1, "max": 8, "label": "Compartments" },
  "lid":    { "type": "boolean", "value": true, "label": "Include lid" },
  "style":  { "value": "round", "options": ["round", "square"], "label": "Corner style" },
  "color":  { "type": "color", "value": "#1d6fd8", "label": "Colour" },
  "fit":    { "value": 0.2, "min": 0.05, "max": 0.5, "step": 0.05, "unit": "mm", "label": "Clearance", "advanced": true }
}
```

| field | notes |
|---|---|
| `value` | required; the default |
| `type` | `number` (default for numbers), `integer`, `boolean`, `choice` (default when `options` is given), `color` (default for `#hex` text) |
| `min`, `max` | required for number/integer (literal numbers, not expressions) |
| `step` | slider step |
| `unit` | `"mm"`, `"°"`, `"%"`... display only |
| `label` | short name in the user's words |
| `description` | tooltip: what it changes, when to change it |
| `group` | sliders are grouped by this (e.g. Size, Style, Fit, Mounting) |
| `advanced` | `true` hides it in Simple mode |
| `options` | for choice params: the allowed values |

Param names: letters, digits and `_`, not starting with a digit; not `true`, `false` or `pi`. A plain value (`"width": 80`) is allowed but gives no slider range, which is a warning.

## derived

```json
"derived": { "outer_w": "width + 2 * wall", "pitch": "(length - 2 * margin) / max(count - 1, 1)" }
```

Names follow the param rules and must not repeat a param. Each value is an expression (or a number) that may use params and *earlier* derived values.

## Expressions

- Numbers: `12`, `0.4`, `1e-3`. Text: `"round"` or `'round'`. Booleans: `true`, `false`. Constant `pi`.
- Operators by precedence (low to high): `c ? a : b`, `||`, `&&`, `== !=`, `< <= > >=`, `+ -`, `* / %`, unary `- !`, `^` (power).
- Functions: `min(a, b, ...)`, `max(...)`, `clamp(x, lo, hi)`, `abs`, `round`, `floor`, `ceil`, `sqrt`, `pow(a, b)`, `sin cos tan` (degrees), `asin acos atan atan2(y, x)` (degrees), `if(c, a, b)`, `inch(x)` (= x × 25.4).
- Division by zero and `sqrt` of a negative number are errors, so guard with `max(..., 1)` where a param can reach 0.

## parts

```json
{
  "id": "box",
  "name": "Box body",
  "shape": { "type": "hollow_box", "inner": ["width", "depth", "height"], "wall": "wall" },
  "add": [ /* features unioned to the shape */ ],
  "cut": [ /* features subtracted after all adds */ ],
  "color": "#1d6fd8",
  "place": { "on": "base.top" },
  "enabled": "include_box",
  "print_rotate": [0, 0, 0],
  "allow_disconnected": false
}
```

| key | notes |
|---|---|
| `id` | required, unique, letters/digits/`_` |
| `name` | display name |
| `description` | note |
| `shape` | required; a component (see components.md) |
| `add`, `cut` | arrays of features |
| `color` | `#rgb`, `#rrggbb`, a basic name (black white gray silver red orange yellow green lime blue navy cyan purple pink brown beige gold), or an expression giving a colour (a color param) |
| `place` | where the part sits in the assembly; default: at the origin |
| `enabled` | boolean / expression; a disabled part is skipped |
| `print_rotate` | `[x, y, z]` degrees applied before laying the part on the bed |
| `allow_disconnected` | `true` if the part is intentionally several separate bodies |

## features

```json
{ "id": "post", "shape": { "type": "cylinder", "d": 6, "h": 10 }, "place": { "on": "inside_bottom", "offset": [20, 0, 0] },
  "repeat": { "grid": [2, 2], "spacing": [40, 30] }, "mirror": "x", "enabled": "posts" }
```

`shape` is required. `id` lets later features use this feature's anchors (`"post.top"`) and lets other parts reference it (`"box.post.top"`). The anchors are those of the first instance, before repeat or mirror.

## place

| key | notes |
|---|---|
| `on` | target anchor, see below |
| `at` | `[x, y, z]`: instead of `on`, put the shape's origin (or its `anchor`, if given) at these coordinates |
| `anchor` | the anchor of the thing being placed that goes onto the target |
| `offset` | `[x, y, z]` in the parent's axes (part axes for features, world axes for parts), applied last |
| `rotate` | `[x, y, z]` degrees, applied to the shape first, about X then Y then Z |
| `orient` | `"normal"`: turn the shape so its +Z points along the target face's outward normal (default for features). `"none"`: no turning (default for parts). |

**Target names** (`on`):

- In a feature: `"top"`, `"inside_bottom"` (anchors of the part's base shape; `"base.top"` is the same), or `"<feature id>.<anchor>"` for an earlier feature of the same part.
- In a part: `"<part id>.<anchor>"`, `"<part id>.<feature id>.<anchor>"`, or `"origin"`.

**Anchor names**: `top bottom left right front back center` and combinations joined with `-` (`top-left`, `front-bottom`, `bottom-back-right`). The first word picks the face (and its normal); the others move the point to that edge or corner. Components may add named anchors (hollow_box: `inside_bottom`; lid: `seat`).

**Default `anchor`** when you leave it out:

| what is placed | with orient normal | with orient none |
|---|---|---|
| add feature | `bottom` (stands on the face) | opposite of the target's face word (`on: "front"` → `back`) |
| cut feature | `top` (sinks into the face) | same as the target's face word |
| part | — | opposite face word, other words kept (`on: "box.top-left"` → `bottom-left`) |

Face orientation with `orient: "normal"` (local axes of the placed shape → world):

| face | local X | local Y | local Z (out of the face) |
|---|---|---|---|
| top | +X | +Y | +Z |
| bottom | +X | -Y | -Z |
| front | +X | +Z (up) | -Y |
| back | -X | +Z (up) | +Y |
| right | +Y | +Z (up) | +X |
| left | -Y | +Z (up) | -X |

So a `slot` (long along local X) cut `on: "front"` runs horizontally, and a `box` with size `[w, h, depth]` cut on a side face is a rectangular window `w` wide and `h` tall, `depth` deep.

Cut features placed `on` a face are pushed 0.01 mm outwards automatically so they open cleanly at the surface.

## repeat

Exactly one of:

- `{ "grid": [nx, ny], "spacing": [sx, sy] }`: centre-to-centre spacing; the grid is centred on the placement point and laid out in the face's local X/Y.
- `{ "linear": n, "step": [dx, dy, dz], "center": true }`: `n` copies `step` apart in parent axes; centred on the placement point unless `"center": false`.
- `{ "radial": n, "radius": r, "start_angle": 0, "rotate": true }`: copies on a circle around the face normal through the placement point, the first at local +X.

Counts are rounded to whole numbers and must be 1 to 2000.

## mirror

`"x"` adds a copy mirrored across the part's plane x = 0, `"y"` across y = 0, `"xy"` adds three copies (both planes and the diagonal). Mirroring is applied after repeat.

## splits

See the multi-part skill.

```json
"splits": [ { "part": "tray", "axis": "x", "at": 0, "names": ["tray_left", "tray_right"],
              "joint": { "type": "dovetail", "count": 2 } } ]
```

| key | notes |
|---|---|
| `part` | id of the part (or of a piece from an earlier split) |
| `axis` | `"x"`, `"y"` or `"z"`: the cut plane is perpendicular to it |
| `at` | world coordinate of the plane on that axis (default 0) |
| `names` | `[negative side, positive side]` piece ids (default `<part>_a`, `<part>_b`) |
| `joint` | `{ "type": "dovetail" | "pin" | "magnet" | "none", ... }`: see components.md |

## print

`layer_height`, `infill` (%), `walls` (count), `supports` (boolean; true silences overhang warnings), `nozzle` (mm, default 0.4; sets the thin-wall limit), `material` (text), `bed` (`[x, y, z]` mm, default 256 × 256 × 256).
