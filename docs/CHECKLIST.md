# ParametricDesign: checklist

Everything we have decided since the kickoff, as one list: what is still to do, and
(at the end) what is already done. Decisions themselves are recorded in
[SPEC.md](SPEC.md).

- `[ ]` to do, `[x]` done.
- **You:** marks things only you can do: a decision, a purchase, an account, or
  testing on your PC. Claude can build everything else.

Last updated: 30 October 2026 (Design with AI added).

## Where things stand

- The repository is private. All code is on the branch `claude/dreamy-keller-sr1jue`,
  which is its only branch and also its default; there is no `main` branch and there
  are no pull requests yet.
- CI is green. Every push runs the engine tests, the WebAssembly build and an
  interface type check on Linux (a few minutes). The Windows installer is built
  only when you start it by hand: Actions → Windows installer → Run workflow.
- Your PC still runs 0.1.0, installed from the MSI. 0.2.0 (library, editor, compare,
  Design with AI) is not installed yet.
- 0.2.0 has been tested on Linux (all tests, a full click-through in the browser
  version, a short run of the desktop app). **Nothing has been tested on Windows yet.**

## Suggested order

1. Finish installing 0.2.0 (section 1), test it (section 2) and send feedback.
2. Fix what the testing finds (section 3).
3. Make the decisions in section 0, then set up code signing and updates
   (sections 5 and 6). Without signing, people with Smart App Control can't install
   the app.
4. Run the evals and tune the skills (section 8); add the skill picker to the app
   (section 9).
5. Milestone 2 (section 10), then 3 (OpenCascade), 4 (website), 5 (grow).

## 0. Decisions only you can make

- [ ] **You:** Will you publish as an individual or as a registered company, and in
  which country? This decides which code-signing options are open to you
  (section 5).
- [ ] **You:** Code-signing provider and budget (section 5).
- [ ] **You:** An Anthropic API key, with a spending limit, for the evals
  (section 8).
- [ ] **You:** When the repository becomes public (it is private today). Open core
  means the code will be public eventually, and update downloads from GitHub
  Releases need a public repository.
- [ ] **You:** Where users download the app and its updates: GitHub Releases now,
  or the website later (section 6).
- [ ] **You:** Final product name and logo. "ParametricDesign" is a working name and
  the icon is a placeholder cube. Decide before the website and its domain.
- [ ] **You:** Domain name for the website (milestone 4).
- [ ] **You:** Which service computes the text embeddings behind natural-language
  search on the website (milestone 4).
- [ ] **You:** Terms of use, privacy policy and a takedown process, before the
  website is public (not discussed yet; see section 12).

## 1. Finish installing 0.2.0 on your PC

Run these in `apps\desktop`, where your terminal already is.

- [ ] Close the ParametricDesign app and Claude Code, then stop any running MCP
  server: `Stop-Process -Name pd -Force -ErrorAction SilentlyContinue`
- [ ] Get the latest code: `git pull`
- [ ] Rebuild `pd.exe`, which has a new MCP tool: `cd ..\..`, then
  `cargo build --release -p pd-cli`, then `cd apps\desktop`.
- [ ] Rebuild the browser engine the interface is built against: `npm run wasm`
  (without it the installer build stops with type errors).
- [ ] Remove the stray empty folder that an earlier command created:
  `Remove-Item -Recurse -Force .\apps`
- [ ] Create the folder for the bundled `pd.exe`:
  `New-Item -ItemType Directory -Force src-tauri\binaries`
- [ ] Copy the new `pd.exe` in:
  `Copy-Item ..\..\target\release\pd.exe src-tauri\binaries\pd-x86_64-pc-windows-msvc.exe`
- [ ] Build the installers: `npx tauri build --config src-tauri/tauri.installer.conf.json`
  (if it stops with error 4551, run it again).
- [ ] Install with the **MSI**, not the setup `.exe`:
  `.\src-tauri\target\release\bundle\msi\ParametricDesign_0.2.0_x64_en-US.msi`.
  0.1.0 was installed from the MSI into `C:\Program Files\ParametricDesign`, and the
  MSI upgrades it in place. The setup `.exe` installs a second copy for your user
  only, under `%LOCALAPPDATA%`, and Claude would keep using the old `pd.exe`.
