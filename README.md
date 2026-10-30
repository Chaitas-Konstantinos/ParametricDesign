# ParametricDesign

Describe a design to any LLM using our skills, then open the result in a desktop app to view it in 3D, fine-tune its parameters and export STL / 3MF. A catalog website for community designs comes in a later milestone.

Product and technical spec: [docs/SPEC.md](docs/SPEC.md). What's done and what's next: [docs/CHECKLIST.md](docs/CHECKLIST.md).

## What's here (milestone 1)

| path | what |
|---|---|
| `crates/pd-core` | The engine: the `pd/0.1` JSON design format, expressions, 15 components, placement by anchors, splits with dovetail / pin / magnet joints, validation at every param's min and max, printability checks, measurements. Pure Rust on [`manifold-rust`](https://crates.io/crates/manifold-rust), so it also runs in the browser. |
| `crates/pd-export` | STL and 3MF export (parts, colours, plate layout, thumbnail, the editable design embedded). |
| `crates/pd-render` | Headless renderer for labelled preview images (used by the MCP server so LLMs can see their design). |
| `crates/pd-mcp` | MCP server: `validate_design`, `render_preview`, `measure`, `get_open_design`, `open_in_app` (new design or update to the open one), `export_design`, `list_components`, `get_design_guide`. |
| `crates/pd-library` | The design library on disk: designs, immutable versions, presets, pending updates from Claude, collections, trash; per-computer settings and the inbox the MCP server writes to. |
| `crates/pd-skills` | The skill catalog: export one skill, or bundle the core skills plus chosen ones into one tailored skill. |
| `crates/pd-cli` | `pd`, the command line for all of the above. |
| `crates/pd-app`, `crates/pd-wasm` | The app's engine API, natively and as WebAssembly. |
| `apps/desktop` | The desktop app (Tauri 2, React, three.js). |
| `skills/` | The skills given to LLMs. |
| `examples/` | Reference designs, validated by the tests. |
| `evals/` | 20 real requests and a harness measuring how often an LLM gets the design right. |

## Build and try it

Requirements: Rust (stable), Node 22.

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
./target/release/pd skills bundle --with multi-part,connector-dovetail --brief "A 400 mm desk tray for a 220 mm bed" -o my-skill
```

### Use it from Claude Desktop (or any MCP client)

Add the server to the client's MCP config (Claude Desktop: Settings → Developer → Edit Config):

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

Use the path to your `pd` binary. Then ask for a design: the LLM loads the guide, writes the design, validates and previews it, fixes mistakes, and opens it in the desktop app. For LLMs without MCP, give them the tailored skill (or `pd skills bundle --prompt`) and paste their JSON into the app.

### Desktop app

```bash
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

```bash
cd apps/desktop && npm install && npm run wasm
```

```bash
npx tauri dev
```

On first start the app asks where your library should live (default `Documents\ParametricDesign Library`; a OneDrive or Dropbox folder works for several computers). The library is the home screen: search, favorites, recent, collections, categories, tags and trash. Opening a design gives the editor: view cube and standard views, orthographic/perspective, display modes, measure, section, build plate, exploded view, undo/redo, named presets, Save (Ctrl+S) creating a new version, and version history. When Claude sends an update to the open design you get a side-by-side compare and choose Accept, Save as new or Reject. Settings has theme (follows Windows), accent colour, text size, mm/inches, printer profile and the command to connect Claude.

Per-computer settings and the inbox live in `%APPDATA%\ParametricDesign` (`~/.config/parametricdesign` elsewhere; override with `PD_HOME`).

`npm run dev` alone runs the same UI in a browser, on the WebAssembly engine, with the library kept in browser storage. Windows installers (MSI / NSIS) are built by CI (`.github/workflows/ci.yml`) together with `pd.exe`. On Linux, Tauri needs `libwebkit2gtk-4.1-dev` and its companions.

### Tests and evals

```bash
cargo test --workspace
```

```bash
python3 evals/run.py --mode tools --cases long-tray
```

The evals call the Claude API (`pip install anthropic` and an API key) and cost credit; see `evals/README.md`.

## License

Code: MIT OR Apache-2.0. Example designs: CC-BY-4.0 unless marked otherwise.
