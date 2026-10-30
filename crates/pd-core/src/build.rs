//! Design + parameter values -> placed solids for every part.

use crate::components::{self, ShapeCtx, Solid};
use crate::design::{literal_color, Design, FeatureDef, PartDef, PlaceDef, RepeatDef, ShapeDef};
use crate::eval::{build_env, eval_bool, eval_num, eval_val, eval_vec3, Overrides};
use crate::expr::{fmt_num, Env, Value};
use crate::issues::{suggest, Issue};
use crate::math::*;
use manifold_rust::manifold::Manifold;
use std::collections::HashMap;

pub const PALETTE: &[&str] = &[
    "#f77f00", "#1d6fd8", "#2a9d48", "#d62828", "#7b2cbf", "#fcbf49", "#20c4d8", "#8b5a2b",
];

/// Anchors of one shape instance: its bounding box plus named extras.
#[derive(Clone, Debug)]
pub struct AnchorSet {
    pub bbox: Bbox,
    pub extra: Vec<(String, Anchor)>,
}

impl AnchorSet {
    pub fn get(&self, name: &str) -> Option<Anchor> {
        if let Some((_, a)) = self.extra.iter().find(|(n, _)| n == name) {
            return Some(*a);
        }
        let toks = anchor_tokens(name)?;
        Some(bbox_anchor(&self.bbox, &toks))
    }

    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = ["top", "bottom", "left", "right", "front", "back", "center"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        v.extend(self.extra.iter().map(|(n, _)| n.clone()));
        v
    }

