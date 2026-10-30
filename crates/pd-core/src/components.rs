//! Geometry for every component type. Each shape is built in its local
//! frame: centred on X/Y, resting on z = 0, Z up.

use crate::design::ShapeDef;
use crate::eval::Fields;
use crate::expr::{fmt_num, Env};
use crate::issues::Issue;
use crate::math::{Anchor, Xf};
use manifold_rust::linalg::{Vec2, Vec3};
use manifold_rust::manifold::Manifold;
use std::f64::consts::PI;

pub struct Solid {
    pub m: Manifold,
    /// Component-specific anchors in the local frame.
    pub anchors: Vec<(String, Anchor)>,
    /// Cuts this component makes in other parts: (part id, cutter in that part's local frame).
    pub foreign_cuts: Vec<(String, Manifold)>,
    /// Default print rotation (e.g. lids print upside down).
    pub print_rotate: Option<[f64; 3]>,
}

impl Solid {
    fn plain(m: Manifold) -> Solid {
        Solid {
            m,
            anchors: vec![],
            foreign_cuts: vec![],
            print_rotate: None,
        }
    }
}

/// What a component may need from the rest of the design.
pub struct ShapeCtx<'a> {
    /// Material thickness under the placement point, for `through` cutters.
    pub through_depth: Option<f64>,
    /// Look up another part's base shape by id.
    pub part_shape: &'a dyn Fn(&str) -> Option<&'a ShapeDef>,
}

/// Facet count for a circle of radius `r`.
pub fn segments(r: f64) -> i32 {
    let n = (2.0 * PI * r / 0.5).ceil() as i32;
    let n = n.clamp(24, 144);
    (n + 3) / 4 * 4
}

fn ccw(mut poly: Vec<Vec2>) -> Vec<Vec2> {
    let mut area = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        area += a.x * b.y - b.x * a.y;
    }
    if area < 0.0 {
        poly.reverse();
    }
    poly
}

fn poly_area(poly: &[Vec2]) -> f64 {
    let mut area = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        area += a.x * b.y - b.x * a.y;
    }
    area.abs() / 2.0
}

