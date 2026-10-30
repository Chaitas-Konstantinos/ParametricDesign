//! Splitting a part into printable pieces, with connectors at the seam.

use crate::build::{AnchorSet, BuiltPart, PartOrigin};
use crate::components::{extrude, segments};
use crate::design::{JointDef, SplitDef};
use crate::eval::{eval_num, Fields};
use crate::expr::{fmt_num, Env};
use crate::issues::{suggest, Issue};
use crate::math::*;
use manifold_rust::cross_section::CrossSection;
use manifold_rust::linalg::{Vec2, Vec3};
use manifold_rust::manifold::Manifold;
use manifold_rust::types::Polygons;
use std::collections::HashMap;

/// Dovetail keys thinner than this snap off; the floor/wall must be at least this tall.
const MIN_DOVETAIL_HEIGHT: f64 = 3.0;

/// World -> section frame (u, v, w): w runs along the split axis, (u, v) lie
/// in the cut plane with v vertical for x/y splits.
fn to_section(axis: char) -> Xf {
    match axis {
        'x' => Xf::from_axes([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        'y' => Xf::from_axes([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
        _ => Xf::IDENTITY,
    }
}

fn from_section(axis: char) -> Xf {
    match axis {
        'x' => Xf::from_axes([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        'y' => Xf::from_axes([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
        _ => Xf::IDENTITY,
    }
}

/// w coordinate of the plane `axis = at`.
fn w_of(axis: char, at: f64) -> f64 {
    if axis == 'y' {
        -at
    } else {
        at
    }
}

fn section(solid: &Manifold, axis: char, w: f64) -> CrossSection {
    to_section(axis).apply(solid).slice(w)
}

/// Material intervals along v at a given u (even-odd over all contours).
fn intervals(polys: &Polygons, u: f64) -> Vec<(f64, f64)> {
    let mut vs = vec![];
    for poly in polys {
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            if (a.x <= u) != (b.x <= u) {
                vs.push(a.y + (u - a.x) * (b.y - a.y) / (b.x - a.x));
            }
        }
    }
    vs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    vs.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect()
}

fn inside(polys: &Polygons, u: f64, v: f64) -> bool {
    intervals(polys, u).iter().any(|(a, b)| v > *a && v < *b)
}

struct JointGeo {
    add_neg: Vec<Manifold>,
    cut_neg: Vec<Manifold>,
    cut_pos: Vec<Manifold>,
    pins: Vec<Manifold>,
}

pub fn apply_split(parts: &mut Vec<BuiltPart>, s: &SplitDef, env: &Env, issues: &mut Vec<Issue>) {
    let Some(idx) = parts.iter().position(|p| p.id == s.part) else {
        let ids: Vec<&str> = parts.iter().map(|p| p.id.as_str()).collect();
        issues.push(
            Issue::error(
                "unknown_ref",
                format!("{}.part", s.path),
                format!("no built part \"{}\" to split", s.part),
            )
            .hint_opt(
                suggest(&s.part, ids.iter().copied()).map(|x| format!("Did you mean \"{x}\"?")),
            ),
        );
        return;
    };
    let at = match eval_num(&s.at, env, &format!("{}.at", s.path)) {
        Ok(v) => v,
        Err(e) => {
            issues.push(e);
            return;
        }
    };
    let part = parts[idx].clone();
    let b = Bbox::of(&part.solid);
    let ai = match s.axis {
        'x' => 0,
        'y' => 1,
        _ => 2,
    };
    let mut normal = [0.0; 3];
    normal[ai] = 1.0;
    if at <= b.min[ai] + 0.5 || at >= b.max[ai] - 0.5 {
        issues.push(
            Issue::error("split_misses", format!("{}.at", s.path), format!("the cut plane {} = {} does not go through \"{}\", which spans {} = {}..{}", s.axis, fmt_num(at), s.part, s.axis, fmt_num(b.min[ai]), fmt_num(b.max[ai])))
                .hint("\"at\" is a world coordinate on the split axis; the middle of the part is a good start."),
        );
        return;
    }
    let (pos, neg) = part.solid.split_by_plane(v3(normal), at);
    let (mut neg, mut pos) = (neg, pos);

    let mut pins = vec![];
    if let Some(j) = &s.joint {
        if j.ty != "none" {
            match joint_geometry(&part.solid, s.axis, at, j, env) {
                Ok(g) => {
                    for a in g.add_neg {
                        neg = neg.union(&a);
                    }
                    for c in g.cut_neg {
                        neg = neg.difference(&c);
                    }
                    for c in g.cut_pos {
                        pos = pos.difference(&c);
                    }
                    pins = g.pins;
                }
                Err(e) => {
                    issues.push(e);
                    return;
                }
            }
        }
    }
    let mk = |id: &str, solid: Manifold, origin: PartOrigin, print_rotate: [f64; 3]| BuiltPart {
        id: id.to_string(),
        name: part.name.clone(),
        color: part.color.clone(),
        anchors: AnchorSet {
            bbox: Bbox::of(&solid),
            extra: vec![],
        },
        solid,
        print_rotate,
        path: s.path.clone(),
        allow_disconnected: part.allow_disconnected,
        features: HashMap::new(),
        origin,
    };
    let mut new_parts = vec![
        mk(&s.names[0], neg, PartOrigin::SplitPiece, part.print_rotate),
        mk(&s.names[1], pos, PartOrigin::SplitPiece, part.print_rotate),
    ];
    let pin_rot = match s.axis {
        'x' => [0.0, 90.0, 0.0],
        'y' => [90.0, 0.0, 0.0],
        _ => [0.0, 0.0, 0.0],
    };
    for (i, p) in pins.into_iter().enumerate() {
        let mut bp = mk(
            &format!("{}_pin{}", s.part, i + 1),
            p,
            PartOrigin::Pin,
            pin_rot,
        );
        bp.color = "#c0c0c0".into();
        new_parts.push(bp);
    }
    parts.splice(idx..=idx, new_parts);
}

fn joint_geometry(
    solid: &Manifold,
    axis: char,
    at: f64,
    j: &JointDef,
    env: &Env,
) -> Result<JointGeo, Issue> {
    let f = Fields {
        fields: &j.fields,
        path: &j.path,
        env,
    };
    let w0 = w_of(axis, at);
    let back = from_section(axis);
    let count = f.int("count", 2)?;
    if !(1..=20).contains(&count) {
        return Err(Issue::error(
            "bad_value",
            format!("{}.count", j.path),
            "count must be between 1 and 20",
        ));
    }
    let count = count as usize;
    let margin = 1.2;
    let mut g = JointGeo {
        add_neg: vec![],
        cut_neg: vec![],
        cut_pos: vec![],
        pins: vec![],
    };
    match j.ty.as_str() {
        "dovetail" => {
            if axis == 'z' {
                return Err(Issue::error(
                    "bad_joint",
                    &j.path,
                    "dovetails need a vertical seam (split axis \"x\" or \"y\")",
                )
                .hint("For a horizontal cut (axis \"z\") use \"pin\" or \"magnet\"."));
            }
            let wn = f.len_or("width", 10.0)?;
            let depth = f.len_or("depth", 8.0)?;
            let angle = f.num("angle", 15.0)?;
            if !(0.0..=40.0).contains(&angle) {
                return Err(Issue::error(
                    "bad_value",
                    format!("{}.angle", j.path),
                    "angle must be between 0 and 40 degrees",
                ));
            }
            let c = f.non_negative("clearance", 0.15)?;
            let tan = angle.to_radians().tan();
            let wide = wn + 2.0 * depth * tan;
            let s0 = section(solid, axis, w0).to_polygons();
            let s1 = section(solid, axis, w0 + depth + margin).to_polygons();
            let column = |u: f64| -> Option<(f64, f64)> {
                let mut best: Option<(f64, f64)> = None;
                for (a, b) in intervals(&s0, u) {
                    if best.map_or(true, |(x, y)| b - a > y - x) {
                        best = Some((a, b));
                    }
                }
                let (mut lo, mut hi) = best?;
                let half = wide / 2.0 + c + margin;
                for k in 0..=8 {
                    let us = u - half + 2.0 * half * k as f64 / 8.0;
                    for sec in [&s0, &s1] {
                        let ov = intervals(sec, us)
                            .into_iter()
                            .filter(|(a, b)| *b > lo && *a < hi)
                            .max_by(|x, y| (x.1 - x.0).partial_cmp(&(y.1 - y.0)).unwrap())?;
                        lo = lo.max(ov.0);
                        hi = hi.min(ov.1);
                    }
                }
                (hi - lo >= MIN_DOVETAIL_HEIGHT).then_some((lo, hi))
            };
            let bounds = section(solid, axis, w0).bounds();
            let us: Vec<f64> = match f.opt_nums("positions")? {
                Some(ps) => {
                    for (i, u) in ps.iter().enumerate() {
                        if column(*u).is_none() {
                            return Err(Issue::error("joint_no_room", format!("{}.positions[{i}]", j.path), format!("a {}mm-wide dovetail at {} has no solid material around it on the seam", fmt_num(wide), fmt_num(*u)))
                                .hint("Move it onto solid material (not a hollow area), or remove \"positions\" to place them automatically."));
                        }
                    }
                    ps
                }
                None => {
                    let mut cands = vec![];
                    let mut u = bounds.min.x;
                    while u <= bounds.max.x {
                        if column(u).is_some() {
                            cands.push(u);
                        }
                        u += 0.5;
                    }
                    pick_spread(&cands, count, wide + 2.0).map_err(|found| {
                        Issue::error("joint_no_room", &j.path, format!("only {found} of {count} dovetails ({}mm wide incl. flare, plus {}mm margin) fit on solid material at the seam", fmt_num(wide), margin))
                            .hint("Each key needs at least 3 mm of solid material height (e.g. a floor of 3 mm or more) and room for its width plus margin. Thicken the floor/part where it is cut, lower \"count\" or \"width\"/\"depth\", or use \"pin\"/\"magnet\" joints.")
                    })?
                }
            };
            for u in us {
                let (lo, hi) = column(u).unwrap();
                let key = vec![
                    Vec2::new(u - wn / 2.0, w0 - 0.2),
                    Vec2::new(u + wn / 2.0, w0 - 0.2),
                    Vec2::new(u + wide / 2.0, w0 + depth),
                    Vec2::new(u - wide / 2.0, w0 + depth),
                ];
                let hn = wn / 2.0 + c - tan;
                let hw = wide / 2.0 + c + tan * c;
                let slot = vec![
                    Vec2::new(u - hn, w0 - 1.0),
                    Vec2::new(u + hn, w0 - 1.0),
                    Vec2::new(u + hw, w0 + depth + c),
                    Vec2::new(u - hw, w0 + depth + c),
                ];
                let lay = |poly: Vec<Vec2>, lo: f64, hi: f64| -> Manifold {
                    let m = extrude(poly, hi - lo).translate(Vec3::new(0.0, 0.0, -hi));
                    back.then_after(&Xf::rotate_axis([1.0, 0.0, 0.0], 90.0))
                        .apply(&m)
                };
                g.add_neg.push(lay(key, lo, hi));
                g.cut_pos.push(lay(slot, lo - 1.0, hi + 1.0));
            }
        }
        "pin" | "magnet" => {
            let (d, c, depth_neg, depth_pos) = if j.ty == "pin" {
                let d = f.len_or("d", 4.0)?;
                let l = f.len_or("length", 12.0)?;
                (
                    d,
                    f.non_negative("clearance", 0.15)?,
                    l / 2.0 + 0.5,
                    l / 2.0 + 0.5,
                )
            } else {
                let h = f.len_or("h", 3.0)?;
                (
                    f.len_or("d", 6.0)?,
                    f.non_negative("clearance", 0.1)?,
                    h + 0.2,
                    h + 0.2,
                )
            };
            let r = d / 2.0 + c;
            let erode = |w: f64| section(solid, axis, w).offset(-(r + margin));
            let region = erode(w0)
                .intersection(&erode(w0 - depth_neg - 0.5))
                .intersection(&erode(w0 + depth_pos + 0.5));
            let polys = region.to_polygons();
            let pts: Vec<[f64; 2]> = match f.points("positions")? {
                Some(ps) => {
                    for (i, p) in ps.iter().enumerate() {
                        if !inside(&polys, p[0], p[1]) {
                            return Err(Issue::error("joint_no_room", format!("{}.positions[{i}]", j.path), format!("a {} of {}mm at [{}, {}] is too close to an edge or a hollow (needs {}mm of material around it, also {}mm deep on both sides)", j.ty, fmt_num(d), fmt_num(p[0]), fmt_num(p[1]), fmt_num(margin), fmt_num(depth_neg)))
                                .hint("Move it onto thicker material or remove \"positions\" to place them automatically."));
                        }
                    }
                    ps
                }
                None => {
                    if region.is_empty() {
                        return Err(Issue::error("joint_no_room", &j.path, format!("the seam has no area thick enough for {}mm {}s (plus {}mm margin and {}mm depth each side)", fmt_num(d), j.ty, margin, fmt_num(depth_neg)))
                            .hint("Use a smaller d/length, thicken the part at the cut, or move the cut."));
                    }
                    let b = region.bounds();
                    let mut cands = vec![];
                    let mut u = b.min.x;
                    while u <= b.max.x {
                        if let Some((lo, hi)) = intervals(&polys, u)
                            .into_iter()
                            .max_by(|x, y| (x.1 - x.0).partial_cmp(&(y.1 - y.0)).unwrap())
                        {
                            cands.push((u, (lo + hi) / 2.0));
                        }
                        u += 0.5;
                    }
                    let us: Vec<f64> = cands.iter().map(|c| c.0).collect();
                    let picked = pick_spread(&us, count, 2.0 * r + 2.0).map_err(|found| {
                        Issue::error(
                            "joint_no_room",
                            &j.path,
                            format!("only {found} of {count} {}s fit on the seam", j.ty),
                        )
                        .hint("Lower \"count\" or the size.")
                    })?;
                    picked
                        .iter()
                        .map(|u| {
                            cands
                                .iter()
                                .find(|c| c.0 == *u)
                                .map(|c| [c.0, c.1])
                                .unwrap()
                        })
                        .collect()
                }
            };
            for p in pts {
                let at_uv = |m: Manifold| back.apply(&m.translate(Vec3::new(p[0], p[1], 0.0)));
                if j.ty == "pin" {
                    let hole = Manifold::cylinder(depth_neg + depth_pos, r, r, segments(r))
                        .translate(Vec3::new(0.0, 0.0, w0 - depth_neg));
                    let hole = at_uv(hole);
                    g.cut_neg.push(hole.clone());
                    g.cut_pos.push(hole);
                    if f.bool("make_pins", true)? {
                        let l = depth_neg + depth_pos - 1.4;
                        g.pins.push(at_uv(
                            Manifold::cylinder(l, d / 2.0, d / 2.0, segments(d / 2.0))
                                .translate(Vec3::new(0.0, 0.0, w0 - l / 2.0)),
                        ));
                    }
                } else {
                    g.cut_neg.push(at_uv(
                        Manifold::cylinder(depth_neg + 0.01, r, r, segments(r))
                            .translate(Vec3::new(0.0, 0.0, w0 - depth_neg)),
                    ));
                    g.cut_pos.push(at_uv(
                        Manifold::cylinder(depth_pos + 0.01, r, r, segments(r))
                            .translate(Vec3::new(0.0, 0.0, w0 - 0.01)),
                    ));
                }
            }
        }
        other => {
            return Err(Issue::error(
                "unknown_type",
                &j.path,
                format!("unknown joint type \"{other}\""),
            ))
        }
    }
    Ok(g)
}

/// Pick `n` values from sorted candidates, spread evenly, at least `gap` apart.
fn pick_spread(cands: &[f64], n: usize, gap: f64) -> Result<Vec<f64>, usize> {
    if cands.is_empty() {
        return Err(0);
    }
    let lo = cands[0];
    let hi = *cands.last().unwrap();
    let mut chosen: Vec<f64> = vec![];
    for i in 0..n {
        let target = if n == 1 {
            (lo + hi) / 2.0
        } else {
            lo + (hi - lo) * (0.15 + 0.7 * i as f64 / (n - 1) as f64)
        };
        let best = cands
            .iter()
            .filter(|c| chosen.iter().all(|x| (*x - **c).abs() >= gap))
            .min_by(|a, b| {
                (*a - target)
                    .abs()
                    .partial_cmp(&(*b - target).abs())
                    .unwrap()
            });
        match best {
            Some(b) => chosen.push(*b),
            None => return Err(chosen.len()),
        }
    }
    chosen.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_frames_are_inverse() {
        for axis in ['x', 'y', 'z'] {
            let a = to_section(axis);
            let b = from_section(axis);
            let p = [1.0, 2.0, 3.0];
            let q = b.point(a.point(p));
            assert!((0..3).all(|i| (p[i] - q[i]).abs() < 1e-12), "{axis}");
        }
        // x split: w is world x, v is world z
        let s = to_section('x').point([5.0, 1.0, 2.0]);
        assert_eq!(s, [1.0, 2.0, 5.0]);
        let s = to_section('y').point([5.0, 1.0, 2.0]);
        assert_eq!(s, [5.0, 2.0, -1.0]);
    }

    #[test]
    fn spread() {
        let c: Vec<f64> = (0..=100).map(|i| i as f64).collect();
        let p = pick_spread(&c, 2, 10.0).unwrap();
        assert_eq!(p, vec![15.0, 85.0]);
        assert!(pick_spread(&[1.0, 2.0], 2, 5.0).is_err());
    }
}
