# ParametricDesign — Product & Technical Spec (v0, pre-development)

Status: **milestone 1 implemented** (see §15). Records the decisions made during the kickoff Q&A.

## 1. The idea in one paragraph

People describe what they need — in words and photos — to the LLM of their choice,
armed with our **skills**. The LLM writes a parametric design as **JSON**. Our
**desktop app** shows it in a 3D viewer with a parameter panel next to it, lets the
user measure and fine-tune it (Simple or Advanced mode), and exports print-ready
files (STL, 3MF with colors/plates/settings, STEP, OBJ/GLB). A **catalog website**
stores and categorizes everyone's designs; visitors can preview and do simple
customization in the browser, where all geometry work runs on their own device so
the server stays light.

## 2. Decisions

| Topic | Decision |
|---|---|
| Platform | **Desktop app** (full power) + **catalog website** (browse, preview, simple customize) |
| Desktop tech | **Tauri 2**: Rust core + web UI (TypeScript, React, three.js) |
| Desktop OS at launch | **Windows**. Code stays cross-platform; macOS/Linux later |
| Design format | **JSON** checked against a schema, built from high-level **components**, placed by named **anchors**, values can be **expressions** |
| Geometry kernels | **Manifold** (fast, robust mesh) **and OpenCascade** (B-rep, true fillets, STEP), selectable per design/export |
| Exports (v1) | **STL, 3MF, STEP, OBJ/GLB** |
| 3MF contents | Colors/filament per part, auto-arranged plates, print settings, embedded design + params |
| Viewer tools | Click-to-measure, dimension labels linked to params, section/cut view |
| LLM access | No built-in LLM at first. Users bring **any LLM** plus our **skills** |
| LLM → app | **Local MCP server** in the desktop app (validate / render / measure / open) **+ paste** as fallback |
| Skills | A **catalog of skills**: core skills are mandatory and auto-selected, the rest optional. The app **bundles the selection into one tailored skill**; any skill can also be exported on its own |
| Audience | Everyone, with **Simple** and **Advanced** modes. Focus on 3D printing/hobbyists and easy exports; laser/CNC is low priority |
| Website | **Preview + simple customize** in the browser (Rust core compiled to WebAssembly); STL/3MF download |
| Backend | **Supabase** (Postgres, Auth, Storage, pgvector) + static frontend on a CDN |
| Discovery | Categories + tags, natural-language search, filters (connector/skills used, multicolor, supports, fits my printer) |
| Publish quality | Automatic validation + auto-generated thumbnails/turntable |
| Business model | **Open core**: app, engine, skills and catalog free and open source; paid extras later (cloud sync, private designs, hosted AI generation) |
| First milestone | Prove the **LLM → design loop** |

## 3. System overview

```
 ┌────────────── User's LLM (Claude, ChatGPT, Gemini, local…) ──────────────┐
 │  tailored skill  +  photos  +  description                                │
 └───────────────┬──────────────────────────────────────▲────────────────────┘
                 │ MCP tools (or paste JSON)            │ errors, images, measurements
 ┌───────────────▼──────────────────────────────────────┴────────────────────┐
 │ Desktop app (Tauri, Windows)                                               │
 │  UI (React + three.js): viewer · param panel · Simple/Advanced · measure  │
 │  Rust core: schema · expressions · components · anchors · validation      │
 │             kernels: Manifold | OpenCascade                               │
 │             exporters: STL · 3MF · STEP · OBJ · GLB                       │
 │  MCP server (stdio) · skill catalog + bundler · publish to catalog        │
 └───────────────┬───────────────────────────────────────────────────────────┘
                 │ publish (design JSON + thumbnails)
 ┌───────────────▼───────────────────────────────────────────────────────────┐
 │ Catalog: static site on CDN  +  Supabase (DB, Auth, Storage, pgvector)    │
 │  Browser runs the same Rust core as WASM (Manifold only): preview,        │
 │  Simple-mode sliders, STL/3MF download. No server-side rendering.         │
 └───────────────────────────────────────────────────────────────────────────┘
```

## 4. Design format (JSON) — sketch

Goals, in order: **(1) an LLM gets it right on the first try**, (2) it can be checked
with exact, fixable error messages, (3) it doesn't depend on one kernel.