    /// Transform by `xf` (bbox via its 8 corners; exact for 90° rotations).
    pub fn transformed(&self, xf: &Xf) -> AnchorSet {
        let b = &self.bbox;
        let mut nb = Bbox {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        };
        for i in 0..8 {
            let c = [
                if i & 1 == 0 { b.min[0] } else { b.max[0] },
                if i & 2 == 0 { b.min[1] } else { b.max[1] },
                if i & 4 == 0 { b.min[2] } else { b.max[2] },
            ];
            let p = xf.point(c);
            for k in 0..3 {
                nb.min[k] = nb.min[k].min(p[k]);
                nb.max[k] = nb.max[k].max(p[k]);
            }
        }
        AnchorSet {
            bbox: nb,
            extra: self
                .extra
                .iter()
                .map(|(n, a)| (n.clone(), a.transformed(xf)))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PartOrigin {
    Design,
    SplitPiece,
    Pin,
}

#[derive(Clone)]
pub struct BuiltPart {
    pub id: String,
    pub name: Option<String>,
    pub color: String,
    /// Solid in world (assembled) coordinates.
    pub solid: Manifold,
    pub print_rotate: [f64; 3],
    pub path: String,
    pub allow_disconnected: bool,
    pub anchors: AnchorSet,
    pub features: HashMap<String, AnchorSet>,
    pub origin: PartOrigin,
}

pub struct Model {
    pub name: String,
    pub parts: Vec<BuiltPart>,
    pub env: Env,
    pub issues: Vec<Issue>,
}

impl Model {
    pub fn part(&self, id: &str) -> Option<&BuiltPart> {
        self.parts.iter().find(|p| p.id == id)
    }
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.is_error())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Part,
    Add,
    Cut,
}

struct Placed {
    xf: Xf,
    /// Placement point (target + offset) and local axes there.
    point: V3,
    ux: V3,
    uy: V3,
    normal: V3,
}

/// Default own-anchor name for a placement onto `target_name`.
fn default_own_anchor(mode: Mode, orient: bool, target_name: &str, normal: V3) -> String {
    let last = target_name.rsplit('.').next().unwrap_or(target_name);
    match (mode, orient) {
        (Mode::Add, true) => "bottom".into(),
        (Mode::Cut, true) => "top".into(),
        _ => {
            let toks = anchor_tokens(last).filter(|t| !t.is_empty());
            let first = match &toks {
                Some(t) => t[0],
                None => match token_for_normal(normal) {
                    Some(t) => t,
                    None => return "center".into(),
                },
            };
            let lead = if mode == Mode::Cut {
                first
            } else {
                opposite_token(first)
            };
            let mut out = vec![lead];
            if mode == Mode::Part {
                if let Some(t) = &toks {
                    out.extend(t.iter().skip(1).copied());
                }
            }
            out.join("-")
        }
    }
}

fn own_anchor(
    solid: &Solid,
    rotated: &Manifold,
    r: &Xf,
    name: &str,
    path: &str,
) -> Result<Anchor, Issue> {
    if let Some((_, a)) = solid.anchors.iter().find(|(n, _)| n == name) {
        return Ok(a.transformed(r));
    }
    match anchor_tokens(name) {
        Some(toks) => Ok(bbox_anchor(&Bbox::of(rotated), &toks)),
        None => {
            let mut names: Vec<&str> =
                vec!["top", "bottom", "left", "right", "front", "back", "center"];
            names.extend(solid.anchors.iter().map(|(n, _)| n.as_str()));
            Err(Issue::error(
                "unknown_anchor",
                format!("{path}.anchor"),
                format!("\"{name}\" is not an anchor of this shape"),
            )
            .hint_opt(
                suggest(name, names.iter().copied())
                    .map(|s| format!("Did you mean \"{s}\"?"))
                    .or(Some(format!("Anchors: {}", names.join(", ")))),
            ))
        }
    }
}

fn place(
    solid: &Solid,
    place: Option<&PlaceDef>,
    mode: Mode,
    target: Option<(Anchor, &str)>,
    env: &Env,
) -> Result<Placed, Issue> {
    let Some(pl) = place else {
        return Ok(Placed {
            xf: Xf::IDENTITY,
            point: [0.0; 3],
            ux: [1.0, 0.0, 0.0],
            uy: [0.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        });
    };
    let r = match &pl.rotate {
        Some(rv) => Xf::rotate_xyz(eval_vec3(rv, env, &format!("{}.rotate", pl.path))?),
        None => Xf::IDENTITY,
    };
    let offset = match &pl.offset {
        Some(o) => eval_vec3(o, env, &format!("{}.offset", pl.path))?,
        None => [0.0; 3],
    };
    let rotated = r.apply(&solid.m);
    if let Some((ta, tname)) = target {
        let orient = pl.orient.unwrap_or(mode != Mode::Part);
        let o = if orient {
            orient_to(ta.normal)
        } else {
            Xf::IDENTITY
        };
        let own_name = pl
            .anchor
            .clone()
            .unwrap_or_else(|| default_own_anchor(mode, orient, tname, ta.normal));
        let own = own_anchor(solid, &rotated, &r, &own_name, &pl.path)?;
        let point = add(ta.pos, offset);
        let mut t = sub(point, o.point(own.pos));
        if mode == Mode::Cut {
            t = add(t, scale(ta.normal, 0.01));
        }
        let xf = Xf::translate(t).then_after(&o).then_after(&r);
        return Ok(Placed {
            xf,
            point,
            ux: o.dir([1.0, 0.0, 0.0]),
            uy: o.dir([0.0, 1.0, 0.0]),
            normal: ta.normal,
        });
    }
    let at = match &pl.at {
        Some(a) => eval_vec3(a, env, &format!("{}.at", pl.path))?,
        None => [0.0; 3],
    };
    let point = add(at, offset);
    let own = match &pl.anchor {
        Some(n) => own_anchor(solid, &rotated, &r, n, &pl.path)?.pos,
        None => [0.0; 3],
    };
    let xf = Xf::translate(sub(point, own)).then_after(&r);
    Ok(Placed {
        xf,
        point,
        ux: [1.0, 0.0, 0.0],
        uy: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    })
}

fn count(v: &crate::design::Val, env: &Env, path: &str, max: i64) -> Result<usize, Issue> {
    let n = eval_num(v, env, path)?.round() as i64;
    if n < 1 || n > max {
        return Err(Issue::error(
            "bad_value",
            path,
            format!("count {n} must be between 1 and {max}"),
        ));
    }
    Ok(n as usize)
}

/// Instance transforms for repeat + mirror. `Some(normal)` = also mirror.
fn instances(f: &FeatureDef, p: &Placed, env: &Env) -> Result<Vec<(Xf, Option<V3>)>, Issue> {
    let rp = format!("{}.repeat", f.path);
    let mut xfs = vec![];
    match &f.repeat {
        None => xfs.push(p.xf),
        Some(RepeatDef::Grid { n, spacing }) => {
            let nx = count(&n[0], env, &format!("{rp}.grid[0]"), 200)?;
            let ny = count(&n[1], env, &format!("{rp}.grid[1]"), 200)?;
            let sx = eval_num(&spacing[0], env, &format!("{rp}.spacing[0]"))?;
            let sy = eval_num(&spacing[1], env, &format!("{rp}.spacing[1]"))?;
            if nx * ny > 2000 {
                return Err(Issue::error(
                    "too_many",
                    &rp,
                    "grid makes more than 2000 copies",
                ));
            }
            for i in 0..nx {
                for j in 0..ny {
                    let d = add(
                        scale(p.ux, (i as f64 - (nx - 1) as f64 / 2.0) * sx),
                        scale(p.uy, (j as f64 - (ny - 1) as f64 / 2.0) * sy),
                    );
                    xfs.push(Xf::translate(d).then_after(&p.xf));
                }
            }
        }
        Some(RepeatDef::Linear { n, step, center }) => {
            let n = count(n, env, &format!("{rp}.linear"), 2000)?;
            let st = eval_vec3(step, env, &format!("{rp}.step"))?;
            let shift = if *center { (n - 1) as f64 / 2.0 } else { 0.0 };
            for k in 0..n {
                xfs.push(Xf::translate(scale(st, k as f64 - shift)).then_after(&p.xf));
            }
        }
        Some(RepeatDef::Radial {
            n,
            radius,
            start_angle,
            rotate,
        }) => {
            let n = count(n, env, &format!("{rp}.radial"), 2000)?;
            let rad = eval_num(radius, env, &format!("{rp}.radius"))?;
            let start = eval_num(start_angle, env, &format!("{rp}.start_angle"))?;
            for k in 0..n {
                let a = start + 360.0 * k as f64 / n as f64;
                if *rotate {
                    let about = Xf::translate(p.point)
                        .then_after(&Xf::rotate_axis(p.normal, a))
                        .then_after(&Xf::translate(scale(p.point, -1.0)));
                    xfs.push(
                        about
                            .then_after(&Xf::translate(scale(p.ux, rad)))
                            .then_after(&p.xf),
                    );
                } else {
                    let (s, c) = a.to_radians().sin_cos();
                    xfs.push(
                        Xf::translate(add(scale(p.ux, rad * c), scale(p.uy, rad * s)))
                            .then_after(&p.xf),
                    );
                }
            }
        }
    }
    let mut out: Vec<(Xf, Option<V3>)> = xfs.iter().map(|x| (*x, None)).collect();
    if let Some(m) = &f.mirror {
        let normals: Vec<V3> = match m.as_str() {
            "x" => vec![[1.0, 0.0, 0.0]],
            "y" => vec![[0.0, 1.0, 0.0]],
            _ => vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        };
        let base = out.clone();
        for n in &normals {
            out.extend(base.iter().map(|(x, _)| (*x, Some(*n))));
        }
        if normals.len() == 2 {
            // the diagonal copy: mirrored across both planes = rotated 180° about Z
            let rz = Xf::rotate_axis([0.0, 0.0, 1.0], 180.0);
            out.extend(base.iter().map(|(x, _)| (rz.then_after(x), None)));
        }
    }
    Ok(out)
}

struct LocalPart {
    solid: Manifold,
    base: Solid,
    base_anchors: AnchorSet,
    features: HashMap<String, AnchorSet>,
}

fn resolve_local(
    name: &str,
    base: &AnchorSet,
    feats: &HashMap<String, AnchorSet>,
    path: &str,
) -> Result<Anchor, Issue> {
    let (scope, anchor) = match name.split_once('.') {
        Some((s, a)) => (Some(s), a),
        None => (None, name),
    };
    let set = match scope {
        None | Some("base") | Some("self") => base,
        Some(fid) => match feats.get(fid) {
            Some(s) => s,
            None => {
                let mut ids: Vec<&str> = feats.keys().map(|s| s.as_str()).collect();
                ids.push("base");
                return Err(Issue::error("unknown_ref", path, format!("\"{fid}\" is not an earlier feature of this part"))
                    .hint(match suggest(fid, ids.iter().copied()) {
                        Some(s) => format!("Did you mean \"{s}\"? Features can refer to the base shape (\"top\", \"base.top\") or to an earlier \"add\" feature with an id."),
                        None => "Features can only refer to their own part: the base shape (\"top\", \"base.inside_bottom\") or an earlier \"add\" feature with an id. Use offset for exact positions.".into(),
                    }));
            }
        },
    };
    set.get(anchor).ok_or_else(|| {
        let names = set.names();
        Issue::error(
            "unknown_anchor",
            path,
            format!("\"{anchor}\" is not an anchor"),
        )
        .hint_opt(
            suggest(anchor, names.iter().map(|s| s.as_str()))
                .map(|s| format!("Did you mean \"{s}\"?"))
                .or(Some(format!(
                    "Anchors: {}, or combinations like top-left.",
                    names.join(", ")
                ))),
        )
    })
}

fn build_local(
    part: &PartDef,
    env: &Env,
    shapes: &HashMap<String, &ShapeDef>,
    warnings: &mut Vec<Issue>,
) -> Result<(LocalPart, Vec<(String, Manifold)>), Issue> {
    let lookup = |id: &str| shapes.get(id).copied();
    let cx = ShapeCtx {
        through_depth: None,
        part_shape: &lookup,
    };
    let base = components::build(&part.shape, env, &cx)?;
    let base_anchors = AnchorSet {
        bbox: Bbox::of(&base.m),
        extra: base.anchors.clone(),
    };
    let mut foreign = base.foreign_cuts.clone();
    let mut solid = base.m.clone();
    let mut features: HashMap<String, AnchorSet> = HashMap::new();

    for (mode, list) in [(Mode::Add, &part.add), (Mode::Cut, &part.cut)] {
        for f in list {
            if let Some(e) = &f.enabled {
                if !eval_bool(e, env, &format!("{}.enabled", f.path))? {
                    continue;
                }
            }
            let target = match f.place.as_ref().and_then(|p| p.on.as_ref()) {
                Some(on) => Some((
                    resolve_local(
                        on,
                        &base_anchors,
                        &features,
                        &format!("{}.place.on", f.path),
                    )?,
                    on.as_str(),
                )),
                None => None,
            };
            let mut through_depth = None;
            if mode == Mode::Cut && f.shape.fields.contains_key("through") {
                let fx = crate::eval::Fields::of(&f.shape, env);
                if fx.bool("through", false)? {
                    if let Some((ta, _)) = target {
                        through_depth = Some(material_depth(&solid, ta).ok_or_else(|| {
                            Issue::error("no_material", &f.path, format!("there is no material under {} to drill through", fmt_point(ta.pos)))
                                .hint("The anchor is over empty space (e.g. the open top of a hollow_box). Place the hole on a solid face, e.g. \"on\": \"inside_bottom\" or \"bottom\", or add an offset onto a wall.")
                        })?);
                    }
                }
            }
            let fcx = ShapeCtx {
                through_depth,
                part_shape: &lookup,
            };
            let fs = components::build(&f.shape, env, &fcx)?;
            foreign.extend(fs.foreign_cuts.iter().cloned());
            let p = place(&fs, f.place.as_ref(), mode, target, env)?;
            let mut combined: Option<Manifold> = None;
            for (xf, mirror) in instances(f, &p, env)? {
                let mut inst = xf.apply(&fs.m);
                if let Some(n) = mirror {
                    inst = inst.mirror(v3(n));
                }
                combined = Some(match combined {
                    Some(c) => c.union(&inst),
                    None => inst,
                });
            }
            let Some(geom) = combined else { continue };
            if let Some(id) = &f.id {
                let set = AnchorSet {
                    bbox: Bbox::of(&p.xf.apply(&fs.m)),
                    extra: fs
                        .anchors
                        .iter()
                        .map(|(n, a)| (n.clone(), a.transformed(&p.xf)))
                        .collect(),
                };
                features.insert(id.clone(), set);
            }
            let before = solid.volume();
            solid = if mode == Mode::Add {
                solid.union(&geom)
            } else {
                solid.difference(&geom)
            };
            let after = solid.volume();
            if mode == Mode::Cut && before - after < 1e-3 {
                warnings.push(
                    Issue::warning("cut_no_effect", &f.path, format!("this cut removes nothing; the cutter ends up at {} which is outside the material", fmt_bbox(&Bbox::of(&geom))))
                        .hint("Check place.on / offset. A cutter placed \"on\" a face sinks into that face; make it long enough (or use \"through\": true for screw holes)."),
                );
            }
            if mode == Mode::Add && after - before < 1e-3 {
                warnings.push(
                    Issue::warning(
                        "add_no_effect",
                        &f.path,
                        "this add is completely inside the part and changes nothing",
                    )
                    .hint("Check its placement and size."),
                );
            }
        }
    }
    Ok((
        LocalPart {
            solid,
            base,
            base_anchors,
            features,
        },
        foreign,
    ))
}

/// Thickness of material under an anchor, measured inward along -normal.
fn material_depth(solid: &Manifold, a: Anchor) -> Option<f64> {
    let len = 4000.0;
    let o = add(a.pos, scale(a.normal, 1.0));
    let e = sub(a.pos, scale(a.normal, len - 1.0));
    let mut hits = solid.ray_cast(v3(o), v3(e));
    hits.sort_by(|x, y| x.distance.partial_cmp(&y.distance).unwrap());
    if hits.len() < 2 {
        return None;
    }
    let entry = hits[0].distance * len;
    if (entry - 1.0).abs() > 0.05 {
        return None;
    }
    let exit = hits
        .iter()
        .skip(1)
        .find(|h| h.distance * len > entry + 1e-4)?;
    Some(exit.distance * len - entry + 0.5)
}

pub fn fmt_point(p: V3) -> String {
    format!(
        "[{}, {}, {}]",
        fmt_num((p[0] * 100.0).round() / 100.0),
        fmt_num((p[1] * 100.0).round() / 100.0),
        fmt_num((p[2] * 100.0).round() / 100.0)
    )
}

pub fn fmt_bbox(b: &Bbox) -> String {
    format!(
        "x {}..{}, y {}..{}, z {}..{}",
        r2(b.min[0]),
        r2(b.max[0]),
        r2(b.min[1]),
        r2(b.max[1]),
        r2(b.min[2]),
        r2(b.max[2])
    )
}

fn r2(v: f64) -> String {
    fmt_num((v * 100.0).round() / 100.0)
}

fn part_deps(p: &PartDef, lid_target: Option<&str>) -> Vec<String> {
    let mut deps = vec![];
    match p.place.as_ref().and_then(|pl| pl.on.as_ref()) {
        Some(on) => {
            if let Some((pid, _)) = on.split_once('.') {
                deps.push(pid.to_string());
            }
        }
        None => {
            if p.place.is_none() {
                if let Some(t) = lid_target {
                    deps.push(t.to_string());
                }
            }
        }
    }
    deps
}

/// Build every part at the given parameter values.
pub fn build(design: &Design, overrides: &Overrides) -> Model {
    let (env, mut issues) = build_env(design, overrides);
    let mut model = Model {
        name: design.name.clone(),
        parts: vec![],
        env: env.clone(),
        issues: vec![],
    };
    if issues.iter().any(|i| i.is_error()) {
        model.issues = issues;
        return model;
    }

    // Which parts are enabled.
    let mut enabled: Vec<&PartDef> = vec![];
    for p in &design.parts {
        match &p.enabled {
            Some(e) => match eval_bool(e, &env, &format!("{}.enabled", p.path)) {
                Ok(true) => enabled.push(p),
                Ok(false) => {}
                Err(e) => issues.push(e),
            },
            None => enabled.push(p),
        }
    }
    let shapes: HashMap<String, &ShapeDef> =
        enabled.iter().map(|p| (p.id.clone(), &p.shape)).collect();
    let all_ids: Vec<&str> = design.parts.iter().map(|p| p.id.as_str()).collect();

    // 1. local geometry
    let mut locals: HashMap<String, LocalPart> = HashMap::new();
    let mut foreign: Vec<(String, Manifold, String)> = vec![];
    for p in &enabled {
        match build_local(p, &env, &shapes, &mut issues) {
            Ok((lp, fc)) => {
                foreign.extend(fc.into_iter().map(|(t, m)| (t, m, p.path.clone())));
                locals.insert(p.id.clone(), lp);
            }
            Err(e) => issues.push(e),
        }
    }
    // 2. cuts components make in other parts (e.g. snap-lid grooves)
    for (target, cutter, from) in foreign {
        match locals.get_mut(&target) {
            Some(lp) => lp.solid = lp.solid.difference(&cutter),
            None => {
                if !shapes.contains_key(&target) {
                    issues.push(Issue::error(
                        "unknown_ref",
                        &from,
                        format!("refers to part \"{target}\" which does not exist or is disabled"),
                    ));
                }
            }
        }
    }

    // 3. placement in dependency order
    let lid_target = |p: &PartDef| -> Option<String> {
        if p.shape.ty == "lid" {
            crate::eval::Fields::of(&p.shape, &env).part_ref("fits")
        } else {
            None
        }
    };
    let mut order: Vec<&PartDef> = vec![];
    let mut state: HashMap<String, u8> = HashMap::new();
    fn visit<'a>(
        p: &'a PartDef,
        all: &[&'a PartDef],
        lid: &dyn Fn(&PartDef) -> Option<String>,
        state: &mut HashMap<String, u8>,
        order: &mut Vec<&'a PartDef>,
        issues: &mut Vec<Issue>,
    ) {
        match state.get(&p.id) {
            Some(2) => return,
            Some(1) => {
                issues.push(
                    Issue::error(
                        "placement_cycle",
                        &p.path,
                        format!("part \"{}\" is (indirectly) placed on itself", p.id),
                    )
                    .hint("Parts must be placed on parts that do not depend on them."),
                );
                return;
            }
            _ => {}
        }
        state.insert(p.id.clone(), 1);
        for d in part_deps(p, lid(p).as_deref()) {
            if let Some(dp) = all.iter().find(|q| q.id == d) {
                visit(dp, all, lid, state, order, issues);
            }
        }
        state.insert(p.id.clone(), 2);
        order.push(p);
    }
    for p in &enabled {
        visit(
            p,
            &enabled,
            &lid_target,
            &mut state,
            &mut order,
            &mut issues,
        );
    }

    let mut placed: Vec<BuiltPart> = vec![];
    for (idx, p) in order.iter().enumerate() {
        let Some(lp) = locals.get(&p.id) else {
            continue;
        };
        let auto_lid = if p.place.is_none() {
            lid_target(p)
        } else {
            None
        };
        let auto_place;
        let place_def: Option<&PlaceDef> = match (&p.place, &auto_lid) {
            (Some(pl), _) => Some(pl),
            (None, Some(t)) => {
                auto_place = PlaceDef {
                    on: Some(format!("{t}.top")),
                    at: None,
                    anchor: Some("seat".into()),
                    offset: None,
                    rotate: None,
                    orient: None,
                    path: format!("{}.place", p.path),
                };
                Some(&auto_place)
            }
            _ => None,
        };
        let target = match place_def.and_then(|pl| pl.on.as_ref()) {
            Some(on) => match resolve_world(
                on,
                &placed,
                &all_ids,
                &shapes,
                &format!("{}.place.on", p.path),
            ) {
                Ok(a) => Some((a, on.as_str())),
                Err(e) => {
                    issues.push(e);
                    continue;
                }
            },
            None => None,
        };
        let placed_xf = match place(&lp.base, place_def, Mode::Part, target, &env) {
            Ok(pl) => pl.xf,
            Err(e) => {
                issues.push(e);
                continue;
            }
        };
        let color = match &p.color {
            None => PALETTE[design
                .parts
                .iter()
                .position(|q| q.id == p.id)
                .unwrap_or(idx)
                % PALETTE.len()]
            .to_string(),
            Some(c) => match eval_val(c, &env, &format!("{}.color", p.path)) {
                Ok(Value::Str(s)) => match literal_color(&s) {
                    Some(c) => c,
                    None => {
                        issues.push(
                            Issue::error(
                                "bad_color",
                                format!("{}.color", p.path),
                                format!("\"{s}\" is not a color"),
                            )
                            .hint("Use \"#rrggbb\" or a color param."),
                        );
                        continue;
                    }
                },
                Ok(other) => {
                    issues.push(Issue::error(
                        "bad_color",
                        format!("{}.color", p.path),
                        format!("color evaluated to {other}, not a color"),
                    ));
                    continue;
                }
                Err(e) => {
                    issues.push(e);
                    continue;
                }
            },
        };
        let print_rotate = match &p.print_rotate {
            Some(r) => match eval_vec3(r, &env, &format!("{}.print_rotate", p.path)) {
                Ok(v) => v,
                Err(e) => {
                    issues.push(e);
                    continue;
                }
            },
            None => lp.base.print_rotate.unwrap_or([0.0; 3]),
        };
        placed.push(BuiltPart {
            id: p.id.clone(),
            name: p.name.clone(),
            color,
            solid: placed_xf.apply(&lp.solid),
            print_rotate,
            path: p.path.clone(),
            allow_disconnected: p.allow_disconnected,
            anchors: lp.base_anchors.transformed(&placed_xf),
            features: lp
                .features
                .iter()
                .map(|(k, v)| (k.clone(), v.transformed(&placed_xf)))
                .collect(),
            origin: PartOrigin::Design,
        });
    }
    // keep the design's part order
    placed.sort_by_key(|bp| {
        design
            .parts
            .iter()
            .position(|q| q.id == bp.id)
            .unwrap_or(usize::MAX)
    });

    // 4. splits + joints
    for s in &design.splits {
        crate::joints::apply_split(&mut placed, s, &env, &mut issues);
    }

    model.parts = placed;
    model.issues = issues;
    model
}

fn resolve_world(
    on: &str,
    placed: &[BuiltPart],
    all_ids: &[&str],
    shapes: &HashMap<String, &ShapeDef>,
    path: &str,
) -> Result<Anchor, Issue> {
    if matches!(on, "origin" | "bed" | "world") {
        return Ok(Anchor {
            pos: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
        });
    }
    let Some((pid, rest)) = on.split_once('.') else {
        return Err(Issue::error(
            "bad_ref",
            path,
            format!("\"{on}\" must name a part and an anchor, like \"box.top\""),
        )
        .hint_opt(
            suggest(on, all_ids.iter().copied()).map(|s| {
                format!("e.g. \"{s}.top\". Use \"at\": [x, y, z] for absolute positions.")
            }),
        ));
    };
    let Some(bp) = placed.iter().find(|b| b.id == pid) else {
        if all_ids.contains(&pid) && !shapes.contains_key(pid) {
            return Err(Issue::error("unknown_ref", path, format!("part \"{pid}\" is disabled (its \"enabled\" is false), so nothing can be placed on it")));
        }
        if all_ids.contains(&pid) {
            return Err(Issue::error(
                "unknown_ref",
                path,
                format!("part \"{pid}\" could not be built, so nothing can be placed on it"),
            )
            .hint("Fix the errors reported for that part first."));
        }
        return Err(
            Issue::error("unknown_ref", path, format!("no part with id \"{pid}\"")).hint_opt(
                suggest(pid, all_ids.iter().copied()).map(|s| format!("Did you mean \"{s}\"?")),
            ),
        );
    };
    let (set, anchor) = match rest.split_once('.') {
        Some((fid, a)) => match bp.features.get(fid) {
            Some(s) => (s, a),
            None => {
                return Err(Issue::error(
                    "unknown_ref",
                    path,
                    format!("part \"{pid}\" has no feature with id \"{fid}\""),
                ))
            }
        },
        None => (&bp.anchors, rest),
    };
    set.get(anchor).ok_or_else(|| {
        let names = set.names();
        Issue::error(
            "unknown_anchor",
            path,
            format!("\"{anchor}\" is not an anchor of \"{pid}\""),
        )
        .hint_opt(
            suggest(anchor, names.iter().map(|s| s.as_str()))
                .map(|s| format!("Did you mean \"{s}\"?"))
                .or(Some(format!("Anchors: {}", names.join(", ")))),
        )
    })
}