- [ ] Check that Settings → Apps lists one ParametricDesign, version 0.2.0.
- [ ] In Claude Code, run `/mcp` and reconnect `parametricdesign`: it should show
  9 tools (`get_open_design` and `get_design_request` are the new ones).
- [ ] First start: choose the library folder (`Documents\ParametricDesign Library`,
  or a OneDrive folder), your printer, and keep "Add 5 example designs".
- [ ] Bring in the designs Claude sent to 0.1.0: **New design → Import…**, from
  `Documents\ParametricDesign\inbox` and `inbox\opened`.
- [ ] After importing, delete the old `inbox` and `exports` folders from the code
  folder (git already ignores them).
- [ ] Only if you also use Claude Desktop: point it at the installed `pd.exe`
  (Settings → Claude in the app shows the config).

## 2. Test 0.2.0 on Windows

Send screenshots of anything wrong, slow or confusing.

**First start and settings**
- [ ] "Choose folder…" opens the Windows folder picker, Start creates the library, and
  the example designs appear with thumbnails.
- [ ] Theme "Follow Windows" changes when you switch Windows between light and
  dark; Light and Dark override it.
- [ ] Accent colours, Large text and inches all apply straight away.
- [ ] Printer: the bed size and nozzle of your printer (or a custom one) are used by
  the "fits the bed" check and for the plate in the viewer.
- [ ] Settings → Claude shows the right `pd.exe` path, and the copy buttons work.

**Library**
- [ ] Search (Ctrl+F) by name, tag and category; every sort option.
- [ ] Favorites, Recent, New from Claude.
- [ ] Trash: move to trash, restore, delete forever, empty trash.
- [ ] Collections: create, rename, delete, and put one design in several.
- [ ] Card menu (⋯): rename, duplicate, tags and category, collections.
- [ ] Import a `.pd.json` and a `.3mf` (including one exported by the app); paste
  JSON.

**Editor**
- [ ] Sliders and number boxes, Simple / Advanced, the unsaved-changes dot.
- [ ] Ctrl+S saves the next version with an automatic description; Versions: open
  an older one, copy one as a new design.
- [ ] Undo and redo (Ctrl+Z, Ctrl+Y).
- [ ] Presets: save, switch, rename, overwrite, delete.
- [ ] Export 3MF, STL and STL per part (Ctrl+E); file names include the preset
  name.
- [ ] Open an exported 3MF in Bambu Studio, OrcaSlicer or PrusaSlicer: parts,
  colours and plate placement are right.
- [ ] Leaving the design or closing the window with unsaved changes asks
  Save / Don't save / Cancel.
- [ ] "Check every slider value" and "Copy fix-it message for Claude".

**Viewer**
- [ ] Mouse: orbit, pan (middle or right button), zoom towards the cursor.
- [ ] View cube faces, Home and Fit; keys F, H or 0, 1, 2, 3, P, D, M, X, E and Esc.
- [ ] Display modes, Ortho, Plate, Exploded, Section (X / Y / Z and the slider),
  Measure (snapping to corners).
- [ ] Dragging sliders stays smooth on a big design.

**Claude**
- [ ] Ask for a new design: a notification appears, and the design shows under
  "New from Claude" with a thumbnail.
- [ ] Open a design and ask for a change ("make it 20 mm taller"): Claude reads it
  with `get_open_design`, and the compare screen opens by itself.
- [ ] Compare: side by side and overlay, parameter and part changes listed.
  - [ ] Accept saves a new version by Claude.
  - [ ] "Save as a new design" creates a separate design.
  - [ ] Reject keeps it in Versions, marked "not accepted".
  - [ ] Closing it (decide later) leaves a banner in the editor.
- [ ] A design Claude sends while the app is closed appears at the next start.
- [ ] Claude's checks use your printer's bed size.

**Design with AI**
- [ ] New design → Design with AI: describe something, add photos (choose, drop or
  paste with Ctrl+V), add a caption to each.
- [ ] Ticking a connector also ticks Multi-part models; picking another connector
  replaces the first; unticking it removes Multi-part models again.