/// Centred rectangle with rounded corners.
pub fn rounded_rect(w: f64, h: f64, r: f64) -> Vec<Vec2> {
    let r = r.min(w / 2.0 - 1e-6).min(h / 2.0 - 1e-6);
    if r <= 1e-6 {
        return vec![
            Vec2::new(-w / 2.0, -h / 2.0),
            Vec2::new(w / 2.0, -h / 2.0),
            Vec2::new(w / 2.0, h / 2.0),
            Vec2::new(-w / 2.0, h / 2.0),
        ];
    }
    let n = (segments(r) / 4).max(4);
    let mut pts = Vec::new();
    let corners = [
        (w / 2.0 - r, -h / 2.0 + r, -90.0),
        (w / 2.0 - r, h / 2.0 - r, 0.0),
        (-w / 2.0 + r, h / 2.0 - r, 90.0),
        (-w / 2.0 + r, -h / 2.0 + r, 180.0),
    ];
    for (cx, cy, start) in corners {
        for i in 0..=n {
            let a = (start + 90.0 * i as f64 / n as f64).to_radians();
            pts.push(Vec2::new(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts
}

fn circle(r: f64, cx: f64, cy: f64, n: i32) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / n as f64;
            Vec2::new(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

pub fn extrude(poly: Vec<Vec2>, h: f64) -> Manifold {
    Manifold::extrude(&vec![ccw(poly)], h, 0, 0.0, Vec2::new(1.0, 1.0))
}

fn cyl(r: f64, h: f64) -> Manifold {
    Manifold::cylinder(h, r, r, segments(r))
}

fn cone(r0: f64, r1: f64, h: f64) -> Manifold {
    Manifold::cylinder(h, r0, r1, segments(r0.max(r1)))
}

fn tr(m: &Manifold, x: f64, y: f64, z: f64) -> Manifold {
    m.translate(Vec3::new(x, y, z))
}

fn bad(path: &str, msg: impl Into<String>) -> Issue {
    Issue::error("bad_value", path, msg)
}

// --- metric hardware tables (mm) -------------------------------------------

struct Screw {
    clearance: f64,
    tap: f64,
    insert: f64,
    insert_len: f64,
    cap_d: f64,
    cap_h: f64,
    csk_d: f64,
    nut_af: f64,
    nut_h: f64,
}

fn screw(size: &str) -> Screw {
    let t = |c, t, i, il, cd, ch, kd, af, nh| Screw {
        clearance: c,
        tap: t,
        insert: i,
        insert_len: il,
        cap_d: cd,
        cap_h: ch,
        csk_d: kd,
        nut_af: af,
        nut_h: nh,
    };
    match size {
        "M2" => t(2.4, 1.7, 3.2, 4.0, 4.4, 2.2, 4.2, 4.0, 1.6),
        "M2.5" => t(2.9, 2.2, 3.6, 4.0, 5.0, 2.7, 5.0, 5.0, 2.0),
        "M3" => t(3.4, 2.6, 4.0, 5.7, 6.2, 3.2, 6.2, 5.5, 2.4),
        "M4" => t(4.5, 3.5, 5.6, 8.1, 8.0, 4.2, 8.2, 7.0, 3.2),
        "M5" => t(5.5, 4.4, 6.4, 9.5, 9.5, 5.2, 10.2, 8.0, 4.0),
        "M6" => t(6.6, 5.2, 8.0, 12.7, 11.0, 6.2, 12.2, 10.0, 5.0),
        _ => t(9.0, 7.0, 10.0, 12.7, 14.5, 8.2, 16.4, 13.0, 6.5),
    }
}

// --- container helpers -----------------------------------------------------

pub struct HollowBoxDims {
    pub outer: [f64; 3],
    pub inner: [f64; 3],
    pub wall: f64,
    pub floor: f64,
    pub radius: f64,
    pub open_top: bool,
}

pub fn hollow_box_dims(f: &Fields) -> Result<HollowBoxDims, Issue> {
    let wall = f.len_or("wall", 2.0)?;
    let floor = match f.opt_len("floor")? {
        Some(v) => v,
        None => wall,
    };
    let radius = f.non_negative("radius", 0.0)?;
    let open_top = f.text("open", "top", &["top", "none"])? == "top";
    let lid_t = if open_top { 0.0 } else { floor };
    let (outer, inner) = if let Some(inner) = f.size3("inner")? {
        (
            [
                inner[0] + 2.0 * wall,
                inner[1] + 2.0 * wall,
                inner[2] + floor + lid_t,
            ],
            inner,
        )
    } else if let Some(outer) = f.size3("outer")? {
        let inner = [
            outer[0] - 2.0 * wall,
            outer[1] - 2.0 * wall,
            outer[2] - floor - lid_t,
        ];
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            if inner[i] <= 0.0 {
                return Err(bad(&format!("{}.outer", f.path), format!("outer {axis} ({}) is too small for the walls/floor: inside would be {}", fmt_num(outer[i]), fmt_num(inner[i])))
                    .hint("Make outer larger than 2 x wall (x, y) and floor (z), or give \"inner\" instead."));
            }
        }
        (outer, inner)
    } else {
        return Err(Issue::error(
            "missing_field",
            f.path,
            "hollow_box needs \"inner\" or \"outer\"",
        ));
    };
    Ok(HollowBoxDims {
        outer,
        inner,
        wall,
        floor,
        radius,
        open_top,
    })
}

fn rounded_box(size: [f64; 3], r: f64) -> Manifold {
    extrude(rounded_rect(size[0], size[1], r), size[2])
}

// --- builders ----------------------------------------------------------------

pub fn build(shape: &ShapeDef, env: &Env, cx: &ShapeCtx) -> Result<Solid, Issue> {
    let f = Fields::of(shape, env);
    let p = |k: &str| format!("{}.{k}", shape.path);
    let radius_from = |d: &str, r: &str| -> Result<f64, Issue> {
        if let Some(d) = f.opt_len(d)? {
            Ok(d / 2.0)
        } else {
            f.len(r)
        }
    };
    let s = match shape.ty.as_str() {
        "box" => {
            let size = f.size3("size")?.unwrap();
            let r = f.non_negative("radius", 0.0)?;
            if r > 0.0 && 2.0 * r >= size[0].min(size[1]) {
                return Err(bad(
                    &p("radius"),
                    format!(
                        "radius {} is too big for a {} x {} box",
                        fmt_num(r),
                        fmt_num(size[0]),
                        fmt_num(size[1])
                    ),
                )
                .hint("radius must be less than half the smaller of x and y."));
            }
            Solid::plain(rounded_box(size, r))
        }
        "cylinder" => {
            let r = radius_from("d", "r")?;
            let h = f.len("h")?;
            let r_top = if f.has("d_top") {
                f.non_negative("d_top", 0.0)? / 2.0
            } else if f.has("r_top") {
                f.non_negative("r_top", 0.0)?
            } else {
                r
            };
            let n = match f.opt_num("segments")? {
                Some(n) if n >= 3.0 => n as i32,
                Some(_) => return Err(bad(&p("segments"), "segments must be at least 3")),
                None => segments(r.max(r_top)),
            };
            Solid::plain(Manifold::cylinder(h, r, r_top, n))
        }
        "sphere" => {
            let r = radius_from("d", "r")?;
            Solid::plain(tr(&Manifold::sphere(r, segments(r)), 0.0, 0.0, r))
        }
        "tube" => {
            let ro = f.len("d")? / 2.0;
            let h = f.len("h")?;
            let ri = match f.opt_len("d_inner")? {
                Some(d) => d / 2.0,
                None => ro - f.len("wall")?,
            };
            if ri <= 0.0 || ri >= ro {
                return Err(bad(
                    &shape.path,
                    format!(
                        "tube inner radius {} must be between 0 and the outer radius {}",
                        fmt_num(ri),
                        fmt_num(ro)
                    ),
                ));
            }
            Solid::plain(cyl(ro, h).difference(&tr(&cyl(ri, h + 2.0), 0.0, 0.0, -1.0)))
        }
        "prism" => {
            let pts = f.points("points")?.unwrap();
            if pts.len() < 3 {
                return Err(bad(&p("points"), "prism needs at least 3 points"));
            }
            let poly: Vec<Vec2> = pts.iter().map(|q| Vec2::new(q[0], q[1])).collect();
            if poly_area(&poly) < 1e-6 {
                return Err(bad(
                    &p("points"),
                    "prism outline has no area (points are in a line?)",
                ));
            }
            let h = f.len("h")?;
            let m = extrude(poly, h);
            if m.is_empty() {
                return Err(bad(&p("points"), "prism outline is invalid (self-crossing?)").hint("List the outline corners in order around the shape without crossing lines."));
            }
            Solid::plain(m)
        }
        "revolve" => {
            let pts = f.points("profile")?.unwrap();
            if pts.len() < 3 {
                return Err(bad(
                    &p("profile"),
                    "revolve profile needs at least 3 points",
                ));
            }
            if let Some((i, q)) = pts.iter().enumerate().find(|(_, q)| q[0] < -1e-9) {
                return Err(bad(
                    &format!("{}[{i}]", p("profile")),
                    format!(
                        "radius {} is negative; profile points are [radius, z] with radius >= 0",
                        fmt_num(q[0])
                    ),
                ));
            }
            let poly: Vec<Vec2> = pts.iter().map(|q| Vec2::new(q[0].max(0.0), q[1])).collect();
            if poly_area(&poly) < 1e-6 {
                return Err(bad(&p("profile"), "revolve profile has no area"));
            }
            let angle = f.num("angle", 360.0)?;
            if angle <= 0.0 || angle > 360.0 {
                return Err(bad(&p("angle"), "angle must be in (0, 360]"));
            }
            let rmax = pts.iter().map(|q| q[0]).fold(0.0, f64::max);
            let m = Manifold::revolve(&vec![ccw(poly)], segments(rmax), angle);
            let zmin = pts.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min);
            Solid::plain(tr(&m, 0.0, 0.0, -zmin))
        }
        "torus" => {
            let rr = f.len("d")? / 2.0;
            let t = f.len("thickness")? / 2.0;
            if t >= rr {
                return Err(bad(
                    &p("thickness"),
                    "thickness must be smaller than d (otherwise the ring has no hole)",
                ));
            }
            let m = Manifold::revolve(
                &vec![circle(t, rr, 0.0, segments(t))],
                segments(rr + t),
                360.0,
            );
            Solid::plain(tr(&m, 0.0, 0.0, t))
        }
        "wedge" => {
            let s = f.size3("size")?.unwrap();
            let tri = vec![
                Vec2::new(-s[1] / 2.0, 0.0),
                Vec2::new(s[1] / 2.0, 0.0),
                Vec2::new(s[1] / 2.0, s[2]),
            ];
            let m = extrude(tri, s[0]);
            // local (x', y', z') = (Y, Z, X) -> world
            let xf = Xf::translate([-s[0] / 2.0, 0.0, 0.0]).then_after(&Xf::from_axes(
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
            ));
            Solid::plain(xf.apply(&m))
        }
        "slot" => {
            let l = f.len("length")?;
            let w = f.len("width")?;
            let h = f.len("h")?;
            if l < w {
                return Err(bad(
                    &p("length"),
                    format!(
                        "length {} is shorter than width {}; length is the overall length",
                        fmt_num(l),
                        fmt_num(w)
                    ),
                ));
            }
            Solid::plain(extrude(rounded_rect(l, w, w / 2.0), h))
        }
        "hollow_box" => {
            let d = hollow_box_dims(&f)?;
            let outer = rounded_box(d.outer, d.radius);
            let cavity_h = if d.open_top {
                d.inner[2] + 1.0
            } else {
                d.inner[2]
            };
            let cavity = tr(
                &rounded_box(
                    [d.inner[0], d.inner[1], cavity_h],
                    (d.radius - d.wall).max(0.0),
                ),
                0.0,
                0.0,
                d.floor,
            );
            let mut s = Solid::plain(outer.difference(&cavity));
            s.anchors.push((
                "inside_bottom".into(),
                Anchor {
                    pos: [0.0, 0.0, d.floor],
                    normal: [0.0, 0.0, 1.0],
                },
            ));
            s
        }
        "lid" => build_lid(&f, shape, cx)?,
        "screw_hole" => {
            let size = f.text("size", "M3", crate::spec::SCREW_SIZES)?;
            let sc = screw(&size);
            let fit = f.text("fit", "clearance", &["clearance", "tap", "insert"])?;
            let head = f.text("head", "none", &["none", "cap", "countersunk"])?;
            let d = match fit.as_str() {
                "tap" => sc.tap,
                "insert" => sc.insert,
                _ => sc.clearance,
            };
            let depth = if f.bool("through", false)? {
                cx.through_depth.ok_or_else(|| {
                    Issue::error("bad_place", &shape.path, "\"through\" needs the hole to be placed with \"on\" a face of the part").hint("Add \"place\": {\"on\": \"top\"} (or another face), or give \"depth\" instead.")
                })?
            } else {
                match f.opt_len("depth")? {
                    Some(v) => v,
                    None if fit == "insert" => sc.insert_len + 1.0,
                    None => {
                        return Err(Issue::error(
                            "missing_field",
                            &shape.path,
                            "screw_hole needs \"depth\" or \"through\": true",
                        ))
                    }
                }
            };
            let mut m = cyl(d / 2.0, depth);
            match head.as_str() {
                "cap" => {
                    if depth <= sc.cap_h {
                        return Err(bad(
                            &shape.path,
                            format!(
                                "hole depth {} is not deeper than the cap head recess {}",
                                fmt_num(depth),
                                fmt_num(sc.cap_h)
                            ),
                        ));
                    }
                    m = m.union(&tr(
                        &cyl(sc.cap_d / 2.0, sc.cap_h),
                        0.0,
                        0.0,
                        depth - sc.cap_h,
                    ));
                }
                "countersunk" => {
                    let ch = ((sc.csk_d - d) / 2.0).min(depth);
                    m = m.union(&tr(
                        &cone(d / 2.0, sc.csk_d / 2.0, ch),
                        0.0,
                        0.0,
                        depth - ch,
                    ));
                }
                _ => {}
            }
            Solid::plain(m)
        }
        "nut_trap" => {
            let size = f.text("size", "M3", crate::spec::SCREW_SIZES)?;
            let sc = screw(&size);
            let c = f.non_negative("clearance", 0.15)?;
            let depth = match f.opt_len("depth")? {
                Some(v) => v,
                None => sc.nut_h + 0.3,
            };
            let rc = (sc.nut_af / 2.0 + c) / (PI / 6.0).cos();
            let hex: Vec<Vec2> = (0..6)
                .map(|i| {
                    let a = PI / 3.0 * i as f64;
                    Vec2::new(rc * a.cos(), rc * a.sin())
                })
                .collect();
            Solid::plain(extrude(hex, depth))
        }
        "magnet_pocket" => {
            let d = f.len("d")?;
            let h = f.len("h")?;
            let c = f.non_negative("clearance", 0.15)?;
            Solid::plain(cyl(d / 2.0 + c, h + 0.2))
        }
        "standoff" => {
            let size = f.text("screw", "M3", crate::spec::SCREW_SIZES)?;
            let sc = screw(&size);
            let h = f.len("h")?;
            let fit = f.text("fit", "tap", &["tap", "insert"])?;
            let hole = if fit == "insert" { sc.insert } else { sc.tap };
            let d = match f.opt_len("d")? {
                Some(d) => d,
                None => ((hole + 3.6) * 2.0).ceil() / 2.0,
            };
            if d <= hole + 0.8 {
                return Err(bad(
                    &p("d"),
                    format!(
                        "standoff diameter {} leaves no wall around the {} hole ({})",
                        fmt_num(d),
                        size,
                        fmt_num(hole)
                    ),
                ));
            }
            Solid::plain(cyl(d / 2.0, h).difference(&tr(
                &cyl(hole / 2.0, h + 2.0),
                0.0,
                0.0,
                1.0_f64.min(h / 2.0),
            )))
        }
        other => {
            return Err(Issue::error(
                "unknown_type",
                &shape.path,
                format!("unknown shape type \"{other}\""),
            ))
        }
    };
    if s.m.is_empty() {
        return Err(Issue::error(
            "empty_shape",
            &shape.path,
            format!("\"{}\" produced no geometry", shape.ty),
        )
        .hint("Check that its sizes are positive and its outline is valid."));
    }
    Ok(s)
}

fn build_lid(f: &Fields, shape: &ShapeDef, cx: &ShapeCtx) -> Result<Solid, Issue> {
    let target = f.part_ref("fits").unwrap_or_default();
    let path = format!("{}.fits", shape.path);
    let Some(tshape) = (cx.part_shape)(&target) else {
        return Err(Issue::error(
            "unknown_ref",
            &path,
            format!("no part with id \"{target}\""),
        ));
    };
    if tshape.ty != "hollow_box" {
        return Err(Issue::error(
            "bad_ref",
            &path,
            format!(
                "lid must fit a part whose shape is hollow_box, but \"{target}\" is a {}",
                tshape.ty
            ),
        ));
    }
    let tf = Fields {
        fields: &tshape.fields,
        path: &tshape.path,
        env: f.env,
    };
    let d = hollow_box_dims(&tf)?;
    if !d.open_top {
        return Err(Issue::error(
            "bad_ref",
            &path,
            format!("\"{target}\" is closed (open: \"none\"); a lid needs an open top"),
        ));
    }
    let style = f.text("style", "lip", &["lip", "flat", "snap"])?;
    let c = f.non_negative("clearance", 0.2)?;
    let t = f.len_or("thickness", 2.0)?;
    let lip_h = if style == "flat" {
        0.0
    } else {
        f.len_or("lip_height", 4.0)?
    };
    let lip_w = f.len_or("lip_wall", 1.6)?;
    let plate = tr(
        &rounded_box([d.outer[0], d.outer[1], t], d.radius),
        0.0,
        0.0,
        lip_h,
    );
    let mut solid = Solid {
        m: plate,
        anchors: vec![],
        foreign_cuts: vec![],
        print_rotate: Some([180.0, 0.0, 0.0]),
    };
    solid.anchors.push((
        "seat".into(),
        Anchor {
            pos: [0.0, 0.0, lip_h],
            normal: [0.0, 0.0, -1.0],
        },
    ));
    if style == "flat" {
        return Ok(solid);
    }
    if lip_h >= d.inner[2] {
        return Err(bad(
            &format!("{}.lip_height", shape.path),
            format!(
                "lip_height {} does not fit in the box's inside depth {}",
                fmt_num(lip_h),
                fmt_num(d.inner[2])
            ),
        ));
    }
    let lo = [d.inner[0] - 2.0 * c, d.inner[1] - 2.0 * c];
    let li = [lo[0] - 2.0 * lip_w, lo[1] - 2.0 * lip_w];
    if li[0] <= 0.0 || li[1] <= 0.0 {
        return Err(bad(
            &format!("{}.lip_wall", shape.path),
            "lip_wall is too thick for this box",
        ));
    }
    let r_out = (d.radius - d.wall - c).max(0.0);
    let ring = rounded_box([lo[0], lo[1], lip_h + 0.5], r_out).difference(&tr(
        &rounded_box([li[0], li[1], lip_h + 2.0], (r_out - lip_w).max(0.0)),
        0.0,
        0.0,
        -1.0,
    ));
    solid.m = solid.m.union(&ring);
    if style == "snap" {
        let s = f.len_or("snap_size", 0.6)?;
        let z = lip_h / 2.0;
        let box_z = d.outer[2] - lip_h / 2.0;
        // (axis along which the bump runs, side length, position of lip face, groove face)
        let sides = [
            (
                [0.0, 1.0, 0.0],
                lo[1],
                [lo[0] / 2.0, 0.0],
                [d.inner[0] / 2.0, 0.0],
            ),
            (
                [0.0, 1.0, 0.0],
                lo[1],
                [-lo[0] / 2.0, 0.0],
                [-d.inner[0] / 2.0, 0.0],
            ),
            (
                [1.0, 0.0, 0.0],
                lo[0],
                [0.0, lo[1] / 2.0],
                [0.0, d.inner[1] / 2.0],
            ),
            (
                [1.0, 0.0, 0.0],
                lo[0],
                [0.0, -lo[1] / 2.0],
                [0.0, -d.inner[1] / 2.0],
            ),
        ];
        let mut grooves = Vec::new();
        for (axis, len, lip_pos, groove_pos) in sides {
            let l = len * 0.6;
            let lay = |r: f64, extra: f64| -> Manifold {
                let c = tr(
                    &Manifold::cylinder(l + extra, r, r, 16),
                    0.0,
                    0.0,
                    -(l + extra) / 2.0,
                );
                if axis[0] == 1.0 {
                    c.rotate(0.0, 90.0, 0.0)
                } else {
                    c.rotate(90.0, 0.0, 0.0)
                }
            };
            solid.m = solid.m.union(&tr(&lay(s, 0.0), lip_pos[0], lip_pos[1], z));
            grooves.push(tr(&lay(s + 0.1, 1.0), groove_pos[0], groove_pos[1], box_z));
        }
        let all = grooves.into_iter().reduce(|a, b| a.union(&b)).unwrap();
        solid.foreign_cuts.push((target, all));
    }
    Ok(solid)
}
