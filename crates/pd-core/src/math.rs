//! Small affine-transform and anchor helpers.

use manifold_rust::linalg::{Mat3x4, Vec3};
use manifold_rust::manifold::Manifold;

pub type V3 = [f64; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn length(a: V3) -> f64 {
    dot(a, a).sqrt()
}
pub fn normalize(a: V3) -> V3 {
    let l = length(a);
    if l == 0.0 {
        a
    } else {
        scale(a, 1.0 / l)
    }
}

/// Affine transform stored as 3 rows of `[r0 r1 r2 t]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub m: [[f64; 4]; 3],
}

impl Xf {
    pub const IDENTITY: Xf = Xf {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    };

    pub fn translate(t: V3) -> Xf {
        let mut x = Xf::IDENTITY;
        x.m[0][3] = t[0];
        x.m[1][3] = t[1];
        x.m[2][3] = t[2];
        x
    }

    /// Transform whose columns are the images of the X, Y and Z axes.
    pub fn from_axes(x: V3, y: V3, z: V3) -> Xf {
        Xf {
            m: [
                [x[0], y[0], z[0], 0.0],
                [x[1], y[1], z[1], 0.0],
                [x[2], y[2], z[2], 0.0],
            ],
        }
    }

    /// Rotation about an arbitrary unit axis through the origin (degrees).
    pub fn rotate_axis(axis: V3, deg: f64) -> Xf {
        let a = normalize(axis);
        let (s, c) = deg.to_radians().sin_cos();
        let t = 1.0 - c;
        let [x, y, z] = a;
        Xf {
            m: [
                [t * x * x + c, t * x * y - s * z, t * x * z + s * y, 0.0],
                [t * x * y + s * z, t * y * y + c, t * y * z - s * x, 0.0],
                [t * x * z - s * y, t * y * z + s * x, t * z * z + c, 0.0],
            ],
        }
    }

    /// Euler rotation applied about X, then Y, then Z (degrees).
    pub fn rotate_xyz(r: V3) -> Xf {
        let rx = Xf::rotate_axis([1.0, 0.0, 0.0], r[0]);
        let ry = Xf::rotate_axis([0.0, 1.0, 0.0], r[1]);
        let rz = Xf::rotate_axis([0.0, 0.0, 1.0], r[2]);
        rz.then_after(&ry).then_after(&rx)
    }

    /// `self ∘ other`: apply `other` first, then `self`.
    pub fn then_after(&self, other: &Xf) -> Xf {
        let a = &self.m;
        let b = &other.m;
        let mut r = [[0.0; 4]; 3];
        for i in 0..3 {
            for j in 0..4 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += a[i][k] * b[k][j];
                }
                if j == 3 {
                    s += a[i][3];
                }
                r[i][j] = s;
            }
        }
        Xf { m: r }
    }

    pub fn point(&self, p: V3) -> V3 {
        let m = &self.m;
        [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
        ]
    }

    pub fn dir(&self, d: V3) -> V3 {
        let m = &self.m;
        [
            m[0][0] * d[0] + m[0][1] * d[1] + m[0][2] * d[2],
            m[1][0] * d[0] + m[1][1] * d[1] + m[1][2] * d[2],
            m[2][0] * d[0] + m[2][1] * d[1] + m[2][2] * d[2],
        ]
    }

    pub fn to_manifold(&self) -> Mat3x4 {
        let m = &self.m;
        Mat3x4::from_cols(
            Vec3::new(m[0][0], m[1][0], m[2][0]),
            Vec3::new(m[0][1], m[1][1], m[2][1]),
            Vec3::new(m[0][2], m[1][2], m[2][2]),
            Vec3::new(m[0][3], m[1][3], m[2][3]),
        )
    }

    pub fn apply(&self, s: &Manifold) -> Manifold {
        if *self == Xf::IDENTITY {
            s.clone()
        } else {
            s.transform(&self.to_manifold())
        }
    }
}

pub fn v3(p: V3) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bbox {
    pub min: V3,
    pub max: V3,
}