```json
{
  "format": "pd/0.1",
  "meta": {
    "name": "Lidded box",
    "category": "storage/boxes",
    "tags": ["box", "lid", "snap-fit"],
    "license": "CC-BY-4.0",
    "skills": ["core-parametric", "printability", "multi-part", "connector-snap-fit"]
  },
  "params": {
    "width":   { "value": 40,  "min": 10, "max": 200, "unit": "mm", "label": "Inner width", "group": "Size" },
    "height":  { "value": 20,  "min": 5,  "max": 100, "unit": "mm", "label": "Inner height", "group": "Size" },
    "wall":    { "value": 2,   "min": 1,  "max": 5, "step": 0.5, "unit": "mm", "advanced": true },
    "fit_tol": { "value": 0.2, "min": 0,  "max": 0.6, "step": 0.05, "unit": "mm", "advanced": true },
    "color":   { "value": "#ff7a00", "type": "color", "label": "Box color" }
  },
  "derived": { "outer_w": "width + 2*wall" },
  "parts": [
    { "id": "box",
      "shape": { "type": "hollow_box", "size": ["width", "width", "height"], "wall": "wall", "open": "top" },
      "color": "color" },
    { "id": "lid",
      "shape": { "type": "lid", "fits": "box", "clearance": "fit_tol",
                 "connector": { "type": "snap_fit", "count": 4 } },
      "place": { "on": "box.top", "align": "center" } }
  ],
  "print": { "layer_height": 0.2, "infill": 15, "walls": 3, "supports": false }
}
```

Key rules:

- **Params** carry everything the UI needs: range, step, unit, label, group, and
  `advanced: true` to hide them in Simple mode. Types: number, integer, boolean,
  choice, color, text.
- **Expressions**: a small, safe math language (`+ - * /`, `min`, `max`, `clamp`,
  `round`, `if(cond, a, b)`, trig). No loops; repetition comes from pattern components.
- **Components** do the hard geometry so the LLM doesn't have to:
  primitives (box, cylinder, sphere, cone, prism, extruded polygon), booleans,
  `hollow_box`, `lid`, `screw_hole`/`heat_insert` (M2–M8), `magnet_pocket`,
  `snap_fit`, `dovetail`, `pin_hole`, `hinge`, `text` (emboss/engrave),
  `fillet`/`chamfer`, patterns (linear, grid, radial), `mirror`.
- **Anchors**: every component exposes named anchors (`top`, `bottom`, `front`, …,
  `center`, plus component-specific ones like `lid.seat`). Parts are placed by anchor
  (`"on": "box.top"`), not raw XYZ, which removes the most common LLM mistake.
- **Multi-part**: each part has an assembled placement (for the viewer) and gets
  an automatic print orientation (for plates/export).
- **Kernel-agnostic**: every component is implemented for both kernels. Where
  Manifold can only approximate something (e.g. a true fillet), validation warns.

## 5. Validation (runs in the app, over MCP, in CI and on publish)

Errors are written **for the LLM**: JSON path + what's wrong + how to fix it.

1. Schema check (types, required fields, unknown component/anchor names with
   "did you mean").
2. Expression evaluation (undefined names, divide-by-zero, cycles).
3. Geometry build at **default, min and max** of every param (plus random samples).
4. Printability: manifold output, min wall thickness vs nozzle, unintended overlaps
   between separate parts, floating pieces, fits on the selected printer's bed,
   overhang warnings.

## 6. MCP server (desktop app)

Local stdio server, usable from MCP-capable clients (e.g. Claude Desktop, Cursor,
LM Studio). Tools:

| Tool | Purpose |
|---|---|
| `get_schema`, `list_components` | Reference docs, anchors and examples on demand |
| `validate_design(json)` | Errors/warnings as above |
| `render_preview(json, views, params?)` | PNG images (multi-view, exploded, section), so the LLM can **see** its result |
| `measure(json, query)` | Bounding boxes, distances, clearances between parts |
| `open_in_app(json)` | Load into the desktop viewer for the user |

The skills tell the LLM to loop **validate → render → fix** before handing over,
so the user rarely needs any back-and-forth.

## 7. Skills

Format: open Agent Skills layout (`SKILL.md` + references + examples).

**Core (mandatory, auto-selected)**
- `core-parametric`: the JSON format, components, anchors, expressions, what makes a
  good parameter (named in user terms, sensible ranges, tolerances marked advanced),
  ask-no-questions policy, and the MCP self-check loop.
