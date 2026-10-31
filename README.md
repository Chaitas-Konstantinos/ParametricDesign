# ParametricDesign

**Describe an object in words and photos. An AI designs it. You fine-tune it with sliders and print it.**

ParametricDesign turns a description like *"a 400 mm desk tray with three compartments that fits my 220 mm printer"* into a 3D model you can adjust and 3D print. You don't have to learn CAD.

1. **You describe** what you need to the AI you already use (Claude, ChatGPT, Gemini or another), with photos if you have them.
2. **The AI designs it.** Our **skills** (instruction packs) teach it to write the design as a small **JSON** file. The design is *parametric*: every size that matters (width, wall thickness, number of compartments) is a named setting with a range.
3. **The AI checks its own work.** Through our **MCP server** (a local connector), the AI can validate the design, render preview images of it, take measurements and fix its mistakes before you see the design.
4. **You open it in the desktop app.** The design shows in 3D with sliders for every setting. You measure it, cut through it, compare versions and save presets.
5. **You print it.** Export STL or 3MF. A 3MF file keeps the colours and parts and is laid out on your printer's bed. It also contains the design itself, so the file opens again as an editable design.

Later, a **catalog website** will let people share, search and customise each other's designs in the browser.

The project is **open core**: the app, engine, skills and catalog are free and open source, and paid extras (cloud sync, private designs, hosted AI) may come later. The app is **Windows first**; the code is cross-platform.

---

## Contents