impl Bbox {
    pub fn of(m: &Manifold) -> Bbox {
        let b = m.bounding_box();
        Bbox {
            min: [b.min.x, b.min.y, b.min.z],
            max: [b.max.x, b.max.y, b.max.z],
        }
    }
    pub fn size(&self) -> V3 {
        sub(self.max, self.min)
    }
    pub fn center(&self) -> V3 {
        scale(add(self.min, self.max), 0.5)
    }
    pub fn is_valid(&self) -> bool {
        self.min
            .iter()
            .zip(self.max.iter())
            .all(|(a, b)| a.is_finite() && b.is_finite() && a <= b)
    }
    pub fn overlaps(&self, o: &Bbox, eps: f64) -> bool {
        (0..3).all(|i| self.min[i] < o.max[i] - eps && o.min[i] < self.max[i] - eps)
    }
    pub fn union(&self, o: &Bbox) -> Bbox {
        Bbox {
            min: [
                self.min[0].min(o.min[0]),
                self.min[1].min(o.min[1]),
                self.min[2].min(o.min[2]),
            ],
            max: [
                self.max[0].max(o.max[0]),
                self.max[1].max(o.max[1]),
                self.max[2].max(o.max[2]),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub pos: V3,
    pub normal: V3,
}

impl Anchor {
    pub fn transformed(&self, xf: &Xf) -> Anchor {
        Anchor {
            pos: xf.point(self.pos),
            normal: normalize(xf.dir(self.normal)),
        }
    }
}

const TOKENS: &[(&str, usize, f64)] = &[
    ("top", 2, 1.0),
    ("bottom", 2, -1.0),
    ("right", 0, 1.0),
    ("left", 0, -1.0),
    ("back", 1, 1.0),
    ("front", 1, -1.0),
];

/// Split an anchor name like "top-left" into face tokens.
pub fn anchor_tokens(name: &str) -> Option<Vec<&'static str>> {
    let mut out = Vec::new();
    for t in name.split(|c| c == '-' || c == '_' || c == ' ') {
        let t = t.to_ascii_lowercase();
        match t.as_str() {
            "center" | "centre" | "middle" | "" => {}
            _ => {
                let (tok, _, _) = TOKENS.iter().find(|(n, _, _)| *n == t)?;
                out.push(*tok);
            }
        }
    }
    let mut axes = [false; 3];
    for t in &out {
        let (_, axis, _) = TOKENS.iter().find(|(n, _, _)| n == t).unwrap();
        if axes[*axis] {
            return None;
        }
        axes[*axis] = true;
    }
    Some(out)
}

pub fn token_normal(tok: &str) -> V3 {
    let (_, axis, sign) = TOKENS.iter().find(|(n, _, _)| *n == tok).unwrap();
    let mut n = [0.0; 3];
    n[*axis] = *sign;
    n
}

pub fn opposite_token(tok: &str) -> &'static str {
    match tok {
        "top" => "bottom",
        "bottom" => "top",
        "left" => "right",
        "right" => "left",
        "front" => "back",
        _ => "front",
    }
}

/// Face token pointing along an axis-aligned direction, if any.
pub fn token_for_normal(n: V3) -> Option<&'static str> {
    TOKENS
        .iter()
        .find(|(_, axis, sign)| (n[*axis] - sign).abs() < 1e-6)
        .map(|(t, _, _)| *t)
}

/// Generic anchor on a bounding box.
pub fn bbox_anchor(b: &Bbox, tokens: &[&str]) -> Anchor {
    let mut pos = b.center();
    for t in tokens {
        let (_, axis, sign) = TOKENS.iter().find(|(n, _, _)| n == t).unwrap();
        pos[*axis] = if *sign > 0.0 {
            b.max[*axis]
        } else {
            b.min[*axis]
        };
    }
    let normal = tokens
        .first()
        .map(|t| token_normal(t))
        .unwrap_or([0.0, 0.0, 1.0]);
    Anchor { pos, normal }
}

/// Rotation taking local +Z to `n`. On side faces local Y points up and
/// local X runs horizontally along the face.
pub fn orient_to(n: V3) -> Xf {
    let n = normalize(n);
    match token_for_normal(n) {
        Some("top") => Xf::IDENTITY,
        Some("bottom") => Xf::from_axes([1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]),
        Some("front") => Xf::from_axes([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
        Some("back") => Xf::from_axes([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        Some("right") => Xf::from_axes([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        Some("left") => Xf::from_axes([0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0]),
        _ => {
            let z = [0.0, 0.0, 1.0];
            let axis = cross(z, n);
            let s = length(axis);
            let c = dot(z, n);
            if s < 1e-9 {
                return if c > 0.0 {
                    Xf::IDENTITY
                } else {
                    Xf::rotate_axis([1.0, 0.0, 0.0], 180.0)
                };
            }
            Xf::rotate_axis(axis, s.atan2(c).to_degrees())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: V3, b: V3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
    }

    #[test]
    fn orient_maps_z_to_normal_and_y_up() {
        for n in [
            [0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 1.0],
        ] {
            let o = orient_to(n);
            assert!(close(o.dir([0.0, 0.0, 1.0]), n));
            let det = dot(
                o.dir([1.0, 0.0, 0.0]),
                cross(o.dir([0.0, 1.0, 0.0]), o.dir([0.0, 0.0, 1.0])),
            );
            assert!((det - 1.0).abs() < 1e-9);
            if n[2] == 0.0 {
                assert!(close(o.dir([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]));
            }
        }
    }

    #[test]
    fn euler_order_and_compose() {
        let r = Xf::rotate_xyz([90.0, 0.0, 90.0]);
        // X then Z: +Y -> +Z (about X) -> stays +Z (about Z)
        assert!(close(r.dir([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]));
        let t = Xf::translate([1.0, 2.0, 3.0]).then_after(&Xf::rotate_axis([0.0, 0.0, 1.0], 90.0));
        assert!(close(t.point([1.0, 0.0, 0.0]), [1.0, 3.0, 3.0]));
    }

    #[test]
    fn anchors() {
        let b = Bbox {
            min: [-1.0, -2.0, 0.0],
            max: [1.0, 2.0, 4.0],
        };
        let a = bbox_anchor(&b, &anchor_tokens("top-left").unwrap());
        assert!(close(a.pos, [-1.0, 0.0, 4.0]));
        assert!(close(a.normal, [0.0, 0.0, 1.0]));
        assert!(anchor_tokens("top-bottom").is_none());
        assert!(anchor_tokens("upper").is_none());
        assert_eq!(anchor_tokens("center").unwrap().len(), 0);
    }
}