- `printability`: walls, overhangs, tolerances/clearances, orientation, bed size.
- `from-photos`: estimating dimensions from photos and reference objects; making any
  uncertain dimension a parameter.

**Optional (examples)**
- `multi-part`: split into separately printed parts that connect.
- Connectors (pick one): `connector-snap-fit`, `connector-dovetail`, `connector-pin-hole`,
  `connector-magnet`, `connector-screw` (incl. heat-set inserts).
- `hinge`, `threads`, `text-and-labels`, `multicolor`, `electronics-enclosure`,
  `gridfinity`.

**In the app:** the core skills are locked on; the user ticks optional ones, adds a
description and photos → **Export tailored skill** (one merged skill package) and a
ready-to-paste starter message. Every skill can also be exported on its own. The MCP
server exposes the same skills so MCP clients can load them directly.

## 8. Desktop app UX

- **Simple mode**: only non-advanced params, grouped with friendly labels, presets,
  one big "Export for my printer" button.
- **Advanced mode**: all params (tolerances, clearances), a global fit profile per
  printer (tight/normal/loose), kernel switch, mesh resolution, export options,
  JSON editor.
- **Viewer**: orbit/pan/zoom, assembled ↔ exploded ↔ print layout, click-to-measure
  (distance, angle, radius, wall thickness; mm/inch), dimension labels linked to
  params (click jumps to the slider, drag changes the value), section/cut plane.
- **Printer profiles**: bed size, nozzle, AMS/MMU slots. Used for plates, validation
  and "fits my printer".

## 9. Exports

| Format | Notes |
|---|---|
| STL | Per part or merged; binary |
| 3MF | Parts as separate objects with colors → filament slots; auto-arranged plates for the chosen printer; print settings for Bambu Studio / OrcaSlicer / PrusaSlicer project metadata; source JSON + chosen params embedded so the file reopens editable |
| STEP | OpenCascade kernel only (desktop) |
| OBJ / GLB | With colors/materials, for rendering/AR/web |

The website offers STL and 3MF only (Manifold in WASM).

## 10. Catalog website

- **Frontend**: static site (Vite + React) on a CDN; uses the WASM build of the core
  and the shared viewer/param-panel components.
- **Supabase**: Auth (email, Google, GitHub), Postgres (designs, **versions**,
  categories, tags, skills used, printer-fit metadata), Storage (design JSON,
  thumbnails, turntables), pgvector (natural-language search; embeddings computed
  when a design is published).
- **Publishing**: the desktop app validates and renders thumbnails/turntable locally,
  then uploads. The server **re-validates** with the WASM core (never trusts the
  client) before the design is listed.
- **Paid extras (later)**: cloud sync, private designs, hosted AI generation. They stay
  in a separate private service, so the open-source parts stay fully usable alone.

## 11. Repository layout (proposed monorepo)

```
crates/
  pd-schema/            JSON types + design.schema.json generation
  pd-core/              expressions, components, anchors, validation
  pd-kernel-manifold/
  pd-kernel-occt/       (milestone 3)
  pd-export/            stl, 3mf, obj, glb, step
  pd-render/            headless preview renderer (for MCP)
  pd-mcp/               MCP server
  pd-cli/               `pd validate | render | export` (dev + CI + evals)
  pd-wasm/              browser build of the core (milestone 4)
apps/
  desktop/              Tauri app
  web/                  catalog site (milestone 4)
packages/
  viewer/               shared three.js viewer + param panel (TS)
skills/                 core/ and optional/ skills
examples/               reference designs (also used as tests)
evals/                  prompt suite measuring LLM first-try success
docs/
```

## 12. Roadmap

**M1: LLM → design loop** (proves the riskiest assumption)
- Schema v0.1, expression engine, ~15 core components, anchors.
- Manifold kernel, STL + basic 3MF export.
- `pd` CLI and MCP server (`validate`, `render_preview`, `measure`, `open_in_app`).
- Core skills + `multi-part` + 2 connector skills; skill bundler.
- Minimal Tauri window: viewer + param sliders + export.
- **Eval suite**: ~20 real requests (some with photos) → measure the % valid on the
  first try and after MCP self-correction. Proposed targets: ≥80% first try,
  ≥95% after self-fix.

**M2: Desktop app proper** — Simple/Advanced, measuring tools, dimension labels,
section view, full 3MF (colors, plates, print settings, embedded design), OBJ/GLB,
printer profiles, more components and skills.