- [ ] Send to Claude: the message is copied; paste it into Claude Code. Claude calls
  `get_design_request`, sees your photos, and the design arrives in the library.
- [ ] Export tailored skill: upload the `.zip` in Claude.ai (Settings → Capabilities
  → Skills), start a chat with your photos and the starter message.
- [ ] Copy prompt and Save as file: try them in ChatGPT or Gemini, then add the
  answer with New design → Paste JSON.
- [ ] The download button on a skill saves that skill on its own.

**OneDrive** (only if your library is there)
- [ ] The same library on two PCs: changes made on one show up on the other.
- [ ] Changing the same design on both at once keeps both as versions and loses
  nothing.

## 3. Fixes and polish found so far

- [ ] Double-clicking a `.pd.json` file: the installer registers the file type, but
  the app ignores the file it is given and just shows the library. It should open
  that design, including when the app is already running. Also check that Windows
  copes with the double extension `.pd.json`.
- [ ] Settings → Claude: show whether Claude is connected (the mockup had
  "Connected · Claude Code"), and add one-click "Connect Claude Desktop" (writes its
  config file) and "Connect Claude Code" (runs `claude mcp add`). The Milestone 1
  plan promised this; today you copy commands by hand.
- [ ] The view doesn't re-fit after a big size change (you have to press F).
  Re-fit automatically when the model's size changes a lot.
- [ ] Measurement points stay on screen after accepting Claude's update; clear them.
- [ ] Compare screen: the "Sync cameras" switch and the "your presets still work"
  note from the mockup.
- [ ] Offer a one-time "Import designs from the previous version" for the old
  `Documents\ParametricDesign\inbox`, so nobody has to import by hand.
- [ ] One command to build a local installer (for example `npm run installer`) that
  builds `pd.exe`, copies it in and builds the installer.
- [ ] The interface code is one 870 kB file: split out three.js so the app starts
  faster.
- [ ] Clean up the remaining style warnings from Rust's linter, Clippy (in
  `pd-core` and `pd-render`).
- [ ] Fix the MSI "upgrade code" in `tauri.conf.json` before renaming the app. By
  default it is derived from the product name, so after a rename Windows would
  install a second copy instead of upgrading.

## 4. Developing on Windows with Smart App Control on

Decided: Smart App Control stays on.

- [ ] Confirm that `npx tauri dev --release` opens the app window. The terminal
  waiting at "Running target\release\parametricdesign.exe" is normal; it returns
  when you close the app.
- [ ] If Smart App Control blocks the build again: set up WSL, the Linux
  environment built into Windows. Linux programs aren't checked by Smart App
  Control, and the app runs in a normal window. Claude writes the steps: WSL,
  Rust, Node, the Linux build libraries, and VS Code's WSL extension.
- [ ] Describe this workaround in the README for future contributors.

## 5. Code signing

Needed so Windows (Smart App Control and SmartScreen) lets people install and run
the app. It must cover `parametricdesign.exe`, `pd.exe`, the MSI and the setup
`.exe`.

- [ ] **You:** Choose a provider (prices to confirm before buying):
  - Microsoft Azure Artifact Signing, about $10/month: individuals in the USA or
    Canada only; registered companies in the USA, Canada, EU or UK. Easiest to use
    from CI.
  - Certum Open Source Code Signing, from about €49/year: individuals anywhere, for
    open-source projects. Its phone-app sign-in is awkward to automate in CI.
  - A standard OV certificate (Certum, SSL.com, Sectigo…), about $150–300/year.
- [ ] **You:** Complete the identity check (a few business days).
- [ ] **You:** Add the signing credentials to GitHub as Actions secrets.
- [ ] CI signs all four files, with a timestamp so signatures stay valid after the
  certificate expires.
- [ ] CI refuses to publish anything unsigned (it checks every signature).
- [ ] Publisher name, description and copyright filled in for the installer and
  the file properties.
- [ ] Test on a clean Windows 11 PC or virtual machine with Smart App Control on:
  download, install, run, connect Claude.
