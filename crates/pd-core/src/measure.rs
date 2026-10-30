//! Measurement queries against a built model (used by the MCP `measure` tool
//! and the viewer).

use crate::build::{fmt_point, Model};
use crate::checks::min_thickness;
use crate::expr::fmt_num;
use crate::math::{Bbox, V3};
use crate::mesh::Mesh;
use manifold_rust::manifold::Manifold;
use serde_json::{json, Value as Json};

const PLA_DENSITY: f64 = 1.24; // g/cm³

fn r(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn rv(v: V3) -> [f64; 3] {
    v.map(r)
}

fn solid_of<'a>(model: &'a Model, id: Option<&str>) -> Result<(String, Manifold), String> {
    match id {
        None | Some("all") | Some("*") => {
            let all = model
                .parts
                .iter()
                .map(|p| p.solid.clone())
                .reduce(|a, b| a.union(&b))
                .ok_or("the model has no parts")?;
            Ok(("all".into(), all))
        }
        Some(id) => model
            .part(id)
            .map(|p| (id.to_string(), p.solid.clone()))
            .ok_or_else(|| {
                format!(
                    "no part \"{id}\"; parts are: {}",
                    model
                        .parts
                        .iter()
                        .map(|p| p.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }),
    }
}

fn bbox_json(b: &Bbox) -> Json {
    json!({ "size": rv(b.size()), "min": rv(b.min), "max": rv(b.max), "center": rv(b.center()) })
}

pub fn measure_one(model: &Model, q: &Json) -> Json {
    let ty = q.get("type").and_then(|t| t.as_str()).unwrap_or("bbox");
    let s = |k: &str| q.get(k).and_then(|v| v.as_str());
    let res: Result<Json, String> = (|| {
        match ty {
        "bbox" | "size" => {
            let (id, m) = solid_of(model, s("part"))?;
            Ok(json!({ "part": id, "bbox": bbox_json(&Bbox::of(&m)) }))
        }
        "volume" | "mass" | "weight" => {
            let (id, m) = solid_of(model, s("part"))?;
            let v = m.volume();
            Ok(json!({ "part": id, "volume_mm3": r(v), "pla_grams_solid": r(v / 1000.0 * PLA_DENSITY), "surface_mm2": r(m.surface_area()) }))
        }
        "distance" | "gap" | "clearance" => {
            let (a, ma) = solid_of(model, s("a"))?;
            let (b, mb) = solid_of(model, s("b"))?;
            let overlap = ma.intersection(&mb).volume();
            if overlap > 1e-3 {
                return Ok(json!({ "a": a, "b": b, "gap": 0.0, "overlap_mm3": r(overlap) }));
            }
            let g = ma.min_gap(&mb, 100.0);
            Ok(json!({ "a": a, "b": b, "gap": r(g), "note": if g >= 100.0 { "gap is 100 mm or more" } else { "smallest distance between the two surfaces" } }))
        }
        "anchor" => {
            let rf = s("ref").ok_or("anchor query needs \"ref\": \"part.anchor\"")?;
            let (pid, rest) = rf.split_once('.').ok_or("ref must look like \"box.top\"")?;
            let p = model.part(pid).ok_or(format!("no part \"{pid}\""))?;
            let (set, a) = match rest.split_once('.') {
                Some((fid, a)) => (p.features.get(fid).ok_or(format!("no feature \"{fid}\" in \"{pid}\""))?, a),
                None => (&p.anchors, rest),
            };
            let an = set.get(a).ok_or(format!("\"{a}\" is not an anchor of \"{pid}\""))?;
            Ok(json!({ "ref": rf, "position": rv(an.pos), "normal": rv(an.normal) }))
        }
        "thickness" | "wall" => {
            let (id, m) = solid_of(model, s("part"))?;
            match min_thickness(&Mesh::of(&m), &m) {
                Some((d, at)) => Ok(json!({ "part": id, "min_wall": r(d), "at": rv(at), "note": "sampled; thinnest wall found by rays cast inward from the surface" })),
                None => Ok(json!({ "part": id, "min_wall": null, "note": "no wall thinner than 20 mm found" })),
            }
        }
        "section" => {
            let (id, m) = solid_of(model, s("part"))?;
            let axis = s("axis").unwrap_or("z");
            let at = q.get("at").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let rot = match axis {
                "x" => m.rotate(0.0, -90.0, 0.0),
                "y" => m.rotate(90.0, 0.0, 0.0),
                _ => m.clone(),
            };
            let h = if axis == "y" { -at } else { at };
            let cs = rot.slice(h);
            let b = cs.bounds();
            Ok(json!({
                "part": id, "axis": axis, "at": at, "area_mm2": r(cs.area()), "contours": cs.num_contour(),
                "extent_2d": if cs.is_empty() { json!(null) } else { json!({"min": [r(b.min.x), r(b.min.y)], "max": [r(b.max.x), r(b.max.y)]}) },
                "note": "extent_2d is in the cut plane: z-cut -> (x, y); x-cut -> (z, y) as rotated; y-cut -> (x, z)"
            }))
        }
        "params" => Ok(json!(model.env.iter().map(|(k, v)| (k.clone(), v.to_json())).collect::<serde_json::Map<_, _>>())),
        other => Err(format!("unknown measure type \"{other}\"; use bbox, volume, distance, anchor, thickness, section or params")),
    }
    })();
    match res {
        Ok(v) => v,
        Err(e) => json!({ "query": q, "error": e }),
    }
}

/// Run queries; with none, report every part's box and the gaps between parts.
pub fn measure(model: &Model, queries: &[Json]) -> Vec<Json> {
    if !queries.is_empty() {
        return queries.iter().map(|q| measure_one(model, q)).collect();
    }
    let mut out: Vec<Json> = model
        .parts
        .iter()
        .map(|p| measure_one(model, &json!({"type": "bbox", "part": p.id})))
        .collect();
    for i in 0..model.parts.len() {
        for j in i + 1..model.parts.len() {
            out.push(measure_one(
                model,
                &json!({"type": "distance", "a": model.parts[i].id, "b": model.parts[j].id}),
            ));
        }
    }
    let _ = fmt_point;
    let _ = fmt_num;
    out
}