- [Who it's for](#who-its-for)
- [How it works, end to end](#how-it-works-end-to-end)
- [The design format](#the-design-format)
- [The engine](#the-engine)
- [Validation and printability checks](#validation-and-printability-checks)
- [Exports](#exports)
- [Skills](#skills)
- [The MCP server (connector)](#the-mcp-server-connector)
- [The desktop app](#the-desktop-app)
- [Where your data lives](#where-your-data-lives)
- [The `pd` command line](#the-pd-command-line)
- [Evals: measuring how good the AI is](#evals-measuring-how-good-the-ai-is)
- [Architecture](#architecture)
- [Repository layout](#repository-layout)
- [Build and run](#build-and-run)
- [Status and roadmap](#status-and-roadmap)
- [Key decisions](#key-decisions)
- [Known risks](#known-risks)
- [License](#license)

---

## Who it's for

- **3D-printing hobbyists** who want custom boxes, brackets, organisers, enclosures, spacers, vases and adapters without learning CAD.
- **People who already design** and want a fast way to get a parametric starting point they can tweak.
- **Makers who share designs**: a parametric design isn't one fixed file; anyone can resize it for their own case.

The app has two modes. **Simple** shows only the main settings with friendly labels. **Advanced** also shows tolerances, clearances and technical options. Laser cutting and CNC are low priority; the focus is FDM 3D printing.

---

## How it works, end to end

```
 ┌──────────────── Your AI (Claude, ChatGPT, Gemini, local models…) ───────────────┐
 │   our skills  +  your description  +  your photos                               │
 └────────────────┬──────────────────────────────────────────▲─────────────────────┘
                  │ MCP tools (or copy and paste the JSON)   │ errors, preview images, measurements
 ┌────────────────▼──────────────────────────────────────────┴─────────────────────┐
 │ ParametricDesign desktop app (Tauri, Windows)                                    │
 │   Interface (React + three.js): library · 3D editor · sliders · measure · compare│
 │   Rust engine: format · expressions · components · anchors · validation         │
 │                Manifold geometry kernel · STL / 3MF export · preview renderer   │
 │   pd.exe: MCP server · skill catalog and bundler · command line                 │
 └────────────────┬──────────────────────────────────────────────────────────────────┘
                  │ publish (later)
 ┌────────────────▼──────────────────────────────────────────────────────────────────┐
 │ Catalog website (planned): static site + Supabase                                 │
 │   The same Rust engine runs in the visitor's browser as WebAssembly:              │
 │   preview, simple sliders, STL/3MF download. No geometry work on the server.      │
 └───────────────────────────────────────────────────────────────────────────────────┘
```

There are three ways to get a design from an AI into the app:

| Way | For | What happens |
|---|---|---|
| **MCP connector** | Claude Desktop, Claude Code, Cursor, LM Studio, Codex, Gemini CLI and anything else that supports MCP | The AI calls our tools directly: reads the guide, validates, renders previews, measures and sends the design straight into the app. Best quality, because the AI sees and fixes its own mistakes. |
| **Tailored skill** | Apps that support Agent Skills (for example Claude.ai) | The app exports one `.zip` skill made of the core skills, the optional skills you picked, your description and photos. You upload it and send the starter message. |
| **One prompt** | Any chatbot | The app builds a single prompt with everything in it. You paste it in, then paste the JSON the AI writes back into the app. The app's checks give you a "fix-it message" to send back if something is wrong. |

### A typical session

1. In the app: **New design → Design with AI**. Type a name and what you need, add photos (with captions such as "the shelf is 32 cm deep"), and tick the optional skills (for example *Multi-part* + *Dovetail connector* for something bigger than your printer). Your printer is filled in.
2. Click **Send to Claude**. The app saves the request, and a short message is copied for you to paste into your AI.
3. The AI calls `get_design_request` and gets your description and photos, then `get_design_guide` for the skills you picked. It writes the design and calls `validate_design` and `render_preview` until it's right, then `open_in_app`.
4. The app shows a notification: the design is in your library under **New from Claude**.
5. You open it, move the sliders, save presets ("for the big drawer") and export a 3MF for your printer.
6. Later you ask the AI: *"make it 20 mm taller"*. It reads the open design with `get_open_design` and sends an update. The app opens a **side-by-side compare** of the old and new versions with the changed parameters and parts listed. You choose **Accept**, **Save as new** or **Reject**.

---

## The design format

A design is one JSON file (`.pd.json`), format `pd/0.1`. It was designed for three things, in this order:
1. **An AI gets it right on the first try.**
2. **It can be checked** with exact error messages that say how to fix the problem.
3. **It doesn't depend on one geometry kernel.**

```json
{
  "format": "pd/0.1",
  "meta": {
    "name": "Lidded box",
    "category": "storage/boxes",
    "tags": ["box", "lid", "snap-fit"],
    "license": "CC-BY-4.0",
    "skills": ["core-parametric", "printability", "connector-snap-fit"]
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

**Params** are the settings the user can change. Each one carries everything the interface needs: value, min, max, step, unit, label, group, and `advanced: true` to hide it in Simple mode. Types: number, integer, boolean, choice, colour and text.

**Derived values and expressions.** A safe little maths language computes any value from the params:
- operators: `+ - * /`, comparisons;
- functions: `min`, `max`, `clamp`, `abs`, `round`, `floor`, `ceil`, `sqrt`, `pow`, the trig functions (`sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`), `if(cond, a, b)` and `inch()`.

There are no loops: repeat components do repetition. Mistakes such as unknown names, cycles or division by zero give clear errors with "did you mean" suggestions.

**Components** do the hard geometry so the AI doesn't have to. There are 15 today:
- basic shapes: `box`, `cylinder`, `sphere`, `tube`, `prism`, `revolve`, `torus`, `wedge`, `slot`;
- boxes and lids: `hollow_box`, and `lid` with a built-in snap fit;
- hardware: `screw_hole` (M2–M8, countersinks), `nut_trap`, `magnet_pocket`, `standoff`.

Components can be combined (add, subtract, intersect), repeated (linear, grid, radial) and mirrored.

**Anchors.** Every component has named points such as `top`, `bottom`, `front`, `center`, plus special ones like a lid's seat. Parts are placed by anchor (`"on": "box.top"`) instead of raw X/Y/Z coordinates. This removes the most common AI mistake: parts floating in the wrong place.

**Splits and joints.** A large part can be split into pieces that fit your printer's bed and joined with **dovetail** keys, **alignment pins** or **magnets**. The engine cuts the joints and adds the clearances.

**Multi-part designs.** Each part has a position in the assembly (for the viewer) and gets its own print orientation (for the build plate and export).

---

## The engine

`crates/pd-core` is the heart of the project, written in pure Rust:

- reads the design, evaluates the expressions and builds the geometry for any set of param values;
- uses **[manifold-rust](https://crates.io/crates/manifold-rust)**, a pure-Rust port of the Manifold geometry kernel. It is fast and robust, and always produces watertight meshes that print;
- needs **no C++ toolchain**, so the same engine compiles to **WebAssembly** (about 2.9 MB) and runs in the browser exactly as on the desktop;
- computes measurements: bounding boxes, distances and clearances between parts, wall thickness.

The engine is reached through:
- `pd-app`, a JSON API used by the desktop app;
- `pd-wasm`, the same API for the browser;
- `pd-cli`, the command line;
- `pd-mcp`, the AI connector.

---

## Validation and printability checks

The same checks run in the app, over MCP, in the command line, in CI and (later) when a design is published. Each error is written **for the AI**: the JSON path, what's wrong and how to fix it.

1. **Format**: types, required fields, unknown component or anchor names (with "did you mean").
2. **Expressions**: undefined names, cycles, division by zero.
3. **Geometry at every extreme**: the design is built at the default value **and at the min and max of every param**. A design that breaks when someone drags a slider to the end is caught before the user finds it.
4. **Printability**:
   - watertight output;
   - walls too thin for the nozzle;
   - separate parts that overlap by accident;
   - pieces floating in the air;
   - fits the bed of **your** printer;
   - overhang warnings.

In the app, **Check every slider value** runs the full check, and **Copy fix-it message** gives you a ready message to paste back to your AI.

---

## Exports

| Format | Contents |
|---|---|
| **STL** | Binary. Whole design, or one file per part. |
| **3MF** | Each part as a separate object with its colour (maps to filament slots, AMS/MMU), plates laid out automatically for your printer's bed, print settings, a thumbnail, and the **design plus your chosen values embedded**, so opening the 3MF in the app gives you the editable design back. Tested with Bambu Studio, OrcaSlicer and PrusaSlicer. |

File names include the design name and preset. Planned formats are OBJ and GLB (milestone 2) and STEP (milestone 3, with the OpenCascade kernel).

---

## Skills

Skills are instruction packs in the open **Agent Skills** layout (a `SKILL.md` with references and examples). They teach an AI the format, good parametric habits and 3D-printing rules. Every skill is in `skills/`, and `skills/catalog.json` lists them.

**Core skills** are always included:

| Skill | Teaches |
|---|---|
| `core-parametric` | The `pd/0.1` format, components, anchors, expressions, what makes a good parameter (named in the user's words, sensible ranges, tolerances marked advanced), not asking needless questions, and the **validate → render → fix** loop. It also says to update an existing design by its id instead of making a duplicate. |
| `printability` | FDM rules: walls, tolerances and clearances, overhangs, orientation, bed size. |
| `from-photos` | Estimating sizes from photos, sketches and reference objects, and turning every uncertain size into a parameter. |

**Optional skills**, which you tick when you need them:

| Skill | Use it for | Notes |
|---|---|---|
| `multi-part` | Objects printed as separate pieces: splitting big parts, assemblies, clearances | |
| `connector-dovetail` | Pieces joined by sliding dovetail keys (flat or tray-like objects) | needs multi-part; pick one connector |
| `connector-pin` | Pieces aligned with printed pins, filament or dowels, then glued | needs multi-part; pick one connector |
| `connector-magnet` | Pieces held together by disc magnets: removable, any cut direction | needs multi-part; pick one connector |
| `connector-snap-fit` | Boxes whose lids click on | |
| `connector-screws` | Metric screw holes, countersinks, nut traps, heat-set inserts | |
| `electronics-enclosure` | Cases for circuit boards: standoffs, port cut-outs, vents, screwed lids | needs screws |

**Tailored skill.** The app (or `pd skills bundle`) merges the core skills, the ticked skills, your description, your photos and your printer into **one skill**. You get it as a folder or a `.zip`, plus a starter message. It can also become **one prompt** for chatbots without skill support. Any single skill can be exported on its own too.

Planned skills include hinges, threads, text and labels, multicolour and Gridfinity.

---

## The MCP server (connector)

`pd mcp` runs a local **MCP server** over stdio. It ships inside the app as `pd.exe`. Any MCP client can use it, and it offers 10 tools:

| Tool | What the AI uses it for |
|---|---|
| `get_design_guide` | Load the instructions for the chosen skills |
| `list_components` | Reference for every component, anchor and joint |
| `validate_design` | Check a design: errors and warnings with fixes |
| `render_preview` | Get **images** of the design (several views, exploded, print layout, section, highlighted part), so the AI can *see* what it made |
| `measure` | Sizes, distances and clearances between parts |
| `get_open_design` | Read the design open in the app, with the user's current slider values |
| `list_designs` | Search the library by name, category or tag, to update an existing design instead of creating a duplicate |
| `get_design_request` | Get the request prepared in the app's **Design with AI** screen: description, printer, chosen skills and the **photos as images** |
| `open_in_app` | Send a **new** design to the library, or an **update** to an existing one (it opens the compare view) |
| `export_design` | Write STL or 3MF files |

The server also offers a `design` prompt (a brief plus optional skills).

There are no permission prompts for these tools if you allow them in your client. Only the app and the AI client talk to the server. It runs on your computer and sends nothing to the internet.

**Connecting a client.**

Claude Code:

```bash
claude mcp add --scope user parametricdesign -- "C:\Program Files\ParametricDesign\pd.exe" mcp
```

Claude Desktop (Settings → Developer → Edit Config):

```json
{
  "mcpServers": {
    "parametricdesign": {
      "command": "C:\\Program Files\\ParametricDesign\\pd.exe",
      "args": ["mcp"]
    }
  }
}
```

Other clients (Codex, Gemini CLI, Cursor, LM Studio…) take the same command: `"<path to pd.exe>" mcp`.

---

## The desktop app

Built with **Tauri 2** (a Rust backend and a web interface), **React 19** and **three.js**. It follows Windows' light or dark theme.

### First start
A short setup:
1. **Library folder.** The default is `Documents\ParametricDesign Library`. A OneDrive or Dropbox folder works for using the same library on several computers. You can add 5 example designs.
2. **Your printer.** Bambu Lab A1, A1 mini, P1S and X1 Carbon, Prusa MK4S and CORE One, Creality Ender-3 V3, or a custom bed size and nozzle.

### Library (home screen)
- Cards with automatic thumbnails, search (Ctrl+F) by name, tag and category, and sorting.
- **Favorites**, **Recent**, **New from Claude**, **Collections** (a design can be in several), **Categories**, **Tags**, **Trash** (restore, delete forever, empty).
- **Version tag** on each card (for example `v3/3`): click it to list every version and open any of them.
- **Combine**: designs with the same name (or any two you pick) merge into one design with all their versions, ordered by date. A banner offers this when duplicates appear, or you can keep them separate.
- **New design**:
  - **Design with AI…**;
  - **Import** a `.pd.json` or `.3mf`;
  - **Paste JSON**;
  - **Add example designs**.
- Card menu: open, favorite, rename, duplicate, combine with…, collections, tags & category, move to trash.

### Editor
- **Viewer:**
  - orbit, pan and zoom towards the cursor;
  - **view cube** with standard views (Home and Fit), orthographic or perspective;
  - display modes: shaded, shaded + edges, wireframe, x-ray;
  - **measure** with snapping;
  - **section** along X, Y or Z with filled caps;
  - **build plate** view, **exploded** view.
- **Sliders and number boxes** for every param, grouped, in **Simple** or **Advanced** mode. An unsaved-changes dot shows when you've changed something.
- **Undo / redo** (Ctrl+Z / Ctrl+Y).
- **Presets**: named sets of values (save, switch, rename, overwrite, delete).
- **Save (Ctrl+S)** creates a new **immutable version** with an automatic description of what changed. **Version history** opens any version or copies it as a new design. **Save as new design** is also available.
- **Export** (Ctrl+E): 3MF, STL, STL per part.
- **Checks**: problems at the current values, **Check every slider value**, **Copy fix-it message**.
- Leaving with unsaved changes asks Save / Don't save / Cancel.
- Keyboard shortcuts:

  | Key | Action |
  |---|---|
  | F | Fit |
  | H or 0 | Home view |
  | 1 / 2 / 3 | Standard views |
  | P | Perspective / orthographic |
  | D | Display mode |
  | M | Measure |
  | X | Section |
  | E | Exploded |
  | Esc | Cancel / close |

### Compare (updates from the AI)
When the AI sends a change to a design:
- you see the old and new design **side by side** or **overlaid**;
- the changed parameters and parts are listed;
- **Accept** saves it as a new version; **Save as new** makes a separate design; **Reject** keeps it in the history marked "not accepted".

An update that arrives while the app is closed shows at the next start.

### Design with AI
The skill picker:
- **name** and **description**;
- **photos** (choose, drop or paste; up to 10, each with a caption; resized automatically);
- your **printer**;
- **skills**: core skills locked on, optional skills ticked. Required skills follow automatically, and only one skill per group (for example, one connector).

Three ways out:
1. **Send to Claude** (desktop): saves the request for `get_design_request` and copies a starter message.
2. **Export tailored skill** (`.zip`) plus a starter message.
3. **Copy prompt** or **Save as file** for any chatbot.

### Settings
- **Look:** theme (follow Windows, light, dark), accent colour, text size.
- **Units:** mm or inches.
- **Printer:** printer profiles (bed size and nozzle feed the checks, the build plate and the 3MF layout).
- **Connecting your AI:** the `pd.exe` path and connection commands, with copy buttons.

### Browser mode
`npm run dev` runs the **same interface in a web browser** on the WebAssembly engine, with the library kept in browser storage. It's used for development and tests, and it's the start of the catalog website.

---

## Where your data lives

| What | Where |
|---|---|
| **Library** | The folder you chose (default `Documents\ParametricDesign Library`). Each design is a folder of **immutable version files** (`v0001-<time>.json`, …) plus its presets, tags, collections, thumbnail and pending AI updates. Saving never overwrites a file, so if OneDrive or Dropbox syncs two computers at once, you get an extra version instead of lost work. |
| **Per-computer settings** | `%APPDATA%\ParametricDesign` on Windows (`~/.config/parametricdesign` elsewhere; override with `PD_HOME`): where the library is, theme, printer, the inbox the MCP server writes new designs and updates to, the open design for `get_open_design`, and the last 20 Design with AI requests (with their photos). |
| **The app** | MSI installer: `C:\Program Files\ParametricDesign` (with `pd.exe` inside). |

Nothing leaves your computer. The app has no account, no telemetry and no server.

---

## The `pd` command line

```
pd validate <design.pd.json> [--set name=value]... [--json] [--quick]
pd render   <design.pd.json> [-o out.png] [--views iso,front,right,top] [--layout assembled|exploded|print]
                             [--section y=0] [--highlight <part>] [--set name=value]...
pd export   <design.pd.json> --format stl|3mf [-o out] [--part <id>] [--assembled] [--set name=value]...
pd measure  <design.pd.json> [--query '{"type":"distance","a":"box","b":"lid"}']... [--set name=value]...
pd docs components                Markdown reference of every component and joint
pd skills list                    Skills in the catalog
pd skills export <id> [-o dir|file.zip]
pd skills bundle [--with a,b] [--title name] [--brief text | --brief-file f] [--photo img]...
                 [-o dir|file.zip] [--prompt]
pd mcp                            Run the MCP server on stdio
```

`--set` overrides params, for example `--set width=120 --set style=round`.

---

## Evals: measuring how good the AI is

Skill quality decides the whole product, so it is measured. `evals/` has:
- **20 real requests**: boxes, brackets, enclosures, split trays, vases and more, each with the skills a user would pick;
- **automatic checks**: valid at every param extreme, number of parts, components used, joint type, overall size, fits the bed.

`evals/run.py` runs them against the Claude API in two modes:
- `--mode oneshot`: the tailored skill only. This measures **first-try success**.
- `--mode tools`: the skill plus the MCP tools. This measures **success after self-correction**.

The targets are **80 % on the first try** and **95 % after self-correction**. Every skill change should be checked with the evals. A run costs API credit, so you need an Anthropic API key with a spending limit.

---

## Architecture

```
                       ┌───────────────┐
                       │   pd-core     │  format · expressions · components · anchors
                       │  (pure Rust)  │  joints · validation · checks · measurements
                       └──────┬────────┘
          ┌───────────────────┼───────────────────────┬───────────────┐
   ┌──────▼─────┐      ┌──────▼──────┐         ┌──────▼──────┐  ┌─────▼──────┐
   │ pd-export  │      │  pd-render  │         │ pd-library  │  │ pd-skills  │
   │ STL · 3MF  │      │ CPU preview │         │ designs ·   │  │ catalog ·  │
   └──────┬─────┘      │  renderer   │         │ versions ·  │  │ bundler ·  │
          │            └──────┬──────┘         │ settings ·  │  │ prompts    │
          │                   │                │ inbox       │  └─────┬──────┘
          └─────────┬─────────┴────────┬───────┴──────┬──────┴────────┘
             ┌──────▼──────┐    ┌──────▼──────┐ ┌─────▼──────┐
             │   pd-app    │    │   pd-mcp    │ │   pd-cli   │ ──► pd.exe (CLI + `pd mcp`)
             │ JSON API    │    │ MCP server  │ └────────────┘
             └──┬───────┬──┘    └─────────────┘
        ┌───────▼──┐ ┌──▼──────────┐
        │ Tauri    │ │  pd-wasm    │
        │ backend  │ │ WebAssembly │
        └───────┬──┘ └──┬──────────┘
                └───┬───┘
            ┌───────▼─────────────────────┐
            │ apps/desktop (React,        │  same interface on the desktop
            │ three.js)                   │  and in the browser
            └─────────────────────────────┘
```

- **The app and the AI talk through files**, not network sockets. The MCP server writes new designs and updates to the inbox and reads the open design and requests from the per-computer folder, and the app watches that folder. This works even when the app is closed.
- **One engine everywhere.** Desktop, browser, CLI and MCP all run the same Rust code, so a design that validates in one place validates everywhere.
- **No build scripts in new crates.** Windows **Smart App Control** blocks unsigned programs compiled on your own PC, and that includes Rust build scripts (error `os error 4551`). New code avoids crates that need them; the skill files are embedded with a checked-in `embedded.rs` instead of a `build.rs`.

---

## Repository layout

| Path | What |
|---|---|
| `crates/pd-core` | The engine (see above) |
| `crates/pd-export` | STL and 3MF export |
| `crates/pd-render` | Headless CPU renderer for labelled preview images (no GPU, works in CI and the browser) |
| `crates/pd-mcp` | The MCP server and its 10 tools |
| `crates/pd-library` | Library on disk: designs, versions, presets, pending updates, collections, trash; per-computer settings, inbox, requests |
| `crates/pd-skills` | Skill catalog, tailored bundles (folder or `.zip`), single prompts, starter messages |
| `crates/pd-cli` | The `pd` command line |
| `crates/pd-app` | The app's engine API (JSON in, JSON out) |
| `crates/pd-wasm` | The same API compiled to WebAssembly |
| `apps/desktop` | The desktop app: `src/` (React interface: `screens/` Library, Editor, Compare, Ask (Design with AI), Settings; `components/` Viewer, ParamPanel, menus, icons) and `src-tauri/` (Rust backend) |
| `skills/` | The skills given to AIs, plus `catalog.json` |
| `examples/` | Reference designs (snap-lid box, dovetail tray, board enclosure, vase, wall bracket), validated by the tests |
| `evals/` | Eval cases and runner |
| `docs/SPEC.md` | Product and technical spec, with every decision |
| `docs/CHECKLIST.md` | Everything done and still to do, in order |
| `.github/workflows/ci.yml` | On every push: formatting, all Rust tests, WebAssembly build and interface build (Linux) |
| `.github/workflows/windows-installer.yml` | Windows MSI / NSIS installers with `pd.exe` inside; run by hand from the Actions tab |

---

## Build and run

You need **Rust** (stable) and **Node 22**. On Linux, Tauri also needs `libwebkit2gtk-4.1-dev` and its companions.

**Command line and MCP server**

```bash
cargo build --release -p pd-cli
```

```bash
./target/release/pd validate examples/snap-lid-box.pd.json
```

```bash
./target/release/pd render examples/snap-lid-box.pd.json --layout exploded -o box.png
```

```bash
./target/release/pd export examples/snap-lid-box.pd.json --format 3mf --set width=120
```

```bash
./target/release/pd skills bundle --with multi-part,connector-dovetail --brief "A 400 mm desk tray for a 220 mm bed" -o my-skill.zip
```

**Desktop app**

```bash
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

```bash
cd apps/desktop && npm install && npm run wasm
```

```bash
npx tauri dev --release
```

`--release` reuses build programs Windows has already approved, which avoids Smart App Control errors. `npm run dev` alone runs the interface in a browser.

**Windows installer.** Put `pd.exe` in `apps/desktop/src-tauri/binaries/`, then:

```bash
npx tauri build --config src-tauri/tauri.installer.conf.json
```

Install with the **MSI**. It installs per machine and upgrades older versions in place. Alternatively, run the *Windows installer* workflow on GitHub.

**Tests and evals**

```bash
cargo test --workspace
```

```bash
python3 evals/run.py --mode tools --cases long-tray
```

---

## Status and roadmap

**Current version: 0.2.1.** The full list is in [docs/CHECKLIST.md](docs/CHECKLIST.md).

**Done**
- **Milestone 1, the AI → design loop:**
  - the format, the engine with 15 components, anchors and joints, validation, checks;
  - STL and 3MF export, the preview renderer;
  - the CLI and the MCP server;
  - 3 core and 7 optional skills, the bundler;
  - the eval suite.
- **The desktop app:** library, editor, compare, versions, presets, combine, settings, first start, Design with AI, browser mode.

**Designed, waiting for approval: the AI panel.** A terminal built into the app that runs the AI tool you already have:
- **Claude Code** (your Claude plan), **Codex** (your ChatGPT plan) or **Gemini CLI** (free with a Google account), with no API key and no cost per message;
- one-click **Install** and **Connect** for each tool, and an "AI workspace" folder with instruction files for each tool;
- **Add photo**, **Use my Design with AI request**, and a "*Codex sent a change*" card that leads to Review changes;
- a new Settings window and an AI tool step in first start.

**Next**
- **Releases:** Windows **code signing** (so Smart App Control and SmartScreen don't block the app), an automatic **updater**, GitHub Releases, making the repository public.
- **Milestone 2, the rest of the desktop app:**
  - dimension labels linked to sliders;
  - OBJ/GLB export, full 3MF print settings;
  - more components (hinge, text, fillet/chamfer, threads) and more skills;
  - fit profiles per printer (tight / normal / loose).
- **Milestone 3, OpenCascade:** a second geometry kernel for true fillets and **STEP** export, with a kernel switch (desktop only).
- **Milestone 4, the catalog website:**
  - static site plus **Supabase** (accounts, database, file storage, pgvector for natural-language search);
  - browse by category, tag and filter (connectors, multicolour, supports, "fits my printer");
  - preview and customise in the browser on the WebAssembly engine, then download STL/3MF;
  - publish from the app, with the server re-validating every design;
  - versions, thumbnails and turntable images.
- **Milestone 5, grow:** macOS and Linux builds, community skills, paid extras (cloud sync, private designs, hosted AI generation) in a separate private service.

**Decisions only the owner can make:**
- publishing as an individual or a company (this affects code signing);
- signing provider and budget;
- an API key for the evals;
- when to go public;
- the final name and logo, and the domain;
- the embeddings provider for search;
- terms of use and privacy policy.

---

## Key decisions

| Topic | Decision |
|---|---|
| Platform | Desktop app (full power) + catalog website (browse, preview, simple customise) |
| Desktop technology | Tauri 2: Rust core + React/TypeScript/three.js interface |
| First OS | Windows; code kept cross-platform |
| Design format | JSON from high-level components, placed by anchors, values as expressions |
| Geometry | Manifold now (pure Rust, runs in WebAssembly); OpenCascade later for STEP and true fillets |
| AI access | No built-in AI model to pay for. Users bring their own AI plus our skills; the MCP connector is the main path, paste is the fallback |
| Skills | Core skills always on, optional ones ticked, merged into one tailored skill |
| Audience | Everyone, with Simple and Advanced modes; focus on 3D printing |
| Website backend | Supabase + static frontend on a CDN; geometry runs in the visitor's browser |
| Storage | Immutable version files in a user-chosen folder (cloud-sync safe) |
| Business model | Open core |
| Units | mm inside; mm or inch display |
| Language | English first, ready for translation |

---

## Known risks

1. **Skill quality** decides the product. The evals gate every skill change.
2. **Windows code signing and Smart App Control.** Unsigned apps get blocked or warned about; this needs a certificate or signing service with a yearly cost.
3. **OpenCascade from Rust.** The bindings are young, so it stays desktop only (milestone 3).
4. **Third-party AI rules.** Apps may not sign in to a user's Claude.ai account on their behalf. The design keeps users on their own official tools (MCP connector, their own CLI) or their own API key.

Two early risks are solved: Manifold runs natively and in WebAssembly from one pure-Rust core, and headless rendering uses a small CPU renderer.

---

## License

Code: **MIT OR Apache-2.0**. Example designs: **CC-BY-4.0** unless marked otherwise. Authors choose a Creative Commons license for designs they publish (default CC-BY-4.0).