- [ ] If Windows still warns: submit the files to Microsoft for analysis. New
  publishers can see SmartScreen warnings for the first weeks while downloads
  build reputation.
- [ ] Later: the Microsoft Store as an extra channel.

## 6. Releases and automatic updates

- [ ] **You:** Where downloads live (section 0).
- [ ] Offer one installer type to users, MSI (for all users) or setup `.exe` (per
  user). Offering both leads to duplicate installs like the one avoided in
  section 1.
- [ ] A version-numbering rule and a changelog, plus one command that bumps the
  version in all three places (`package.json`, `tauri.conf.json`,
  `src-tauri/Cargo.toml`).
- [ ] Release workflow: pushing a version tag makes CI build, sign and publish a
  release with notes.
- [ ] Updater key: Claude generates it; **You:** store the private key and its
  password as GitHub secrets; the public key goes into the app.
- [ ] Add Tauri's updater: check at startup and from Settings, show "Update
  available", then download, install and restart. Each release carries the file
  the updater reads (`latest.json`).
- [ ] `pd.exe` is updated together with the app; tell users to reconnect Claude
  after an update, or detect it.
- [ ] Test a real update from one version to the next.
- [ ] Once signed releases exist, install new versions from them instead of
  building on your PC.

## 7. Repository, CI and docs

- [ ] Create a `main` branch and make it the default. Merge the current work into it
  through a pull request, then do future work in pull requests.
- [ ] Add the licence files: MIT and Apache-2.0 for the code, and a CC-BY-4.0 note
  for the example designs. The README names these licences, but no licence files
  exist.
- [ ] When the repository goes public: contributing guide, security policy and
  issue templates.
- [ ] CI: add Clippy to the Linux job.
- [ ] Add the end-to-end test that clicks through the app in a browser to the
  repository, and run it in CI. Today it exists only in a temporary cloud folder
  and will be lost.
- [ ] README: Windows development with Smart App Control, the one-command installer,
  where the app keeps its data.
- [ ] Keep SPEC.md and this checklist up to date.

## 8. LLM → design quality (left over from Milestone 1)

The product depends on LLMs getting designs right with little back-and-forth.

- [ ] **You:** Provide the API key (section 0).
- [ ] Run a few eval cases first, then all 20, both one-shot and with the MCP tools.
- [ ] Measure first-try and after-self-fix success. Targets: at least 80 % first try
  and 95 % after self-fix.
- [ ] Tune the skills until the targets are met. Every later skill change re-runs
  the evals.
- [ ] Add real photos to the photo cases. Today one case describes a photo in words
  and none includes an image.
- [ ] Add cases for changing the open design ("make the open design…").
- [ ] Try the skills with other LLMs (ChatGPT, Gemini) through the tailored prompt
  and paste, since users can bring any LLM.
- [ ] Validation: also build random combinations of slider values. Today each
  slider is checked at its own minimum and maximum, one at a time.

## 9. Skills

- [ ] New optional skills from the spec: hinge, threads, text and labels,
  multicolour, Gridfinity.
- [ ] Community-contributed skills (milestone 5).

## 10. Milestone 2: the rest of the desktop app

**Viewer and measuring**
- [ ] Dimension labels on the model, linked to the sliders: click one to jump to its
  slider, drag it to change the value.
- [ ] Measure angles, radii and wall thickness, and pick edges and faces. Today it
  measures point to point, snapping to corners.
- [ ] A print-layout view: parts turned and arranged the way they will print.

**Simple and Advanced modes**
- [ ] Simple mode: one big "Export for my printer" button.
- [ ] Advanced mode: a fit profile per printer (tight / normal / loose), mesh
  resolution, export options and a JSON editor.

**Printers and 3MF**
- [ ] Printer profiles include AMS / MMU filament slots.
- [ ] 3MF: part colours mapped to filament slots.
- [ ] 3MF: real separate build plates. Today extra plates are placed to the right in
  one scene.
- [ ] 3MF: print settings (layer height, infill, walls, supports) written in the
  project format of Bambu Studio, OrcaSlicer and PrusaSlicer, and tested in each.

**Other exports**
- [ ] OBJ and GLB export, with colours.

