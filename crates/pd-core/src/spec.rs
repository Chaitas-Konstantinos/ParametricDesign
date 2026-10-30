//! Declarative specs for every component and joint type.
//!
//! The same specs drive parsing (field checks, "did you mean"), the
//! `list_components` MCP tool and the generated component reference that ships
//! with the skills, so docs and behaviour cannot drift apart.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldKind {
    /// Number or expression.
    Num,
    /// Whole number or expression (rounded).
    Int,
    Bool,
    /// `[x, y]`.
    Vec2,
    /// `[x, y, z]`; a single number means the same value on all three axes.
    Vec3,
    /// `[[x, y], ...]`.
    Points2,
    /// `[a, b, ...]`.
    NumList,
    Enum(&'static [&'static str]),
    /// Id of another part.
    PartRef,
}

impl FieldKind {
    pub fn describe(&self) -> String {
        match self {
            FieldKind::Num => "number".into(),
            FieldKind::Int => "integer".into(),
            FieldKind::Bool => "boolean".into(),
            FieldKind::Vec2 => "[x, y]".into(),
            FieldKind::Vec3 => "[x, y, z]".into(),
            FieldKind::Points2 => "[[x, y], ...]".into(),
            FieldKind::NumList => "[n, ...]".into(),
            FieldKind::Enum(opts) => opts
                .iter()
                .map(|o| format!("\"{o}\""))
                .collect::<Vec<_>>()
                .join(" | "),
            FieldKind::PartRef => "part id".into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldSpec {
    pub name: &'static str,
    pub kind: FieldKind,
    pub required: bool,
    pub default: Option<&'static str>,
    pub doc: &'static str,
}

const fn req(name: &'static str, kind: FieldKind, doc: &'static str) -> FieldSpec {
    FieldSpec {
        name,
        kind,
        required: true,
        default: None,
        doc,
    }
}
const fn opt(
    name: &'static str,
    kind: FieldKind,
    default: &'static str,
    doc: &'static str,
) -> FieldSpec {
    FieldSpec {
        name,
        kind,
        required: false,
        default: Some(default),
        doc,
    }
}
const fn alt(name: &'static str, kind: FieldKind, doc: &'static str) -> FieldSpec {
    FieldSpec {
        name,
        kind,
        required: false,
        default: None,
        doc,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ComponentSpec {
    pub name: &'static str,
    pub category: &'static str,
    pub summary: &'static str,
    pub fields: &'static [FieldSpec],
    /// Groups where exactly one field must be given.
    pub exactly_one: &'static [&'static [&'static str]],
    /// Groups where at most one field may be given.
    pub at_most_one: &'static [&'static [&'static str]],
    /// Extra named anchors beyond the generic box anchors.
    pub anchors: &'static [(&'static str, &'static str)],
    pub notes: &'static str,
    pub example: &'static str,
}

pub const SCREW_SIZES: &[&str] = &["M2", "M2.5", "M3", "M4", "M5", "M6", "M8"];
const SCREW_FIT: &[&str] = &["clearance", "tap", "insert"];
const STANDOFF_FIT: &[&str] = &["tap", "insert"];
const SCREW_HEAD: &[&str] = &["none", "cap", "countersunk"];
const OPEN: &[&str] = &["top", "none"];
const LID_STYLE: &[&str] = &["lip", "flat", "snap"];

pub const COMPONENTS: &[ComponentSpec] = &[
    ComponentSpec {
        name: "box",
        category: "basic",
        summary: "Solid block, optionally with rounded vertical edges.",
        fields: &[
            req("size", FieldKind::Vec3, "Outer size [x, y, z] in mm."),
            opt("radius", FieldKind::Num, "0", "Rounding radius of the 4 vertical edges."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "box", "size": [60, 40, 10], "radius": 3 }"#,
    },
    ComponentSpec {
        name: "cylinder",
        category: "basic",
        summary: "Cylinder or cone standing on its base, axis along Z.",
        fields: &[
            alt("d", FieldKind::Num, "Diameter (give d or r)."),
            alt("r", FieldKind::Num, "Radius (give d or r)."),
            req("h", FieldKind::Num, "Height."),
            alt("d_top", FieldKind::Num, "Top diameter, for a cone or tapered cylinder."),
            alt("r_top", FieldKind::Num, "Top radius, for a cone or tapered cylinder."),
            alt("segments", FieldKind::Int, "Number of facets around (default: automatic from size)."),
        ],
        exactly_one: &[&["d", "r"]],
        at_most_one: &[&["d_top", "r_top"]],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "cylinder", "d": 10, "h": 20 }"#,
    },
    ComponentSpec {
        name: "sphere",
        category: "basic",
        summary: "Sphere resting on z = 0.",
        fields: &[
            alt("d", FieldKind::Num, "Diameter (give d or r)."),
            alt("r", FieldKind::Num, "Radius (give d or r)."),
        ],
        exactly_one: &[&["d", "r"]],
        at_most_one: &[],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "sphere", "d": 20 }"#,
    },
    ComponentSpec {
        name: "tube",
        category: "basic",
        summary: "Hollow cylinder (pipe, ring, spacer).",
        fields: &[
            req("d", FieldKind::Num, "Outer diameter."),
            alt("d_inner", FieldKind::Num, "Inner (hole) diameter (give d_inner or wall)."),
            alt("wall", FieldKind::Num, "Wall thickness (give d_inner or wall)."),
            req("h", FieldKind::Num, "Height."),
        ],
        exactly_one: &[&["d_inner", "wall"]],
        at_most_one: &[],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "tube", "d": 20, "wall": 2, "h": 15 }"#,
    },
    ComponentSpec {
        name: "prism",
        category: "basic",
        summary: "Any 2D outline extruded straight up.",
        fields: &[
            req("points", FieldKind::Points2, "Outline corners [[x, y], ...] in order (at least 3), local coordinates."),
            req("h", FieldKind::Num, "Extrusion height."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "Points are used as given (not re-centred). Keep the outline simple (not self-crossing).",
        example: r#"{ "type": "prism", "points": [[0, 0], [30, 0], [15, 20]], "h": 5 }"#,
    },
    ComponentSpec {
        name: "revolve",
        category: "basic",
        summary: "A 2D profile spun around the Z axis (vases, knobs, bowls, bottles).",
        fields: &[
            req("profile", FieldKind::Points2, "Profile corners [[radius, z], ...]; radius >= 0, closed automatically."),
            opt("angle", FieldKind::Num, "360", "Sweep angle in degrees."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "To make a hollow vessel, include the wall in the profile (outer edge up, inner edge down).",
        example: r#"{ "type": "revolve", "profile": [[0, 0], [30, 0], [35, 60], [33, 60], [28, 2], [0, 2]] }"#,
    },
    ComponentSpec {
        name: "torus",
        category: "basic",
        summary: "Ring / donut lying flat.",
        fields: &[
            req("d", FieldKind::Num, "Diameter of the ring's centre line."),
            req("thickness", FieldKind::Num, "Diameter of the ring's round cross-section."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "torus", "d": 40, "thickness": 6 }"#,
    },
    ComponentSpec {
        name: "wedge",
        category: "basic",
        summary: "Right-angle triangular prism (gusset, ramp, support rib).",
        fields: &[req("size", FieldKind::Vec3, "Bounding size [x, y, z]. The slope rises from the front (-Y) bottom edge to the back (+Y) top edge; full height is at the back.")],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "Rotate it (place.rotate [0, 0, 90] etc.) to point the slope another way.",
        example: r#"{ "type": "wedge", "size": [3, 20, 20] }"#,
    },
    ComponentSpec {
        name: "slot",
        category: "basic",
        summary: "Stadium / rounded slot (long hole with round ends), long axis along X.",
        fields: &[
            req("length", FieldKind::Num, "Overall length including the round ends."),
            req("width", FieldKind::Num, "Width (= end diameter)."),
            req("h", FieldKind::Num, "Height / depth."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "slot", "length": 20, "width": 6, "h": 3 }"#,
    },
    ComponentSpec {
        name: "hollow_box",
        category: "container",
        summary: "Box with walls and a floor, open at the top by default (trays, bins, enclosures).",
        fields: &[
            alt("inner", FieldKind::Vec3, "Usable inside size [x, y, z] (give inner OR outer). z is the inside depth above the floor."),
            alt("outer", FieldKind::Vec3, "Outside size [x, y, z] (give inner OR outer)."),
            opt("wall", FieldKind::Num, "2", "Side wall thickness."),
            alt("floor", FieldKind::Num, "Floor thickness (default: same as wall)."),
            opt("radius", FieldKind::Num, "0", "Outer rounding of the 4 vertical edges (inside follows)."),
            opt("open", FieldKind::Enum(OPEN), "top", "\"top\" = open tray/bin; \"none\" = closed hollow shell."),
        ],
        exactly_one: &[&["inner", "outer"]],
        at_most_one: &[],
        anchors: &[("inside_bottom", "Centre of the floor's top surface (points up, into the cavity).")],
        notes: "Use inner when the contents decide the size (\"fits a 100 x 60 mm phone\"). Pair with a `lid` part.",
        example: r#"{ "type": "hollow_box", "inner": [80, 50, 30], "wall": 2, "radius": 4 }"#,
    },
    ComponentSpec {
        name: "lid",
        category: "container",
        summary: "Lid that fits a hollow_box part; places itself on that box automatically.",
        fields: &[
            req("fits", FieldKind::PartRef, "Id of the part whose shape is a hollow_box."),
            opt("style", FieldKind::Enum(LID_STYLE), "lip", "\"lip\" = plate + inner lip that drops into the box; \"snap\" = lip with snap bumps (grooves are cut into the box automatically); \"flat\" = plain plate."),
            opt("clearance", FieldKind::Num, "0.2", "Gap per side between lip and box wall. 0.15-0.3 for most printers."),
            opt("thickness", FieldKind::Num, "2", "Plate thickness."),
            opt("lip_height", FieldKind::Num, "4", "How deep the lip reaches into the box."),
            opt("lip_wall", FieldKind::Num, "1.6", "Lip wall thickness."),
            opt("snap_size", FieldKind::Num, "0.6", "Snap bump radius (style \"snap\")."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[("seat", "Underside of the plate (points down). Auto-placed on <fits>.top.")],
        notes: "Leave out \"place\": the lid is put on its box. It is printed upside down (plate on the bed) automatically.",
        example: r#"{ "type": "lid", "fits": "box", "style": "snap", "clearance": "fit" }"#,
    },
    ComponentSpec {
        name: "screw_hole",
        category: "hardware",
        summary: "Metric screw hole cutter with optional counterbore / countersink. Use in \"cut\".",
        fields: &[
            req("size", FieldKind::Enum(SCREW_SIZES), "Metric screw size."),
            alt("depth", FieldKind::Num, "Hole depth (required unless through)."),
            opt("through", FieldKind::Bool, "false", "Cut through the material under the face it is placed on (measured automatically)."),
            opt("fit", FieldKind::Enum(SCREW_FIT), "clearance", "\"clearance\" = screw passes freely; \"tap\" = screw cuts its own thread in plastic; \"insert\" = hole for a heat-set threaded insert."),
            opt("head", FieldKind::Enum(SCREW_HEAD), "none", "Recess for the screw head at the entry."),
        ],
        exactly_one: &[],
        at_most_one: &[&["depth", "through"]],
        anchors: &[],
        notes: "The entry (head end) is the cutter's top: placed with \"on\" a face, it drills into that face.",
        example: r#"{ "type": "screw_hole", "size": "M3", "through": true, "head": "countersunk" }"#,
    },
    ComponentSpec {
        name: "nut_trap",
        category: "hardware",
        summary: "Hexagonal pocket that holds a metric nut. Use in \"cut\".",
        fields: &[
            req("size", FieldKind::Enum(SCREW_SIZES), "Metric nut size."),
            alt("depth", FieldKind::Num, "Pocket depth (default: nut thickness + 0.3)."),
            opt("clearance", FieldKind::Num, "0.15", "Extra gap per side."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "nut_trap", "size": "M3" }"#,
    },
    ComponentSpec {
        name: "magnet_pocket",
        category: "hardware",
        summary: "Round pocket for a disc magnet. Use in \"cut\".",
        fields: &[
            req("d", FieldKind::Num, "Magnet diameter."),
            req("h", FieldKind::Num, "Magnet thickness."),
            opt("clearance", FieldKind::Num, "0.15", "Extra gap per side (diameter grows by 2x)."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "Depth is h + 0.2 so the magnet sits flush.",
        example: r#"{ "type": "magnet_pocket", "d": 6, "h": 3 }"#,
    },
    ComponentSpec {
        name: "standoff",
        category: "hardware",
        summary: "Round post with a screw hole, for mounting circuit boards. Use in \"add\".",
        fields: &[
            req("screw", FieldKind::Enum(SCREW_SIZES), "Screw size."),
            req("h", FieldKind::Num, "Post height."),
            opt("fit", FieldKind::Enum(STANDOFF_FIT), "tap", "\"tap\" = screw threads into plastic; \"insert\" = heat-set insert."),
            alt("d", FieldKind::Num, "Outer diameter (default: sized for the screw)."),
        ],
        exactly_one: &[],
        at_most_one: &[],
        anchors: &[],
        notes: "",
        example: r#"{ "type": "standoff", "screw": "M2.5", "h": 5 }"#,
    },
];

pub fn component(name: &str) -> Option<&'static ComponentSpec> {
    COMPONENTS.iter().find(|c| c.name == name)
}

#[derive(Clone, Copy, Debug)]
pub struct JointSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub fields: &'static [FieldSpec],
    pub notes: &'static str,
}

pub const JOINTS: &[JointSpec] = &[
    JointSpec {
        name: "dovetail",
        summary: "Dovetail keys on the first piece slide down into matching slots in the second. Self-locking sideways; for split axis x or y.",
        fields: &[
            opt("count", FieldKind::Int, "2", "Number of dovetails."),
            opt("width", FieldKind::Num, "10", "Neck width at the seam."),
            opt("depth", FieldKind::Num, "8", "How far the key reaches into the other piece."),
            opt("angle", FieldKind::Num, "15", "Flare angle of the sides in degrees (10-20)."),
            opt("clearance", FieldKind::Num, "0.15", "Gap per side between key and slot."),
            alt("positions", FieldKind::NumList, "Positions along the seam (the horizontal axis lying in the cut plane, world mm). Default: spread automatically over solid material."),
        ],
        notes: "Each key spans the full material height where it sits, so it can slide in from above.",
    },
    JointSpec {
        name: "pin",
        summary: "Matching holes in both pieces plus separately printed alignment pins (or use filament / dowels). Works on any split axis; glue for strength.",
        fields: &[
            opt("count", FieldKind::Int, "2", "Number of pins."),
            opt("d", FieldKind::Num, "4", "Pin diameter."),
            opt("length", FieldKind::Num, "12", "Total pin length (half goes into each piece)."),
            opt("clearance", FieldKind::Num, "0.15", "Gap per side in the holes."),
            opt("make_pins", FieldKind::Bool, "true", "Also output the pins as printable parts."),
            alt("positions", FieldKind::Points2, "[[u, v], ...] positions in the cut plane (world mm). Default: automatic."),
        ],
        notes: "",
    },
    JointSpec {
        name: "magnet",
        summary: "Facing pockets for disc magnets on both sides of the seam. Removable connection; any split axis.",
        fields: &[
            opt("count", FieldKind::Int, "2", "Number of magnet pairs."),
            opt("d", FieldKind::Num, "6", "Magnet diameter."),
            opt("h", FieldKind::Num, "3", "Magnet thickness."),
            opt("clearance", FieldKind::Num, "0.1", "Gap per side."),
            alt("positions", FieldKind::Points2, "[[u, v], ...] positions in the cut plane (world mm). Default: automatic."),
        ],
        notes: "Mind the polarity when gluing magnets in.",
    },
    JointSpec {
        name: "none",
        summary: "Plain cut, no connector (glue or print as separate pieces).",
        fields: &[],
        notes: "",
    },
];

pub fn joint(name: &str) -> Option<&'static JointSpec> {
    JOINTS.iter().find(|j| j.name == name)
}

/// Generic anchors every shape has, from its bounding box.
pub const GENERIC_ANCHORS: &str = "top, bottom, left (-X), right (+X), front (-Y), back (+Y), center, and combinations joined with '-' such as top-left, bottom-front-right";

/// Markdown reference for all components and joints.
pub fn components_markdown() -> String {
    let mut s = String::new();
    s.push_str("# Component reference\n\n");
    s.push_str("Generated from the engine's component specs (`pd docs components`). Every value can be a number or an expression string.\n\n");
    s.push_str("Every shape is built in its own local frame: centred on X and Y, sitting on z = 0, Z up. Every shape has the generic anchors: ");
    s.push_str(GENERIC_ANCHORS);
    s.push_str(".\n\n");
    let mut cat = "";
    for c in COMPONENTS {
        if c.category != cat {
            cat = c.category;
            s.push_str(&format!("## {} components\n\n", capitalize(cat)));
        }
        s.push_str(&format!("### `{}`\n\n{}\n\n", c.name, c.summary));
        s.push_str("| field | type | default | description |\n|---|---|---|---|\n");
        for f in c.fields {
            let def = if f.required {
                "**required**".to_string()
            } else {
                f.default
                    .map(|d| format!("`{d}`"))
                    .unwrap_or_else(|| "—".into())
            };
            s.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                f.name,
                f.kind.describe(),
                def,
                f.doc
            ));
        }
        s.push('\n');
        for g in c.exactly_one {
            s.push_str(&format!(
                "Give exactly one of: {}.\n\n",
                g.iter()
                    .map(|x| format!("`{x}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for g in c.at_most_one {
            s.push_str(&format!(
                "Give at most one of: {}.\n\n",
                g.iter()
                    .map(|x| format!("`{x}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for (a, d) in c.anchors {
            s.push_str(&format!("Extra anchor `{a}`: {d}\n\n"));
        }
        if !c.notes.is_empty() {
            s.push_str(&format!("{}\n\n", c.notes));
        }
        s.push_str(&format!("```json\n{}\n```\n\n", c.example));
    }
    s.push_str("## Joints (for `splits`)\n\n");
    for j in JOINTS {
        s.push_str(&format!("### `{}`\n\n{}\n\n", j.name, j.summary));
        if !j.fields.is_empty() {
            s.push_str("| field | type | default | description |\n|---|---|---|---|\n");
            for f in j.fields {
                let def = f
                    .default
                    .map(|d| format!("`{d}`"))
                    .unwrap_or_else(|| "—".into());
                s.push_str(&format!(
                    "| `{}` | {} | {} | {} |\n",
                    f.name,
                    f.kind.describe(),
                    def,
                    f.doc
                ));
            }
            s.push('\n');
        }
        if !j.notes.is_empty() {
            s.push_str(&format!("{}\n\n", j.notes));
        }
    }
    s
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Compact JSON description of components, for the MCP `list_components` tool.
pub fn components_json(filter: Option<&str>) -> serde_json::Value {
    let fields = |fs: &[FieldSpec]| -> Vec<serde_json::Value> {
        fs.iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.name, "type": f.kind.describe(), "required": f.required,
                    "default": f.default, "doc": f.doc
                })
            })
            .collect()
    };
    let comps: Vec<serde_json::Value> = COMPONENTS
        .iter()
        .filter(|c| filter.map_or(true, |f| c.name == f))
        .map(|c| {
            serde_json::json!({
                "type": c.name, "category": c.category, "summary": c.summary,
                "fields": fields(c.fields),
                "exactly_one_of": c.exactly_one, "at_most_one_of": c.at_most_one,
                "anchors": c.anchors.iter().map(|(a, d)| serde_json::json!({"name": a, "doc": d})).collect::<Vec<_>>(),
                "notes": c.notes, "example": serde_json::from_str::<serde_json::Value>(c.example).unwrap_or_default(),
            })
        })
        .collect();
    let joints: Vec<serde_json::Value> = JOINTS
        .iter()
        .filter(|j| filter.map_or(true, |f| j.name == f))
        .map(|j| serde_json::json!({"type": j.name, "summary": j.summary, "fields": fields(j.fields), "notes": j.notes}))
        .collect();
    serde_json::json!({ "generic_anchors": GENERIC_ANCHORS, "components": comps, "joints": joints })
}