**M3: OpenCascade** — second kernel, true fillets/chamfers, STEP export, kernel switch.

**M4: Catalog website** — Supabase, WASM core, browse/categories/tags/filters,
natural-language search, simple customize + download, publish from the desktop app
with server re-validation.

**M5: Grow** — signed Windows release pipeline, macOS/Linux builds, paid extras,
community-contributed skills.

## 13. Known technical risks (to spike early)

1. ~~**Manifold from one Rust core on native and in WASM.**~~ **Resolved in M1**: the
   pure-Rust port `manifold-rust` compiles natively and to `wasm32-unknown-unknown`
   with no C++ toolchain; the whole engine is a 2.9 MB WASM module.
2. **OpenCascade in Rust.** The bindings are young; desktop only (M3).
3. ~~**Headless rendering**~~ **Resolved in M1** with a small CPU rasteriser (no GPU,
   works in CI and in the browser).
4. **Skill quality** decides the whole product. The eval suite gates every
   skill change.
5. **Windows code signing**, to avoid SmartScreen warnings; needs a certificate or
   signing service (yearly cost).

## 14. Open items (defaults assumed unless changed)

| Item | Default |
|---|---|
| Product name | "ParametricDesign" as working name |
| Code license (open parts) | MIT OR Apache-2.0 (Rust convention) |
| Design licenses | Chosen by the author from a CC list (default CC-BY-4.0) |
| Units | mm internally; mm/inch display toggle |
| UI language | English first, i18n-ready |
| UI framework | React + TypeScript + three.js (react-three-fiber) |

## 15. Milestone 1 status

Done:

- `pd-core`: the pd/0.1 format, expressions, 15 components (box, cylinder, sphere,
  tube, prism, revolve, torus, wedge, slot, hollow_box, lid with snap fit, screw_hole,
  nut_trap, magnet_pocket, standoff), anchor placement, repeat/mirror, splits with
  dovetail / pin / magnet joints, validation at every param's min and max,
  printability checks, measurements. Errors carry a JSON path, a fix hint and
  "did you mean" suggestions.
- STL and 3MF export (parts, colours, plate layout, thumbnail, embedded design that
  reopens editable). OBJ/GLB move to M2.
- Headless preview renderer; MCP server (validate, render, measure, open in app,
  export, component list, design guide); `pd` CLI.
- Skills: 3 core + 7 optional, tailored bundling, single-prompt export; reference
  designs validated by tests.
- Desktop app (Tauri 2 + React + three.js): viewer, Simple/Advanced sliders,
  exploded view, STL/3MF export, open .pd.json/.3mf, paste JSON, inbox for
  `open_in_app`, "copy fix-it message for your LLM". The same UI runs in a browser
  on the WebAssembly engine.
- Eval suite: 20 requests + harness (one-shot and with MCP tools).

Library and editor (after M1, from user feedback):

- Library home in a folder the user picks (OneDrive/Dropbox-safe): every save is a
  new immutable version file, so sync conflicts become extra versions instead of
  lost work. Favorites, recent, collections, categories, tags, search, trash,
  automatic thumbnails, import of .pd.json / .3mf.
- Editor: view cube and standard views, ortho/perspective, display modes (shaded,
  shaded + edges, wireframe, x-ray), measure with snapping, section with caps, build
  plate, exploded view, undo/redo, named presets, manual Save with auto-written
  version messages, version history (open / copy as new design), Save / Don't save /
  Cancel when leaving with changes.
- Claude loop: `get_open_design` lets Claude read the open design and slider values;
  `open_in_app` with mode `update` sends a pending update that opens a side-by-side
  (or overlay) compare showing parameter and part changes; Accept saves a new
  version, Reject keeps it in history marked "not accepted". Mode `new` lands in the
  library as "New from Claude".
- Settings: theme follows Windows with a switch, accent colour, text size, mm/inch,
  printer profiles (bed and nozzle feed the checks, the plate and 3MF layout).

Not done yet / next:

- **Run the evals against real models** (needs an API key) and tune the skills
  until first-try ≥ 80 % and after-self-fix ≥ 95 %.
- Windows installer is built by CI but not yet signed; `pd.exe` should ship inside
  the installer with a "Connect to Claude Desktop" button (M2).
- M2 items from the roadmap: dimension labels, full slicer 3MF metadata (per-object
  filament slots, plates, print settings), OBJ/GLB, text component.
