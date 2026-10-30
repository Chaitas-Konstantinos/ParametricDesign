//! Printability and assembly checks on a built model.

use crate::build::{fmt_bbox, fmt_point, BuiltPart, Model};
use crate::expr::fmt_num;
use crate::issues::Issue;
use crate::math::*;
use crate::mesh::{print_pose, Mesh};
use manifold_rust::manifold::Manifold;

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub nozzle: f64,
    pub bed: [f64; 3],
    pub supports_ok: bool,
    /// Also run the slower mesh-sampling checks (thin walls, overhangs).
    pub thorough: bool,
}

impl Default for CheckOptions {
    fn default() -> Self {
        CheckOptions {
            nozzle: 0.4,
            bed: [256.0, 256.0, 256.0],
            supports_ok: false,
            thorough: true,
        }
    }
}

pub fn check(model: &Model, o: &CheckOptions) -> Vec<Issue> {
    let mut out = vec![];
    for p in &model.parts {
        check_part(p, o, &mut out);
    }
    // parts must not interpenetrate when assembled
    for i in 0..model.parts.len() {
        for j in i + 1..model.parts.len() {
            let (a, b) = (&model.parts[i], &model.parts[j]);
            if a.solid.is_empty()
                || b.solid.is_empty()
                || !Bbox::of(&a.solid).overlaps(&Bbox::of(&b.solid), 1e-3)
            {
                continue;
            }
            let x = a.solid.intersection(&b.solid);
            let v = x.volume();
            if v > 0.5 {
                out.push(
                    Issue::error("parts_overlap", &b.path, format!("parts \"{}\" and \"{}\" overlap by {} mm³ around {}", a.id, b.id, fmt_num(v.round()), fmt_bbox(&Bbox::of(&x))))
                        .hint("Separate parts must not intersect when assembled: add clearance or fix the placement. If they are really one object, make one an \"add\" feature of the other."),
                );
            }
        }
    }
    out
}

fn check_part(p: &BuiltPart, o: &CheckOptions, out: &mut Vec<Issue>) {
    if p.solid.is_empty() {
        out.push(
            Issue::error(
                "empty_part",
                &p.path,
                format!("part \"{}\" has no geometry left", p.id),
            )
            .hint("A cut may be removing everything, or sizes collapse to zero."),
        );
        return;
    }
    let pieces = p.solid.decompose();
    if pieces.len() > 1 && !p.allow_disconnected {
        let mut ps: Vec<(f64, Bbox)> = pieces.iter().map(|m| (m.volume(), Bbox::of(m))).collect();
        ps.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let loose: Vec<String> = ps
            .iter()
            .skip(1)
            .take(3)
            .map(|(v, b)| {
                format!(
                    "{} mm³ at {}",
                    fmt_num(v.round().max(0.0)),
                    fmt_point(b.center())
                )
            })
            .collect();
        out.push(
            Issue::error("disconnected", &p.path, format!("part \"{}\" falls apart into {} separate pieces; loose: {}", p.id, pieces.len(), loose.join("; ")))
                .hint("An added feature does not touch the body (check its place / offset), or a cut splits the part. If separate pieces are intended, make them separate parts or set \"allow_disconnected\": true."),
        );
    }
    let vol = p.solid.volume();
    if vol < 1.0 {
        out.push(Issue::warning(
            "tiny_part",
            &p.path,
            format!("part \"{}\" is tiny ({} mm³)", p.id, fmt_num(vol)),
        ));
    }
    if !o.thorough {
        return;
    }
    let posed = print_pose(p);
    let b = Bbox::of(&posed).size();
    if b[0] > o.bed[0] || b[1] > o.bed[1] || b[2] > o.bed[2] {
        let fits_rotated = b[0] <= o.bed[1] && b[1] <= o.bed[0] && b[2] <= o.bed[2];
        if !fits_rotated {
            out.push(
                Issue::warning("too_big_for_bed", &p.path, format!("part \"{}\" is {} x {} x {} mm as printed; the bed is {} x {} x {} mm", p.id, r1(b[0]), r1(b[1]), r1(b[2]), r1(o.bed[0]), r1(o.bed[1]), r1(o.bed[2])))
                    .hint("Split it into pieces with \"splits\" (see the multi-part skill), or change print_rotate."),
            );
        }
    }
    let mesh = Mesh::of(&posed);
    thin_walls(p, &mesh, &posed, o, out);
    overhangs(p, &mesh, o, out);
}