**Design format**
- [ ] Text component (embossed or engraved), and a text parameter type.
- [ ] Hinge and thread components.
- [ ] Fillets and chamfers: approximated with a warning on the current engine, exact
  with OpenCascade (milestone 3).
- [ ] More components and example designs.
- [ ] Keep all interface text in one place so the app can be translated later
  (English first).

## 11. Milestone 3: OpenCascade

- [ ] Trial run first: OpenCascade from Rust on Windows. Its Rust bindings are young
  (risk 2 in the spec); check app size and build time.
- [ ] The second engine implements every component.
- [ ] True fillets and chamfers.
- [ ] STEP export (desktop app only).
- [ ] Engine switch per design or export (Advanced mode), with warnings wherever the
  current engine only approximates.
- [ ] Tests: both engines build every example with matching sizes.

## 12. Milestone 4: the catalog website

**Accounts and setup**
- [ ] **You:** Supabase project (free tier first, about $25/month later), domain
  name, static hosting.

**Building blocks**
- [ ] Move the viewer and parameter panel into a shared package used by both the app
  and the site.
- [ ] Static site (Vite + React) running the same engine in the visitor's browser.
  The browser engine already exists (2.9 MB).
- [ ] Database: designs, versions, categories, tags, skills used, printer-fit info,
  authors and licences.
- [ ] File storage: design JSON, thumbnails and turntable animations.
- [ ] Sign-in with email, Google or GitHub.

**For visitors**
- [ ] Browse by categories and tags.
- [ ] Natural-language search ("something to hold my headphones under the desk"),
  using embeddings computed when a design is published.
- [ ] Filters: connector and skills used, multicolour, needs supports, fits my
  printer.
- [ ] Design page: 3D preview, Simple-mode sliders and STL / 3MF download, all in
  the browser with no server-side rendering. STEP stays desktop-only.
- [ ] "Open in the desktop app" and download links.

**Publishing from the app**
- [ ] A Publish button: the app validates the design, makes thumbnails and a
  turntable, then uploads.
- [ ] The server checks the design again with the same engine before listing it
  (it never trusts the upload).
- [ ] The author picks a licence from a Creative Commons list (default CC-BY-4.0).
- [ ] Local versions map to online versions.

**Before the site goes public** (not discussed yet)
- [ ] Terms of use, a privacy policy (GDPR applies to EU visitors), and a way to
  report and take down stolen designs.

## 13. Milestone 5: grow

- [ ] macOS builds (Apple Developer account, about $99/year, for signing and
  notarisation) and Linux builds (AppImage, .deb, Flatpak).
- [ ] Paid extras in a separate private service: cloud sync, private designs, hosted
  AI generation.
- [ ] Community skills in the online catalog.
- [ ] Phone and tablet viewer/customizer (Tauri for iOS and Android), later.

The signed Windows release pipeline was planned here but moved up to sections 5
and 6.

## 14. Ideas that came up but weren't chosen

Not planned; kept here so nothing is lost.

- **Printing:** a "Send to slicer" button (opens the 3MF in Bambu Studio, OrcaSlicer
  or PrusaSlicer); a 3MF per preset, named after it.
- **Measuring:** design around a real object (import an STL or 3D scan with clearance
  checks); compare with reference sizes (credit card, AA battery, ruler).
- **Website:** search by size ("box for 120 × 80 × 40 mm"); a remix tree; makes,
  ratings and comments; reports and moderation; featured creators and staff picks.
- **AI:** a chat panel inside the app using your own API key; local models (Ollama,
  LM Studio); ready-made assistants (Custom GPT, Claude Project, Gemini Gem); a
  "custom code" escape hatch in the design format.
- **Other:** SVG / DXF export for laser cutting and CNC; a plugin for OrcaSlicer or
  FreeCAD; templates for teachers and students; a first-start tour; tabs to open
  several designs at once; a high-contrast theme; "fits my printer" and
  "connector used" filters in the local library; sharing a design as a file;
  Raspberry Pi next to the printer.

## 15. Done so far

**Planning**
- [x] Kickoff Q&A (5 rounds) and the spec with every decision ([SPEC.md](SPEC.md)).

