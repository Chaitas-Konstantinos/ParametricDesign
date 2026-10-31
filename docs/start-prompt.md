We're starting a new project from scratch in this empty folder: **ParametricDesign**. Don't write any code yet. First help me brainstorm and decide the architecture. Ask me questions where a decision is mine, give a recommendation with each question, and keep the scope to the core concept below. Everything else comes later.

# The idea

Describe what you need. An AI designs it as a parametric 3D model. You see it, adjust it, and keep talking to the AI until it's right.

ParametricDesign connects a large language model (LLM) to a 3D desktop app. The LLM does the designing, guided by a catalog of expert **skills**. The app shows the result as a **parametric design**: a 3D model whose important sizes are settings the user can change. Users don't need to know CAD. The focus is 3D printing.

# The main workflow

1. **Describe** the object in your own words, with photos if you have them.
2. **Pick skills** from the catalog, for example *Dovetail connector* for a tray that has to be printed in pieces.
3. **The LLM designs it**, following the instructions in the chosen skills.
4. **See the design** in the app's 3D viewer, with every parameter and option it has.
5. **Talk back** to the LLM to change it ("make it taller", "use magnets instead", "split it in two"). Repeat until it's right.

# 1. Talking to the LLM (still open: help me decide)

| Option | How it works | Good | Less good |
|---|---|---|---|
| Inside the app | A chat or AI panel in the app | Everything in one place; the app sees the whole conversation | The app has to connect to an AI provider |
| Outside the app | The user's own AI app (Claude, ChatGPT, Gemini…) connected to the app through an MCP server | Works with the AI the user already has | Two windows; depends on the AI app supporting MCP |

Whichever we choose, the LLM must be able to:
- read the skills the user picked;
- send a design to the app;
- see the design open in the app, including the user's current settings, so it can change it;
- check its own work (is the design valid, does it look right) before handing it over.

# 2. The skill catalog (the heart of the product)

A skill is a well-written, detailed instruction pack that teaches the LLM how to design one thing properly. The LLM is only as good as its skills, so every skill must be written and tested carefully.

**Categories.** The catalog is split into sections. The first section is always included; the user picks from the others:

| Category | Skills | Picked by |
|---|---|---|
| Core | The design language; designing for 3D printing (walls, tolerances, overhangs, orientation); sizes from photos and descriptions | Always included |
| Multi-part | Splitting objects too big for the printer into pieces, assemblies, clearances between parts | The user |
| Connectors | Dovetail, alignment pins, magnets (choose one of these three to join split pieces); snap-fit lids; screws, nuts and heat-set inserts | The user |
| Enclosures | Electronics cases: standoffs, port cut-outs, vents, screwed lids | The user |

Some skills bring others with them: every connector for split pieces adds Multi-part, and Enclosures adds screws. A new category appears as soon as a skill uses it.

**Skills are just folders.** The catalog is a normal folder on the user's computer, with one folder per skill. To add a skill, add a folder. To improve one, edit its text. The app and the LLM pick up the change straight away, with nothing to compile or reinstall. A skill with a mistake is left out and the app says what to fix. App updates never overwrite skills the user changed.

**What a skill contains.** Each skill has two sides:
- **For the user (in the catalog):** name, thumbnail (a picture of what it makes), and a plain-words description of what it does and when to use it.
- **For the LLM:**
  - **full instructions:** how to build the feature step by step, which sizes matter, sensible defaults, tolerances, which parameters to expose, when to use it and when not to, and common mistakes;
  - **reference images:** labelled drawings that use the same names as the parameters, so the LLM builds the same thing the same way every time;
  - **examples:** finished designs that use the skill correctly.

Example: the Dovetail connector skill explains the dovetail's shape, angle, length and clearance in full detail. It says where to place the dovetails on a cut, lists which values the user can adjust, and includes a drawing of the joint with every dimension named. Any LLM that reads it should make a dovetail that fits on the first try.

# 3. The parametric design

The LLM answers with a parametric design, not a fixed 3D file. It contains:
- **the parts** the object is made of;
- **the parameters:** sizes and choices that matter (width, height, wall thickness, number of compartments, connector type), each with a sensible range;
- **how the parameters shape the parts**, so changing one setting updates the whole model correctly.

The app shows the full design and all its options: every parameter, grouped and clearly named, next to the 3D model. Moving a setting updates the model straight away.

# 4. Changing it by talking

The user keeps talking to the LLM about the open design: "make it 20 mm taller", "the lid is too loose", "use magnets instead of the dovetail". The LLM reads the current design, including any settings the user changed, and sends back an updated version. The app shows what changed before the user keeps it. Sliders handle small changes, the LLM handles bigger ones, and both work on the same design.

# 5. The 3D viewer

It must be polished and reliable:
- rotate, pan and zoom smoothly;
- standard views (front, side, top, isometric) with a view cube, in perspective or orthographic;
- display styles: shaded, shaded with edges, wireframe, see-through;
- section view: cut through the model to see inside;
- exploded view: parts move apart along sensible directions to show how they fit together, and move back to the assembly smoothly. This must look good, not just work;
- print plate view (below).

# 6. The print plate

Each part is shown laid out on the printer's bed, ready to print:
- every part can be placed in several orientations: lying on any of its flat faces, standing up, or rotated on the plate. The best orientation (strongest, fewest supports) is suggested first;
- the user can switch orientations part by part and see the result straight away;
- parts are arranged automatically so they fit on the bed without touching, and the plate shows when something doesn't fit.

# Constraints and lessons from an earlier prototype

- **Desktop app, Windows first;** keep the code cross-platform. Everything runs locally on the user's PC.
- **Smart App Control.** On my PC, Windows Smart App Control is on and must stay on. It blocks unsigned programs compiled locally, including Rust build scripts (os error 4551). Prefer tools and dependencies that don't compile and run helper programs during the build, and say when a choice would hit this.
- **Skills stay editable.** Skills must stay in an editable folder (see above), never compiled into the app.
- **What the earlier prototype taught us:**
  - an LLM writes a JSON design much more reliably when it uses high-level components placed by named anchors (e.g. "on top of the box") instead of raw coordinates;
  - the design must be validated at the minimum and maximum of every parameter;
  - errors must say exactly how to fix them.
- **License:** code MIT OR Apache-2.0; designs CC-BY-4.0 unless marked otherwise.

# What I want from this brainstorm

1. Help me decide where the conversation with the LLM happens (inside the app or through an MCP server), with your recommendation.
2. Propose the overall architecture and the technology stack, with the trade-offs.
3. Propose the format of a parametric design and the format of a skill folder.
4. Split the work into small milestones. The first one should prove the main loop: describe → LLM designs with skills → see it in 3D with its parameters → talk back to change it.

Then write the concept brief.
