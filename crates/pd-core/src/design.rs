//! The design document (format `pd/0.1`): types and a parser that reports
//! every problem it finds, each with a JSON path and a fix hint.

use crate::expr::{self, Expr, Value};
use crate::issues::{suggest, Issue};
use crate::spec::{self, FieldKind, FieldSpec};
use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;

pub const FORMAT: &str = "pd/0.1";

/// A value that is either a literal or an expression evaluated later.
#[derive(Clone, Debug)]
pub enum Val {
    Lit(Value),
    Expr { expr: Expr, src: String },
}

#[derive(Clone, Debug)]
pub enum FieldVal {
    One(Val),
    List(Vec<FieldVal>),
}

#[derive(Clone, Debug)]
pub struct ShapeDef {
    pub ty: String,
    pub fields: BTreeMap<String, FieldVal>,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamKind {
    Number,
    Integer,
    Boolean,
    Choice,
    Color,
}

impl ParamKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParamKind::Number => "number",
            ParamKind::Integer => "integer",
            ParamKind::Boolean => "boolean",
            ParamKind::Choice => "choice",
            ParamKind::Color => "color",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParamDef {
    pub name: String,
    pub kind: ParamKind,
    pub value: Value,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub unit: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub group: Option<String>,
    pub advanced: bool,
    pub options: Vec<Value>,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct PlaceDef {
    pub on: Option<String>,
    pub at: Option<[Val; 3]>,
    pub anchor: Option<String>,
    pub offset: Option<[Val; 3]>,
    pub rotate: Option<[Val; 3]>,
    pub orient: Option<bool>,
    pub path: String,
}

#[derive(Clone, Debug)]
pub enum RepeatDef {
    Grid {
        n: [Val; 2],
        spacing: [Val; 2],
    },
    Linear {
        n: Val,
        step: [Val; 3],
        center: bool,
    },
    Radial {
        n: Val,
        radius: Val,
        start_angle: Val,
        rotate: bool,
    },
}

#[derive(Clone, Debug)]
pub struct FeatureDef {
    pub id: Option<String>,
    pub shape: ShapeDef,
    pub place: Option<PlaceDef>,
    pub repeat: Option<RepeatDef>,
    pub mirror: Option<String>,
    pub enabled: Option<Val>,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct PartDef {
    pub id: String,
    pub name: Option<String>,
    pub shape: ShapeDef,
    pub add: Vec<FeatureDef>,
    pub cut: Vec<FeatureDef>,
    pub color: Option<Val>,
    pub place: Option<PlaceDef>,
    pub enabled: Option<Val>,
    pub print_rotate: Option<[Val; 3]>,
    pub allow_disconnected: bool,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct JointDef {
    pub ty: String,
    pub fields: BTreeMap<String, FieldVal>,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct SplitDef {
    pub part: String,
    pub axis: char,
    pub at: Val,
    pub names: [String; 2],
    pub joint: Option<JointDef>,
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub struct PrintSettings {
    pub layer_height: Option<f64>,
    pub infill: Option<f64>,
    pub walls: Option<f64>,
    pub supports: Option<bool>,
    pub nozzle: Option<f64>,
    pub material: Option<String>,
    pub bed: Option<[f64; 3]>,
}

#[derive(Clone, Debug)]
pub struct Design {
    pub name: String,
    pub meta: Json,
    pub params: Vec<ParamDef>,
    pub derived: Vec<(String, Val, String)>,
    pub parts: Vec<PartDef>,
    pub splits: Vec<SplitDef>,
    pub print: PrintSettings,
}

impl Design {
    pub fn param(&self, name: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.name == name)
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct P {
    issues: Vec<Issue>,
}

fn json_type(v: &Json) -> &'static str {
    match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "text",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

fn is_comment_key(k: &str) -> bool {
    k.starts_with('_') || k.starts_with('$') || k == "comment" || k == "notes"
}

pub fn is_identifier(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

// Function names live in their own namespace (`floor(x)` vs a param `floor`).
const RESERVED: &[&str] = &["true", "false", "pi", "PI"];

pub const NAMED_COLORS: &[(&str, &str)] = &[
    ("black", "#1a1a1a"),
    ("white", "#f2f2f2"),
    ("gray", "#8c8c8c"),
    ("grey", "#8c8c8c"),
    ("silver", "#c0c0c0"),
    ("red", "#d62828"),
    ("orange", "#f77f00"),
    ("yellow", "#fcbf49"),
    ("green", "#2a9d48"),
    ("lime", "#8ac926"),
    ("blue", "#1d6fd8"),
    ("navy", "#1d3557"),
    ("cyan", "#20c4d8"),
    ("purple", "#7b2cbf"),
    ("pink", "#ff70a6"),
    ("brown", "#8b5a2b"),
    ("beige", "#e9d8a6"),
    ("gold", "#d4a017"),
];

/// Normalise a literal colour (`#rgb`, `#rrggbb`, `#rrggbbaa`, or a name) to `#rrggbb`.
pub fn literal_color(s: &str) -> Option<String> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix('#') {
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        return match hex.len() {
            3 => Some(format!(
                "#{}",
                hex.chars()
                    .flat_map(|c| [c, c])
                    .collect::<String>()
                    .to_lowercase()
            )),
            6 => Some(format!("#{}", hex.to_lowercase())),
            8 => Some(format!("#{}", hex[..6].to_lowercase())),
            _ => None,
        };
    }
    NAMED_COLORS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(t))
        .map(|(_, h)| h.to_string())
}

impl P {
    fn err(&mut self, code: &'static str, path: &str, msg: impl Into<String>) -> &mut Issue {
        self.issues.push(Issue::error(code, path, msg));
        self.issues.last_mut().unwrap()
    }
    fn warn(&mut self, code: &'static str, path: &str, msg: impl Into<String>) -> &mut Issue {
        self.issues.push(Issue::warning(code, path, msg));
        self.issues.last_mut().unwrap()
    }

    fn obj<'a>(&mut self, v: &'a Json, path: &str, what: &str) -> Option<&'a Map<String, Json>> {
        match v {
            Json::Object(m) => Some(m),
            other => {
                self.err(
                    "wrong_type",
                    path,
                    format!("{what} must be an object {{...}}, got {}", json_type(other)),
                );
                None
            }
        }
    }

    fn check_keys(&mut self, m: &Map<String, Json>, allowed: &[&str], path: &str, what: &str) {
        for k in m.keys() {
            if is_comment_key(k) || allowed.contains(&k.as_str()) {
                continue;
            }
            let p = join(path, k);
            let hint = match suggest(k, allowed.iter().copied()) {
                Some(s) => format!(
                    "Did you mean \"{s}\"? Allowed keys for {what}: {}.",
                    allowed.join(", ")
                ),
                None => format!("Allowed keys for {what}: {}.", allowed.join(", ")),
            };
            self.err(
                "unknown_field",
                &p,
                format!("unknown key \"{k}\" in {what}"),
            )
            .hint = Some(hint);
        }
    }

    fn string(&mut self, v: &Json, path: &str) -> Option<String> {
        match v {
            Json::String(s) => Some(s.clone()),
            other => {
                self.err(
                    "wrong_type",
                    path,
                    format!("expected text, got {}", json_type(other)),
                );
                None
            }
        }
    }

    fn literal_num(&mut self, v: &Json, path: &str) -> Option<f64> {
        match v {
            Json::Number(n) => n.as_f64(),
            other => {
                self.err(
                    "wrong_type",
                    path,
                    format!(
                        "expected a plain number here (no expressions), got {}",
                        json_type(other)
                    ),
                );
                None
            }
        }
    }

    fn bool(&mut self, v: &Json, path: &str) -> Option<bool> {
        match v {
            Json::Bool(b) => Some(*b),
            other => {
                self.err(
                    "wrong_type",
                    path,
                    format!("expected true or false, got {}", json_type(other)),
                );
                None
            }
        }
    }

    /// Number / boolean / expression string.
    fn val(&mut self, v: &Json, path: &str) -> Option<Val> {
        match v {
            Json::Number(n) => Some(Val::Lit(Value::Num(n.as_f64().unwrap_or(0.0)))),
            Json::Bool(b) => Some(Val::Lit(Value::Bool(*b))),
            Json::String(s) => match expr::parse(s) {
                Ok(e) => Some(Val::Expr {
                    expr: e,
                    src: s.clone(),
                }),
                Err(e) => {
                    self.err(
                        "bad_expression",
                        path,
                        format!("cannot read expression \"{s}\": {}", e.message),
                    )
                    .hint = e.hint;
                    None
                }
            },
            other => {
                self.err(
                    "wrong_type",
                    path,
                    format!(
                        "expected a number or an expression string, got {}",
                        json_type(other)
                    ),
                )
                .hint =
                    Some("Write numbers like 12.5 or expressions like \"width + 2*wall\".".into());
                None
            }
        }
    }

    fn vec_n<const N: usize>(&mut self, v: &Json, path: &str, broadcast: bool) -> Option<[Val; N]> {
        match v {
            Json::Array(items) if items.len() == N => {
                let vals: Vec<Option<Val>> = items
                    .iter()
                    .enumerate()
                    .map(|(i, it)| self.val(it, &format!("{path}[{i}]")))
                    .collect();
                if vals.iter().all(|x| x.is_some()) {
                    let vals: Vec<Val> = vals.into_iter().map(|x| x.unwrap()).collect();
                    vals.try_into().ok()
                } else {
                    None
                }
            }
            Json::Array(items) => {
                self.err(
                    "wrong_length",
                    path,
                    format!("expected {N} values, got {}", items.len()),
                );
                None
            }
            Json::Number(_) | Json::String(_) if broadcast => {
                let one = self.val(v, path)?;
                Some(std::array::from_fn(|_| one.clone()))
            }
            other => {
                self.err(
                    "wrong_type",
                    path,
                    format!("expected an array of {N} values, got {}", json_type(other)),
                );
                None
            }
        }
    }

    fn field(&mut self, spec: &FieldSpec, v: &Json, path: &str) -> Option<FieldVal> {
        match spec.kind {
            FieldKind::Num | FieldKind::Int | FieldKind::Bool => {
                self.val(v, path).map(FieldVal::One)
            }
            FieldKind::Vec2 => self
                .vec_n::<2>(v, path, true)
                .map(|a| FieldVal::List(a.into_iter().map(FieldVal::One).collect())),
            FieldKind::Vec3 => self
                .vec_n::<3>(v, path, true)
                .map(|a| FieldVal::List(a.into_iter().map(FieldVal::One).collect())),
            FieldKind::NumList => match v {
                Json::Array(items) => {
                    let mut out = Vec::new();
                    for (i, it) in items.iter().enumerate() {
                        out.push(FieldVal::One(self.val(it, &format!("{path}[{i}]"))?));
                    }
                    Some(FieldVal::List(out))
                }
                other => {
                    self.err(
                        "wrong_type",
                        path,
                        format!("expected an array of numbers, got {}", json_type(other)),
                    );
                    None
                }
            },
            FieldKind::Points2 => match v {
                Json::Array(items) => {
                    let mut out = Vec::new();
                    for (i, it) in items.iter().enumerate() {
                        let p = self.vec_n::<2>(it, &format!("{path}[{i}]"), false)?;
                        out.push(FieldVal::List(p.into_iter().map(FieldVal::One).collect()));
                    }
                    Some(FieldVal::List(out))
                }
                other => {
                    self.err(
                        "wrong_type",
                        path,
                        format!(
                            "expected an array of [x, y] points, got {}",
                            json_type(other)
                        ),
                    );
                    None
                }
            },
            FieldKind::Enum(opts) => match v {
                Json::String(s) if opts.contains(&s.as_str()) => {
                    Some(FieldVal::One(Val::Lit(Value::Str(s.clone()))))
                }
                Json::String(s) => {
                    // Not a literal option: it may be an expression such as a choice
                    // param name (checked against the options when evaluated).
                    let typo = suggest(s, opts.iter().copied()).is_some();
                    if !typo && expr::parse(s).is_ok() {
                        self.val(v, path).map(FieldVal::One)
                    } else {
                        let hint = suggest(s, opts.iter().copied())
                            .map(|x| format!("Did you mean \"{x}\"?"))
                            .unwrap_or_else(|| format!("Use one of: {}.", spec.kind.describe()));
                        self.err("bad_option", path, format!("\"{s}\" is not a valid option"))
                            .hint = Some(hint);
                        None
                    }
                }
                other => {
                    self.err(
                        "wrong_type",
                        path,
                        format!(
                            "expected one of {}, got {}",
                            spec.kind.describe(),
                            json_type(other)
                        ),
                    );
                    None
                }
            },
            FieldKind::PartRef => self
                .string(v, path)
                .map(|s| FieldVal::One(Val::Lit(Value::Str(s)))),
        }
    }

    fn shape(&mut self, v: &Json, path: &str) -> Option<ShapeDef> {
        let m = self.obj(v, path, "a shape")?;
        let ty = match m.get("type") {
            Some(Json::String(t)) => t.clone(),
            Some(other) => {
                self.err(
                    "wrong_type",
                    &join(path, "type"),
                    format!("shape type must be text, got {}", json_type(other)),
                );
                return None;
            }
            None => {
                let guess = m.keys().find(|k| spec::component(k).is_some()).cloned();
                let hint = match guess {
                    Some(g) => format!(
                        "Write {{\"type\": \"{g}\", ...fields}} instead of {{\"{g}\": {{...}}}}."
                    ),
                    None => format!("Add \"type\": one of {}.", component_names()),
                };
                self.err("missing_field", path, "shape has no \"type\"")
                    .hint = Some(hint);
                return None;
            }
        };
        let Some(cs) = spec::component(&ty) else {
            let alias = alias_hint(&ty);
            let hint = alias
                .or_else(|| {
                    suggest(&ty, spec::COMPONENTS.iter().map(|c| c.name))
                        .map(|s| format!("Did you mean \"{s}\"?"))
                })
                .unwrap_or_else(|| format!("Available types: {}.", component_names()));
            self.err(
                "unknown_type",
                &join(path, "type"),
                format!("unknown shape type \"{ty}\""),
            )
            .hint = Some(hint);
            return None;
        };
        let allowed: Vec<&str> = std::iter::once("type")
            .chain(cs.fields.iter().map(|f| f.name))
            .collect();
        self.check_keys(m, &allowed, path, &format!("shape \"{ty}\""));
        let mut fields = BTreeMap::new();
        let mut ok = true;
        for f in cs.fields {
            match m.get(f.name) {
                Some(v) => match self.field(f, v, &join(path, f.name)) {
                    Some(fv) => {
                        fields.insert(f.name.to_string(), fv);
                    }
                    None => ok = false,
                },
                None if f.required => {
                    ok = false;
                    self.err(
                        "missing_field",
                        path,
                        format!("shape \"{ty}\" needs \"{}\" ({})", f.name, f.doc),
                    );
                }
                None => {}
            }
        }
        for g in cs.exactly_one {
            let given: Vec<&str> = g.iter().copied().filter(|k| m.contains_key(*k)).collect();
            if given.len() != 1 {
                ok = false;
                let list = g
                    .iter()
                    .map(|x| format!("\"{x}\""))
                    .collect::<Vec<_>>()
                    .join(" or ");
                if given.is_empty() {
                    self.err(
                        "missing_field",
                        path,
                        format!("shape \"{ty}\" needs {list}"),
                    );
                } else {
                    self.err(
                        "conflicting_fields",
                        path,
                        format!("shape \"{ty}\" takes only one of {list}, not both"),
                    );
                }
            }
        }
        for g in cs.at_most_one {
            let given: Vec<&str> = g.iter().copied().filter(|k| m.contains_key(*k)).collect();
            if given.len() > 1 {
                ok = false;
                self.err(
                    "conflicting_fields",
                    path,
                    format!("shape \"{ty}\" takes at most one of {}", g.join(", ")),
                );
            }
        }
        if ty == "screw_hole"
            && !m.contains_key("depth")
            && !matches!(m.get("through"), Some(Json::Bool(true)))
            && m.get("fit").and_then(|f| f.as_str()) != Some("insert")
        {
            ok = false;
            self.err(
                "missing_field",
                path,
                "screw_hole needs \"depth\" or \"through\": true",
            );
        }
        ok.then(|| ShapeDef {
            ty,
            fields,
            path: path.to_string(),
        })
    }

    fn place(&mut self, v: &Json, path: &str) -> Option<PlaceDef> {
        let m = self.obj(v, path, "place")?;
        self.check_keys(
            m,
            &["on", "at", "anchor", "offset", "rotate", "orient"],
            path,
            "place",
        );
        let mut p = PlaceDef {
            on: None,
            at: None,
            anchor: None,
            offset: None,
            rotate: None,
            orient: None,
            path: path.to_string(),
        };
        if let Some(on) = m.get("on") {
            p.on = self.string(on, &join(path, "on"));
        }
        if let Some(at) = m.get("at") {
            p.at = self.vec_n::<3>(at, &join(path, "at"), false);
        }
        if p.on.is_some() && p.at.is_some() {
            self.err(
                "conflicting_fields",
                path,
                "use either \"on\" (anchor) or \"at\" (coordinates), not both",
            );
        }
        if let Some(a) = m.get("anchor") {
            p.anchor = self.string(a, &join(path, "anchor"));
        }
        if let Some(o) = m.get("offset") {
            p.offset = self.vec_n::<3>(o, &join(path, "offset"), false);
        }
        if let Some(r) = m.get("rotate") {
            p.rotate = self.vec_n::<3>(r, &join(path, "rotate"), false);
        }
        if let Some(o) = m.get("orient") {
            match o {
                Json::String(s) if s == "normal" => p.orient = Some(true),
                Json::String(s) if s == "none" => p.orient = Some(false),
                _ => {
                    self.err(
                        "bad_option",
                        &join(path, "orient"),
                        "orient must be \"normal\" or \"none\"",
                    );
                }
            }
        }
        Some(p)
    }

    fn repeat(&mut self, v: &Json, path: &str) -> Option<RepeatDef> {
        let m = self.obj(v, path, "repeat")?;
        if m.contains_key("grid") {
            self.check_keys(m, &["grid", "spacing"], path, "a grid repeat");
            let n = self.vec_n::<2>(&m["grid"], &join(path, "grid"), false)?;
            let Some(sp) = m.get("spacing") else {
                self.err(
                    "missing_field",
                    path,
                    "grid repeat needs \"spacing\": [x, y] (centre-to-centre distance)",
                );
                return None;
            };
            let spacing = self.vec_n::<2>(sp, &join(path, "spacing"), true)?;
            return Some(RepeatDef::Grid { n, spacing });
        }
        if m.contains_key("linear") {
            self.check_keys(m, &["linear", "step", "center"], path, "a linear repeat");
            let n = self.val(&m["linear"], &join(path, "linear"))?;
            let Some(st) = m.get("step") else {
                self.err(
                    "missing_field",
                    path,
                    "linear repeat needs \"step\": [dx, dy, dz] between copies",
                );
                return None;
            };
            let step = self.vec_n::<3>(st, &join(path, "step"), false)?;
            let center = match m.get("center") {
                Some(c) => self.bool(c, &join(path, "center"))?,
                None => true,
            };
            return Some(RepeatDef::Linear { n, step, center });
        }
        if m.contains_key("radial") {
            self.check_keys(
                m,
                &["radial", "radius", "start_angle", "rotate"],
                path,
                "a radial repeat",
            );
            let n = self.val(&m["radial"], &join(path, "radial"))?;
            let radius = match m.get("radius") {
                Some(r) => self.val(r, &join(path, "radius"))?,
                None => {
                    self.err("missing_field", path, "radial repeat needs \"radius\"");
                    return None;
                }
            };
            let start_angle = match m.get("start_angle") {
                Some(a) => self.val(a, &join(path, "start_angle"))?,
                None => Val::Lit(Value::Num(0.0)),
            };
            let rotate = match m.get("rotate") {
                Some(r) => self.bool(r, &join(path, "rotate"))?,
                None => true,
            };
            return Some(RepeatDef::Radial {
                n,
                radius,
                start_angle,
                rotate,
            });
        }
        self.err("bad_repeat", path, "repeat needs one of \"grid\", \"linear\" or \"radial\"").hint = Some(
            "{\"grid\": [3, 2], \"spacing\": [20, 20]} | {\"linear\": 4, \"step\": [10, 0, 0]} | {\"radial\": 6, \"radius\": 15}".into(),
        );
        None
    }

    fn feature(&mut self, v: &Json, path: &str) -> Option<FeatureDef> {
        let m = self.obj(v, path, "a feature")?;
        if m.contains_key("type") && !m.contains_key("shape") {
            self.err(
                "wrong_structure",
                path,
                "feature has \"type\" directly; it must be wrapped in \"shape\"",
            )
            .hint = Some("Write {\"shape\": {\"type\": ..., ...}, \"place\": {...}}".into());
            return None;
        }
        self.check_keys(
            m,
            &["id", "shape", "place", "repeat", "mirror", "enabled"],
            path,
            "a feature",
        );
        let id = m.get("id").and_then(|v| self.string(v, &join(path, "id")));
        let shape = match m.get("shape") {
            Some(s) => self.shape(s, &join(path, "shape")),
            None => {
                self.err("missing_field", path, "feature needs \"shape\"");
                None
            }
        };
        let place = m
            .get("place")
            .and_then(|p| self.place(p, &join(path, "place")));
        let repeat = m
            .get("repeat")
            .and_then(|r| self.repeat(r, &join(path, "repeat")));
        let mirror = match m.get("mirror") {
            Some(v) => match self.string(v, &join(path, "mirror")) {
                Some(s) if ["x", "y", "xy"].contains(&s.as_str()) => Some(s),
                Some(_) => {
                    self.err(
                        "bad_option",
                        &join(path, "mirror"),
                        "mirror must be \"x\", \"y\" or \"xy\"",
                    )
                    .hint = Some(
                        "\"x\" adds a copy mirrored across the part's YZ plane (x -> -x).".into(),
                    );
                    None
                }
                None => None,
            },
            None => None,
        };
        let enabled = m
            .get("enabled")
            .and_then(|e| self.val(e, &join(path, "enabled")));
        Some(FeatureDef {
            id,
            shape: shape?,
            place,
            repeat,
            mirror,
            enabled,
            path: path.to_string(),
        })
    }

    fn features(&mut self, v: Option<&Json>, path: &str) -> Vec<FeatureDef> {
        match v {
            None => vec![],
            Some(Json::Array(items)) => items
                .iter()
                .enumerate()
                .filter_map(|(i, f)| self.feature(f, &format!("{path}[{i}]")))
                .collect(),
            Some(other) => {
                self.err(
                    "wrong_type",
                    path,
                    format!("expected an array of features, got {}", json_type(other)),
                );
                vec![]
            }
        }
    }

    fn part(&mut self, v: &Json, path: &str) -> Option<PartDef> {
        let m = self.obj(v, path, "a part")?;
        self.check_keys(
            m,
            &[
                "id",
                "name",
                "description",
                "shape",
                "add",
                "cut",
                "color",
                "place",
                "enabled",
                "print_rotate",
                "allow_disconnected",
            ],
            path,
            "a part",
        );
        let id = match m.get("id") {
            Some(v) => self.string(v, &join(path, "id")),
            None => {
                self.err(
                    "missing_field",
                    path,
                    "part needs an \"id\" (short name like \"box\" or \"lid\")",
                );
                None
            }
        };
        if let Some(id) = &id {
            if !is_identifier(id) {
                self.err("bad_id", &join(path, "id"), format!("part id \"{id}\" must use only letters, digits and _ (no spaces, dots or dashes)"));
            }
        }
        let name = m
            .get("name")
            .and_then(|v| self.string(v, &join(path, "name")));
        let shape = match m.get("shape") {
            Some(s) => self.shape(s, &join(path, "shape")),
            None => {
                self.err("missing_field", path, "part needs \"shape\"");
                None
            }
        };
        let add = self.features(m.get("add"), &join(path, "add"));
        let cut = self.features(m.get("cut"), &join(path, "cut"));
        let color = match m.get("color") {
            Some(Json::String(s)) => match literal_color(s) {
                Some(c) => Some(Val::Lit(Value::Str(c))),
                None => self.val(&Json::String(s.clone()), &join(path, "color")),
            },
            Some(other) => {
                self.err(
                    "wrong_type",
                    &join(path, "color"),
                    format!(
                        "color must be text like \"#ff7a00\" or a color param name, got {}",
                        json_type(other)
                    ),
                );
                None
            }
            None => None,
        };
        let place = m
            .get("place")
            .and_then(|p| self.place(p, &join(path, "place")));
        let enabled = m
            .get("enabled")
            .and_then(|e| self.val(e, &join(path, "enabled")));
        let print_rotate = m
            .get("print_rotate")
            .and_then(|r| self.vec_n::<3>(r, &join(path, "print_rotate"), false));
        let allow_disconnected = match m.get("allow_disconnected") {
            Some(v) => self
                .bool(v, &join(path, "allow_disconnected"))
                .unwrap_or(false),
            None => false,
        };
        Some(PartDef {
            id: id?,
            name,
            shape: shape?,
            add,
            cut,
            color,
            place,
            enabled,
            print_rotate,
            allow_disconnected,
            path: path.to_string(),
        })
    }

    fn param(&mut self, name: &str, v: &Json, path: &str) -> Option<ParamDef> {
        if !is_identifier(name) {
            self.err("bad_name", path, format!("param name \"{name}\" must use only letters, digits and _ and not start with a digit"));
            return None;
        }
        if RESERVED.contains(&name) {
            self.err(
                "bad_name",
                path,
                format!("\"{name}\" is a reserved word; pick another param name"),
            );
            return None;
        }
        let mut def = ParamDef {
            name: name.to_string(),
            kind: ParamKind::Number,
            value: Value::Num(0.0),
            min: None,
            max: None,
            step: None,
            unit: None,
            label: None,
            description: None,
            group: None,
            advanced: false,
            options: vec![],
            path: path.to_string(),
        };
        let m = match v {
            Json::Object(m) => m,
            Json::Number(_) | Json::Bool(_) | Json::String(_) => {
                def.value = Value::from_json(v)?;
                def.kind = match &def.value {
                    Value::Bool(_) => ParamKind::Boolean,
                    Value::Str(s) if literal_color(s).is_some() => ParamKind::Color,
                    Value::Str(_) => {
                        self.err("bad_param", path, "text params need \"options\"")
                            .hint = Some(format!(
                            "\"{name}\": {{\"value\": \"a\", \"options\": [\"a\", \"b\"]}}"
                        ));
                        return None;
                    }
                    Value::Num(_) => ParamKind::Number,
                };
                if def.kind == ParamKind::Number {
                    self.warn("param_no_range", path, format!("param \"{name}\" has no min/max, so its slider range has to be guessed"))
                        .hint = Some(format!("\"{name}\": {{\"value\": {}, \"min\": ..., \"max\": ..., \"unit\": \"mm\", \"label\": \"...\"}}", v));
                }
                return Some(def);
            }
            other => {
                self.err(
                    "wrong_type",
                    path,
                    format!(
                        "param must be an object or a plain value, got {}",
                        json_type(other)
                    ),
                );
                return None;
            }
        };
        self.check_keys(
            m,
            &[
                "type",
                "value",
                "min",
                "max",
                "step",
                "unit",
                "label",
                "description",
                "group",
                "advanced",
                "options",
            ],
            path,
            "a param",
        );
        let Some(value) = m.get("value") else {
            self.err(
                "missing_field",
                path,
                format!("param \"{name}\" needs \"value\" (its default)"),
            );
            return None;
        };
        def.value = match Value::from_json(value) {
            Some(v) => v,
            None => {
                self.err(
                    "wrong_type",
                    &join(path, "value"),
                    "param value must be a number, boolean or text",
                );
                return None;
            }
        };
        let declared = m
            .get("type")
            .and_then(|t| self.string(t, &join(path, "type")));
        def.kind = match declared.as_deref() {
            Some("number") => ParamKind::Number,
            Some("integer") | Some("int") => ParamKind::Integer,
            Some("boolean") | Some("bool") => ParamKind::Boolean,
            Some("choice") => ParamKind::Choice,
            Some("color") | Some("colour") => ParamKind::Color,
            Some(other) => {
                self.err(
                    "bad_option",
                    &join(path, "type"),
                    format!("unknown param type \"{other}\""),
                )
                .hint = Some("Use number, integer, boolean, choice or color.".into());
                return None;
            }
            None => {
                if m.contains_key("options") {
                    ParamKind::Choice
                } else {
                    match &def.value {
                        Value::Bool(_) => ParamKind::Boolean,
                        Value::Str(s) if literal_color(s).is_some() => ParamKind::Color,
                        Value::Str(_) => ParamKind::Choice,
                        Value::Num(_) => ParamKind::Number,
                    }
                }
            }
        };
        for (key, slot) in [
            ("min", &mut def.min),
            ("max", &mut def.max),
            ("step", &mut def.step),
        ] {
            if let Some(v) = m.get(key) {
                *slot = self.literal_num(v, &join(path, key));
            }
        }
        for (key, slot) in [
            ("unit", &mut def.unit),
            ("label", &mut def.label),
            ("description", &mut def.description),
            ("group", &mut def.group),
        ] {
            if let Some(v) = m.get(key) {
                *slot = self.string(v, &join(path, key));
            }
        }
        if let Some(a) = m.get("advanced") {
            def.advanced = self.bool(a, &join(path, "advanced")).unwrap_or(false);
        }
        if let Some(o) = m.get("options") {
            match o {
                Json::Array(items) if !items.is_empty() => {
                    def.options = items.iter().filter_map(Value::from_json).collect()
                }
                _ => {
                    self.err(
                        "wrong_type",
                        &join(path, "options"),
                        "options must be a non-empty array",
                    );
                }
            }
        }
        match def.kind {
            ParamKind::Number | ParamKind::Integer => {
                let Value::Num(v) = def.value else {
                    self.err(
                        "wrong_type",
                        &join(path, "value"),
                        format!("{} param needs a numeric value", def.kind.as_str()),
                    );
                    return None;
                };
                if def.kind == ParamKind::Integer && v.fract() != 0.0 {
                    self.err(
                        "wrong_type",
                        &join(path, "value"),
                        "integer param needs a whole number",
                    );
                }
                match (def.min, def.max) {
                    (Some(lo), Some(hi)) => {
                        if lo > hi {
                            self.err(
                                "bad_range",
                                path,
                                format!("min ({lo}) is larger than max ({hi})"),
                            );
                        } else if v < lo || v > hi {
                            self.err(
                                "bad_range",
                                &join(path, "value"),
                                format!("value {v} is outside min..max ({lo}..{hi})"),
                            );
                        }
                    }
                    _ => {
                        self.warn("param_no_range", path, format!("param \"{name}\" should have both \"min\" and \"max\" so it gets a usable slider"));
                    }
                }
                if let Some(s) = def.step {
                    if s <= 0.0 {
                        self.err("bad_range", &join(path, "step"), "step must be positive");
                    }
                }
            }
            ParamKind::Boolean => {
                if !matches!(def.value, Value::Bool(_)) {
                    self.err(
                        "wrong_type",
                        &join(path, "value"),
                        "boolean param needs true or false",
                    );
                }
            }
            ParamKind::Choice => {
                if def.options.is_empty() {
                    self.err(
                        "missing_field",
                        path,
                        "choice param needs \"options\": [...]",
                    );
                } else if !def.options.contains(&def.value) {
                    self.err(
                        "bad_option",
                        &join(path, "value"),
                        format!("value {} is not one of the options", def.value),
                    );
                }
            }
            ParamKind::Color => match &def.value {
                Value::Str(s) => match literal_color(s) {
                    Some(c) => def.value = Value::Str(c),
                    None => {
                        self.err(
                            "bad_color",
                            &join(path, "value"),
                            format!("\"{s}\" is not a color"),
                        )
                        .hint = Some("Use \"#rrggbb\", e.g. \"#ff7a00\".".into());
                    }
                },
                _ => {
                    self.err(
                        "bad_color",
                        &join(path, "value"),
                        "color param needs text like \"#ff7a00\"",
                    );
                }
            },
        }
        Some(def)
    }

    fn joint(&mut self, v: &Json, path: &str) -> Option<JointDef> {
        let m = self.obj(v, path, "a joint")?;
        let ty = match m.get("type") {
            Some(t) => self.string(t, &join(path, "type"))?,
            None => {
                self.err("missing_field", path, "joint needs \"type\"").hint = Some(format!(
                    "One of: {}.",
                    spec::JOINTS
                        .iter()
                        .map(|j| j.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                return None;
            }
        };
        let Some(js) = spec::joint(&ty) else {
            let hint = suggest(&ty, spec::JOINTS.iter().map(|j| j.name))
                .map(|s| format!("Did you mean \"{s}\"?"))
                .unwrap_or_else(|| {
                    format!(
                        "One of: {}.",
                        spec::JOINTS
                            .iter()
                            .map(|j| j.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                });
            self.err(
                "unknown_type",
                &join(path, "type"),
                format!("unknown joint type \"{ty}\""),
            )
            .hint = Some(hint);
            return None;
        };
        let allowed: Vec<&str> = std::iter::once("type")
            .chain(js.fields.iter().map(|f| f.name))
            .collect();
        self.check_keys(m, &allowed, path, &format!("joint \"{ty}\""));
        let mut fields = BTreeMap::new();
        for f in js.fields {
            if let Some(v) = m.get(f.name) {
                fields.insert(f.name.to_string(), self.field(f, v, &join(path, f.name))?);
            }
        }
        Some(JointDef {
            ty,
            fields,
            path: path.to_string(),
        })
    }

    fn split(&mut self, v: &Json, path: &str) -> Option<SplitDef> {
        let m = self.obj(v, path, "a split")?;
        self.check_keys(
            m,
            &["part", "axis", "at", "names", "joint"],
            path,
            "a split",
        );
        let part = match m.get("part") {
            Some(p) => self.string(p, &join(path, "part")),
            None => {
                self.err(
                    "missing_field",
                    path,
                    "split needs \"part\" (id of the part to cut)",
                );
                None
            }
        };
        let axis = match m.get("axis").map(|a| self.string(a, &join(path, "axis"))) {
            Some(Some(a)) if ["x", "y", "z"].contains(&a.as_str()) => a.chars().next(),
            Some(Some(a)) => {
                self.err("bad_option", &join(path, "axis"), format!("axis \"{a}\" must be \"x\", \"y\" or \"z\"")).hint =
                    Some("axis is the direction the cut plane is perpendicular to: \"x\" cuts into left/right pieces.".into());
                None
            }
            Some(None) => None,
            None => {
                self.err(
                    "missing_field",
                    path,
                    "split needs \"axis\": \"x\", \"y\" or \"z\"",
                );
                None
            }
        };
        let at = match m.get("at") {
            Some(a) => self.val(a, &join(path, "at")),
            None => Some(Val::Lit(Value::Num(0.0))),
        };
        let names = match m.get("names") {
            Some(Json::Array(a)) if a.len() == 2 => {
                let n0 = self.string(&a[0], &join(path, "names[0]"));
                let n1 = self.string(&a[1], &join(path, "names[1]"));
                match (n0, n1) {
                    (Some(a), Some(b)) => {
                        for n in [&a, &b] {
                            if !is_identifier(n) {
                                self.err(
                                    "bad_id",
                                    &join(path, "names"),
                                    format!(
                                        "piece name \"{n}\" must use only letters, digits and _"
                                    ),
                                );
                            }
                        }
                        Some([a, b])
                    }
                    _ => None,
                }
            }
            Some(_) => {
                self.err(
                    "wrong_type",
                    &join(path, "names"),
                    "names must be two piece ids: [\"<negative side>\", \"<positive side>\"]",
                );
                None
            }
            None => part.as_ref().map(|p| [format!("{p}_a"), format!("{p}_b")]),
        };
        let joint = m
            .get("joint")
            .and_then(|j| self.joint(j, &join(path, "joint")));
        Some(SplitDef {
            part: part?,
            axis: axis?,
            at: at?,
            names: names?,
            joint,
            path: path.to_string(),
        })
    }

    fn print(&mut self, v: &Json, path: &str) -> PrintSettings {
        let mut ps = PrintSettings::default();
        let Some(m) = self.obj(v, path, "print") else {
            return ps;
        };
        self.check_keys(
            m,
            &[
                "layer_height",
                "infill",
                "walls",
                "supports",
                "nozzle",
                "material",
                "bed",
            ],
            path,
            "print",
        );
        for (k, slot) in [
            ("layer_height", &mut ps.layer_height),
            ("infill", &mut ps.infill),
            ("walls", &mut ps.walls),
            ("nozzle", &mut ps.nozzle),
        ] {
            if let Some(v) = m.get(k) {
                *slot = self.literal_num(v, &join(path, k));
            }
        }
        if let Some(v) = m.get("supports") {
            ps.supports = self.bool(v, &join(path, "supports"));
        }
        if let Some(v) = m.get("material") {
            ps.material = self.string(v, &join(path, "material"));
        }
        if let Some(Json::Array(b)) = m.get("bed") {
            if b.len() == 3 {
                let vals: Vec<f64> = b.iter().filter_map(|x| x.as_f64()).collect();
                if vals.len() == 3 {
                    ps.bed = Some([vals[0], vals[1], vals[2]]);
                }
            }
            if ps.bed.is_none() {
                self.err(
                    "wrong_type",
                    &join(path, "bed"),
                    "bed must be [x, y, z] in mm",
                );
            }
        }
        ps
    }
}

fn alias_hint(ty: &str) -> Option<String> {
    let t = ty.to_lowercase();
    let s = match t.as_str() {
        "cube" | "cuboid" | "block" | "rect" | "rectangle" | "plate" => {
            "Use \"box\" with \"size\": [x, y, z]."
        }
        "cone" | "frustum" => "Use \"cylinder\" with \"d\" and \"d_top\" (d_top: 0 for a point).",
        "ring" | "pipe" | "hollow_cylinder" => {
            "Use \"tube\" with \"d\", \"wall\" (or \"d_inner\") and \"h\"."
        }
        "polygon" | "extrude" | "linear_extrude" | "extrusion" => {
            "Use \"prism\" with \"points\" and \"h\"."
        }
        "rotate_extrude" | "lathe" => "Use \"revolve\" with \"profile\": [[r, z], ...].",
        "tray" | "bin" | "container" | "enclosure" | "case" | "shell" => {
            "Use \"hollow_box\" with \"inner\" or \"outer\" size."
        }
        "hole" | "screw" | "bolt_hole" | "countersink" | "counterbore" => {
            "Use \"screw_hole\" for screws, or a \"cylinder\" in \"cut\" for plain holes."
        }
        "nut" | "hex" | "hexagon" => {
            "Use \"nut_trap\" for nut pockets, or \"prism\" for other hexagons."
        }
        "magnet" => "Use \"magnet_pocket\" in \"cut\".",
        "triangle" | "ramp" | "gusset" => "Use \"wedge\".",
        "stadium" | "oblong" | "capsule" => "Use \"slot\".",
        "donut" => "Use \"torus\".",
        "text" | "label" => "Text is not available yet; use \"prism\" shapes or leave it out.",
        "union" | "difference" | "intersection" | "group" => {
            "Booleans are expressed with the part's \"add\" and \"cut\" lists."
        }
        _ => return None,
    };
    Some(s.to_string())
}

fn component_names() -> String {
    spec::COMPONENTS
        .iter()
        .map(|c| c.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Check references between parts on the raw JSON, so a wrong part id is
/// reported together with any other errors instead of in a later round.
fn check_part_refs(p: &mut P, parts: Option<&Json>) {
    let Some(Json::Array(items)) = parts else {
        return;
    };
    let ids: Vec<&str> = items
        .iter()
        .filter_map(|it| it.get("id").and_then(|i| i.as_str()))
        .collect();
    let check = |p: &mut P, id: &str, path: String, what: &str| {
        if ids.contains(&id) {
            return;
        }
        let hint = suggest(id, ids.iter().copied())
            .map(|s| format!("Did you mean \"{s}\"?"))
            .unwrap_or_else(|| format!("Part ids: {}.", ids.join(", ")));
        p.err(
            "unknown_ref",
            &path,
            format!("{what} \"{id}\" is not a part id"),
        )
        .hint = Some(hint);
    };
    for (i, it) in items.iter().enumerate() {
        let shape = it.get("shape");
        if shape.and_then(|s| s.get("type")).and_then(|t| t.as_str()) == Some("lid") {
            if let Some(f) = shape.and_then(|s| s.get("fits")).and_then(|f| f.as_str()) {
                check(p, f, format!("parts[{i}].shape.fits"), "lid fits");
            }
        }
        if let Some(on) = it
            .get("place")
            .and_then(|pl| pl.get("on"))
            .and_then(|o| o.as_str())
        {
            if let Some((pid, _)) = on.split_once('.') {
                check(p, pid, format!("parts[{i}].place.on"), "place.on refers to");
            }
        }
    }
}

pub fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else if key.starts_with('[') {
        format!("{path}{key}")
    } else {
        format!("{path}.{key}")
    }
}

/// Parse a design. Returns the design (if structurally usable) and all issues found.
pub fn parse(json: &Json) -> (Option<Design>, Vec<Issue>) {
    let mut p = P { issues: Vec::new() };
    let Some(root) = p.obj(json, "", "the design") else {
        return (None, p.issues);
    };
    p.check_keys(
        root,
        &[
            "format", "meta", "params", "derived", "parts", "splits", "print",
        ],
        "",
        "the design",
    );
    match root.get("format") {
        Some(Json::String(f)) if f == FORMAT => {}
        Some(Json::String(f)) => {
            p.err(
                "bad_format",
                "format",
                format!("unsupported format \"{f}\""),
            )
            .hint = Some(format!("Use \"format\": \"{FORMAT}\"."));
        }
        _ => {
            p.err("missing_field", "format", "missing \"format\"").hint =
                Some(format!("Add \"format\": \"{FORMAT}\" at the top."));
        }
    }
    let meta = root.get("meta").cloned().unwrap_or(Json::Null);
    let mut name = String::from("Untitled design");
    match &meta {
        Json::Object(m) => {
            p.check_keys(
                m,
                &[
                    "name",
                    "description",
                    "category",
                    "tags",
                    "license",
                    "author",
                    "skills",
                    "version",
                ],
                "meta",
                "meta",
            );
            match m.get("name") {
                Some(Json::String(n)) => name = n.clone(),
                _ => {
                    p.warn("missing_field", "meta.name", "design has no name");
                }
            }
        }
        Json::Null => {
            p.warn(
                "missing_field",
                "meta",
                "add \"meta\": {\"name\": ..., \"description\": ...}",
            );
        }
        other => {
            p.err(
                "wrong_type",
                "meta",
                format!("meta must be an object, got {}", json_type(other)),
            );
        }
    }

    let mut params = Vec::new();
    match root.get("params") {
        Some(Json::Object(m)) => {
            for (k, v) in m {
                if is_comment_key(k) {
                    continue;
                }
                if let Some(d) = p.param(k, v, &format!("params.{k}")) {
                    params.push(d);
                }
            }
        }
        Some(other) => {
            p.err(
                "wrong_type",
                "params",
                format!(
                    "params must be an object {{\"name\": {{...}}}}, got {}",
                    json_type(other)
                ),
            );
        }
        None => {}
    }

    let mut derived = Vec::new();
    match root.get("derived") {
        Some(Json::Object(m)) => {
            for (k, v) in m {
                if is_comment_key(k) {
                    continue;
                }
                let path = format!("derived.{k}");
                if !is_identifier(k) || RESERVED.contains(&k.as_str()) {
                    p.err("bad_name", &path, format!("\"{k}\" is not a valid name"));
                    continue;
                }
                if params.iter().any(|pd: &ParamDef| pd.name == *k) {
                    p.err(
                        "duplicate_name",
                        &path,
                        format!("\"{k}\" is already a param"),
                    );
                    continue;
                }
                if let Some(val) = p.val(v, &path) {
                    derived.push((k.clone(), val, path));
                }
            }
        }
        Some(other) => {
            p.err(
                "wrong_type",
                "derived",
                format!(
                    "derived must be an object {{\"name\": \"expression\"}}, got {}",
                    json_type(other)
                ),
            );
        }
        None => {}
    }

    let mut parts = Vec::new();
    match root.get("parts") {
        Some(Json::Array(items)) if !items.is_empty() => {
            for (i, it) in items.iter().enumerate() {
                if let Some(part) = p.part(it, &format!("parts[{i}]")) {
                    parts.push(part);
                }
            }
        }
        Some(Json::Array(_)) => {
            p.err("missing_field", "parts", "parts is empty");
        }
        Some(other) => {
            p.err(
                "wrong_type",
                "parts",
                format!("parts must be an array, got {}", json_type(other)),
            );
        }
        None => {
            p.err("missing_field", "parts", "design has no \"parts\"");
        }
    }
    check_part_refs(&mut p, root.get("parts"));
    let mut seen = std::collections::HashSet::new();
    for part in &parts {
        if !seen.insert(part.id.clone()) {
            p.err(
                "duplicate_id",
                &part.path,
                format!("part id \"{}\" is used twice", part.id),
            );
        }
        if params.iter().any(|pd| pd.name == part.id) {
            p.warn(
                "name_clash",
                &part.path,
                format!(
                    "part id \"{}\" is also a param name; this is allowed but confusing",
                    part.id
                ),
            );
        }
    }

    let mut splits = Vec::new();
    match root.get("splits") {
        Some(Json::Array(items)) => {
            for (i, it) in items.iter().enumerate() {
                if let Some(s) = p.split(it, &format!("splits[{i}]")) {
                    splits.push(s);
                }
            }
        }
        Some(other) => {
            p.err(
                "wrong_type",
                "splits",
                format!("splits must be an array, got {}", json_type(other)),
            );
        }
        None => {}
    }
    for s in &splits {
        for n in &s.names {
            if !seen.insert(n.clone()) {
                p.err(
                    "duplicate_id",
                    &join(&s.path, "names"),
                    format!("piece name \"{n}\" is already used by another part or split"),
                );
            }
        }
    }

    let print = match root.get("print") {
        Some(v) => p.print(v, "print"),
        None => PrintSettings::default(),
    };

    let has_errors = p.issues.iter().any(|i| i.is_error());
    let design = (!has_errors).then(|| Design {
        name,
        meta,
        params,
        derived,
        parts,
        splits,
        print,
    });
    (design, p.issues)
}

/// Parse from text, reporting JSON syntax errors as issues.
pub fn parse_str(src: &str) -> (Option<Design>, Vec<Issue>, Option<Json>) {
    match serde_json::from_str::<Json>(src) {
        Ok(json) => {
            let (d, i) = parse(&json);
            (d, i, Some(json))
        }
        Err(e) => (
            None,
            vec![Issue::error("json_syntax", "", format!("not valid JSON: {e}")).hint(
                "Send plain JSON: double-quoted keys, no comments, no trailing commas. Use \"_comment\" keys for notes.",
            )],
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_minimal_design() {
        let j = json!({
            "format": "pd/0.1",
            "meta": {"name": "t"},
            "params": {"w": {"value": 10, "min": 1, "max": 20}},
            "parts": [{"id": "a", "shape": {"type": "box", "size": ["w", 10, 5]}}]
        });
        let (d, issues) = parse(&j);
        assert!(issues.is_empty(), "{issues:?}");
        let d = d.unwrap();
        assert_eq!(d.parts.len(), 1);
        assert_eq!(d.params[0].name, "w");
    }

    #[test]
    fn reports_typos_with_suggestions() {
        let j = json!({
            "format": "pd/0.1",
            "meta": {"name": "t"},
            "parts": [{"id": "a", "shape": {"type": "cube", "siez": 3}},
                      {"id": "b", "shape": {"type": "box", "siez": 3}}]
        });
        let (d, issues) = parse(&j);
        assert!(d.is_none());
        let cube = issues.iter().find(|i| i.code == "unknown_type").unwrap();
        assert!(cube.hint.as_ref().unwrap().contains("box"));
        let typo = issues.iter().find(|i| i.code == "unknown_field").unwrap();
        assert_eq!(typo.path, "parts[1].shape.siez");
        assert!(typo.hint.as_ref().unwrap().contains("size"));
        assert!(issues
            .iter()
            .any(|i| i.code == "missing_field" && i.path == "parts[1].shape"));
    }

    #[test]
    fn colors() {
        assert_eq!(literal_color("#F80"), Some("#ff8800".into()));
        assert_eq!(literal_color("Orange"), Some("#f77f00".into()));
        assert_eq!(literal_color("wall"), None);
    }
}