**Milestone 1: LLM → design loop**
- [x] The `pd/0.1` JSON design format: parameters (number, whole number, on/off,
  choice, colour), formulas, derived values, placement by named anchors, added and
  cut features, repeats (row, grid, circle) and mirroring.
- [x] 15 components: box, cylinder (also cones), sphere, tube, prism (any outline),
  revolve, torus, wedge, slot, hollow box, lid with snap fit, screw hole
  (countersunk, heat-set insert), nut trap, magnet pocket, standoff.
- [x] Splitting big parts into pieces with dovetail, pin or magnet joints.
- [x] Checks at every slider's default, minimum and maximum; printability checks;
  errors that name the exact spot in the JSON and suggest a fix.
- [x] The geometry engine in pure Rust, the same in the app, the MCP server and the
  browser (WebAssembly).
- [x] STL and 3MF export: colours, plate layout, thumbnail, and the design embedded
  so the file reopens editable.
- [x] Preview images rendered without a screen, so LLMs can see their design.
- [x] The `pd` command line.
- [x] MCP server with 9 tools: `validate_design`, `render_preview`, `measure`,
  `get_open_design`, `get_design_request`, `open_in_app`, `export_design`,
  `list_components`, `get_design_guide`.
- [x] 10 skills: 3 core (core-parametric, printability, from-photos) and 7 optional
  (multi-part, snap-fit, dovetail, pin, magnet and screw connectors,
  electronics enclosure). They bundle into one tailored skill or one prompt from
  the command line.
- [x] 5 example designs, checked by the tests.
- [x] Eval suite: 20 real requests and the scoring script (not run yet; section 8).
- [x] CI: Linux tests and the WebAssembly build; a Windows installer with `pd.exe`
  inside.

**Set up on your PC**
- [x] Build tools, Rust, Node, Git and VS Code; the project cloned and built.
- [x] PowerShell script permission; Claude Code installed and added to PATH.
- [x] 0.1.0 installed; Claude Code connected to it over MCP (7 tools).

**Library and editor (0.2.0)**
- [x] Questions round and a clickable mockup.
- [x] Library home: folder you choose (OneDrive-safe: every save is a separate
  version file, so sync conflicts become extra versions), search, favorites, recent,
  "New from Claude", collections, categories, tags, sorting, trash, automatic
  thumbnails, import of `.pd.json` and `.3mf`, paste, example designs.
- [x] Editor: Save with automatic descriptions, version history (open or copy),
  undo and redo, named presets, export with preset names, Save / Don't save /
  Cancel when leaving.
- [x] Viewer: view cube and standard views, ortho / perspective, display modes
  (shaded, shaded with edges, wireframe, X-ray), build plate, exploded view, section
  with filled cut faces, measuring with corner snapping, keyboard shortcuts.
- [x] Claude: reads the open design; updates open a side-by-side or overlay compare;
  accept, save as new or reject; new designs land in the library.
- [x] Settings and first start: library folder, printer profiles (bed and nozzle
  used by the checks, the plate and the 3MF), theme following Windows, accent
  colour, text size, mm or inches, Claude setup commands.
- [x] Windows build fixes: the interface dev server no longer watches the Rust build
  folder; `npx tauri dev --release` works around Smart App Control; version 0.2.0.

**Design with AI (skill picker)**
- [x] New design → Design with AI: name, description, photos (choose, drop or
  paste; scaled down to 1568 px; captions; copy a photo to paste it into a chat)
  and your printer.
- [x] Skills: core skills always included; optional ones ticked, with their
  requirements added and removed automatically and one connector at a time;
  every skill can be exported on its own (`.zip`).
- [x] Three ways out: Send to Claude (the MCP tool `get_design_request` gives
  Claude the brief, the photos as images, the printer and the skills to load);
  one tailored skill as a `.zip` with a starter message; one prompt to copy or
  save, with a starter message for the file.
- [x] The skills are embedded without a build script, so the app gained nothing
  for Smart App Control to block. The command line exports `.zip` files too.
- [x] CI: the Windows installer is built only on demand; the Linux job also
  type-checks and builds the interface, and a newer push cancels an older run.