fn r1(v: f64) -> String {
    fmt_num((v * 10.0).round() / 10.0)
}

fn tri(mesh: &Mesh, t: usize) -> (V3, V3, V3) {
    let g = |i: u32| {
        let k = i as usize * 3;
        [
            mesh.positions[k] as f64,
            mesh.positions[k + 1] as f64,
            mesh.positions[k + 2] as f64,
        ]
    };
    (
        g(mesh.indices[t * 3]),
        g(mesh.indices[t * 3 + 1]),
        g(mesh.indices[t * 3 + 2]),
    )
}

/// Thinnest wall found by casting rays inward from a sample of surface triangles.
pub fn min_thickness(mesh: &Mesh, solid: &Manifold) -> Option<(f64, V3)> {
    let n = mesh.indices.len() / 3;
    let mut tris: Vec<(f64, usize)> = (0..n)
        .map(|t| {
            let (a, b, c) = tri(mesh, t);
            (length(cross(sub(b, a), sub(c, a))) / 2.0, t)
        })
        .collect();
    tris.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let step = (tris.len() / 300).max(1);
    let mut worst: Option<(f64, V3)> = None;
    for (area, t) in tris.iter().step_by(step).take(300) {
        if *area < 0.5 {
            continue;
        }
        let (a, b, c) = tri(mesh, *t);
        let nrm = normalize(cross(sub(b, a), sub(c, a)));
        let cen = scale(add(add(a, b), c), 1.0 / 3.0);
        let from = sub(cen, scale(nrm, 1e-3));
        let to = sub(cen, scale(nrm, 20.0));
        let hits = solid.ray_cast(v3(from), v3(to));
        let Some(h) = hits
            .iter()
            .filter(|h| h.distance * 20.0 > 0.02)
            .min_by(|x, y| x.distance.partial_cmp(&y.distance).unwrap())
        else {
            continue;
        };
        let d = h.distance * (20.0 - 1e-3);
        if worst.map_or(true, |(w, _)| d < w) {
            worst = Some((d, cen));
        }
    }
    worst
}

fn thin_walls(
    p: &BuiltPart,
    mesh: &Mesh,
    solid: &Manifold,
    o: &CheckOptions,
    out: &mut Vec<Issue>,
) {
    let min_wall = 2.0 * o.nozzle;
    if let Some((d, at)) = min_thickness(mesh, solid).filter(|(d, _)| *d < min_wall) {
        out.push(
            Issue::warning("thin_wall", &p.path, format!("part \"{}\" has a wall only {} mm thick near {} (print orientation); minimum for a {} mm nozzle is {} mm", p.id, fmt_num((d * 100.0).round() / 100.0), fmt_point(at), o.nozzle, fmt_num(min_wall)))
                .hint("Increase that wall/feature thickness (or its param's min) so it prints reliably."),
        );
    }
}

fn overhangs(p: &BuiltPart, mesh: &Mesh, o: &CheckOptions, out: &mut Vec<Issue>) {
    if o.supports_ok {
        return;
    }
    let mut total = 0.0;
    let mut over = 0.0;
    let mut where_ = Bbox {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    for t in 0..mesh.indices.len() / 3 {
        let (a, b, c) = tri(mesh, t);
        let cr = cross(sub(b, a), sub(c, a));
        let area = length(cr) / 2.0;
        total += area;
        let nz = normalize(cr)[2];
        let zc = (a[2] + b[2] + c[2]) / 3.0;
        if nz < -0.72 && zc > 0.3 {
            over += area;
            for q in [a, b, c] {
                for k in 0..3 {
                    where_.min[k] = where_.min[k].min(q[k]);
                    where_.max[k] = where_.max[k].max(q[k]);
                }
            }
        }
    }
    if over > 25.0 && over > 0.05 * total {
        out.push(
            Issue::warning("overhang", &p.path, format!("part \"{}\" has {} mm² of downward-facing surface steeper than 45° in its print orientation (around {}); it will need supports or bridging", p.id, fmt_num(over.round()), fmt_bbox(&where_)))
                .hint("Reorient with print_rotate, add chamfers / 45° slopes under overhangs, or split the part. Set print.supports: true if supports are acceptable."),
        );
    }
}
