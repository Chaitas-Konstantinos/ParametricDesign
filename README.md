# ParametricDesign

**Describe what you need. An AI designs it as a parametric 3D model. You see it, adjust it, and keep talking to the AI until it's right.**

ParametricDesign connects a large language model (LLM) to a 3D app. The LLM does the designing, guided by a catalog of expert **skills**. The app shows the result as a **parametric design**: a 3D model whose important sizes are settings you can change. You don't need to know CAD.

This README describes the core concept only. Everything else comes later.

---

## The main workflow

```
   ┌──────────────┐     ┌──────────────────┐     ┌──────────────┐     ┌────────────────────┐
   │ 1. Describe  │ ──► │ 2. Pick skills   │ ──► │ 3. LLM       │ ──► │ 4. See the design  │
   │ what you need│     │ from the catalog │     │ designs it   │     │ in 3D, with all    │
   └──────────────┘     └──────────────────┘     └──────────────┘     │ its options        │
                                                        ▲             └─────────┬──────────┘
                                                        │                       │
                                                        └──── 5. Talk back ─────┘
                                                          "make it taller", "use magnets
                                                           instead", "split it in two"
```

1. **Describe** the object in your own words, with photos if you have them.
2. **Pick skills** from the catalog: for example *Dovetail connector* for a tray that has to be printed in pieces.
3. **The LLM designs it**, following the instructions in the skills you picked.
4. **See the design** in the app's 3D viewer, with every parameter and option it has.
5. **Talk back** to the LLM to change it, and repeat until it's what you want.

---

## 1. Talking to the LLM

The user talks to an LLM in natural language. Where that conversation happens is **still open**:

| Option | How it works | Good | Less good |
|---|---|---|---|
| **Inside the app** | A chat or AI panel in the app | Everything in one place; the app sees the whole conversation | The app has to connect to an AI provider |
| **Outside the app** | The user's own AI app (Claude, ChatGPT, Gemini…) connected to the app through an **MCP server** | Works with the AI the user already has | Two windows; depends on the AI app supporting MCP |

Whichever we choose, the LLM must be able to:
- **read** the skills the user picked;
- **send** a design to the app;
- **see** the design that's open in the app, including the user's current settings, so it can change it;
- **check** its own work (is the design valid, does it look right) before handing it over.

---

## 2. The skill catalog

The catalog is the heart of the product. A **skill** is a well-written, detailed instruction pack that teaches the LLM how to design one thing properly. The LLM is only as good as its skills, so every skill is written and tested carefully.

### Categories

Skills are grouped into sections. For example:

| Category | Example skills |
|---|---|
| **Connectors** | Dovetail, snap-fit, magnets, alignment pins, screws and heat-set inserts |
| **Multi-part** | Splitting objects too big for the printer, assemblies, clearances between parts |
| **Enclosures** | Boxes and lids, electronics cases |
| **Printability** | Wall thickness, tolerances, overhangs, print orientation |
| **From photos** | Estimating sizes from photos and reference objects |

A few **core skills** (the design language and the basics of 3D printing) are always included. The user picks the rest.

### Skills are just folders

The catalog is a normal folder on your computer (`Documents\ParametricDesign Skills`): one folder per skill. **To add a skill, add a folder. To improve one, edit its text.** The app and the LLM pick up the change straight away, with nothing to compile or reinstall. A skill with a mistake is left out and the app says what to fix. App updates never overwrite skills you changed. How to write a skill: [skills/README.md](skills/README.md).

### What a skill contains

Each skill has two sides: one for the user and one for the LLM.

**For the user (in the catalog)**
- **Name**
- **Thumbnail**: a picture of what it makes
- **Description**: what it does and when to use it, in plain words

**For the LLM**
- **Full instructions**: how to build the feature step by step, which sizes matter, sensible defaults, tolerances, which parameters to expose to the user, when to use it and when not to, and common mistakes.
- **Reference images**: labelled drawings that show the feature and its dimensions, using the same names as the parameters. They help the LLM build the same thing the same way every time.
- **Examples**: finished designs that use the skill correctly.

For example, the **Dovetail connector** skill explains the dovetail's shape, angle, length and clearance in full detail. It also says where to place the dovetails on a cut, which values the user can adjust, and shows a drawing of a dovetail joint with every dimension named. Any LLM that reads it can make a dovetail that fits on the first try.

---

## 3. The parametric design

The LLM answers with a **parametric design**, not a fixed 3D file. It contains:
- **the parts** the object is made of;
- **the parameters**: the sizes and choices that matter (width, height, wall thickness, number of compartments, connector type), each with a sensible range;
- **how the parameters shape the parts**, so changing one setting updates the whole model correctly.

The app shows **the full design and all its options**: every parameter, grouped and clearly named, next to the 3D model. Moving a setting updates the model straight away.

---

## 4. Changing it by talking

The user keeps talking to the LLM about the design that's open: *"make it 20 mm taller"*, *"the lid is too loose"*, *"use magnets instead of the dovetail"*. The LLM reads the current design, including any settings the user changed, and sends back an updated version. The app shows **what changed** before the user keeps it.

Small changes the user makes with the sliders. Bigger changes go through the LLM. Both work on the same design.

---

## 5. The 3D viewer

The viewer keeps the views the app has now, and makes them polished and reliable:

- **Rotate, pan and zoom** smoothly.
- **Standard views** (front, side, top, isometric) with a view cube, in perspective or orthographic.
- **Display styles**: shaded, shaded with edges, wireframe, see-through.
- **Section view**: cut through the model to see inside.
- **Exploded view**: parts move apart along sensible directions so you can see how they fit together, and move back to the assembly smoothly. Today this view needs the most polish.
- **Print plate view** (below).

---

## 6. The print plate

Each part is shown laid out on the printer's bed, ready to print:

- Every part can be placed in **several orientations**: lying on any of its flat faces, standing up, or rotated on the plate. The best orientation (strongest, fewest supports) is suggested first.
- The user can **switch orientations** part by part and see the result straight away.
- Parts are **arranged automatically** so they fit on the bed without touching, and the plate shows when something doesn't fit.

---

## License

Code: MIT OR Apache-2.0. Designs: CC-BY-4.0 unless marked otherwise.
