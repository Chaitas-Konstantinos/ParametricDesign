//! Headless CPU renderer producing labelled multi-view PNG previews of a
//! model. No GPU or window needed, so it works inside the MCP server, in CI
//! and in the browser.

mod font;

use pd_core::build::{BuiltPart, Model};
use pd_core::math::{add, cross, dot, normalize, scale, sub, Bbox, V3};
use pd_core::mesh::{print_pose, Mesh};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Iso,
    IsoBack,
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl View {
    pub fn parse(s: &str) -> Option<View> {
        Some(match s.to_ascii_lowercase().as_str() {
            "iso" | "isometric" | "perspective" => View::Iso,
            "iso_back" | "iso-back" | "back_iso" => View::IsoBack,
            "front" => View::Front,
            "back" | "rear" => View::Back,
            "left" => View::Left,
            "right" | "side" => View::Right,
            "top" | "plan" => View::Top,
            "bottom" | "under" => View::Bottom,
            _ => return None,
        })
    }

    fn label(&self) -> &'static str {
        match self {
            View::Iso => "ISO (FRONT-RIGHT-TOP)",
            View::IsoBack => "ISO (BACK-LEFT-TOP)",
            View::Front => "FRONT (LOOKING +Y)",
            View::Back => "BACK (LOOKING -Y)",
            View::Left => "LEFT (LOOKING +X)",
            View::Right => "RIGHT (LOOKING -X)",
            View::Top => "TOP (LOOKING DOWN)",
            View::Bottom => "BOTTOM (LOOKING UP)",
        }
    }

    /// (forward, up) camera vectors.
    fn camera(&self) -> (V3, V3) {
        match self {
            View::Iso => (normalize([-1.0, 1.0, -0.82]), [0.0, 0.0, 1.0]),
            View::IsoBack => (normalize([1.0, -1.0, -0.82]), [0.0, 0.0, 1.0]),
            View::Front => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            View::Back => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
            View::Left => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            View::Right => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            View::Top => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]),
            View::Bottom => ([0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layout {
    /// Parts where they are when assembled.
    Assembled,
    /// Parts pushed apart from the centre.
    Exploded,
    /// Parts as they will sit on the print bed.
    Print,
}

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub views: Vec<View>,
    /// Pixel size of each view cell.
    pub cell: u32,
    pub layout: Layout,
    /// Cut along `axis = at` to show the inside (keeps y > at, x < at, z < at,
    /// so the front, right and top views look at the cut face).
    pub section: Option<(char, f64)>,
    /// Draw only this part in colour; others light grey.
    pub highlight: Option<String>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            views: vec![View::Iso, View::Front, View::Right, View::Top],
            cell: 420,
            layout: Layout::Assembled,
            section: None,
            highlight: None,
        }
    }
}

struct Tri {
    p: [V3; 3],
    n: V3,
    color: [f32; 3],
    cap: bool,
}

struct Scene {
    tris: Vec<Tri>,
    edges: Vec<(V3, V3, V3, V3)>, // a, b, n1, n2
    bbox: Bbox,
}

fn hex_color(s: &str) -> [f32; 3] {
    let h = s.trim_start_matches('#');
    let c = |i: usize| {
        u8::from_str_radix(h.get(i..i + 2).unwrap_or("80"), 16).unwrap_or(128) as f32 / 255.0
    };
    [c(0), c(2), c(4)]
}

fn part_solids<'a>(
    model: &'a Model,
    opts: &RenderOptions,
) -> Vec<(&'a BuiltPart, manifold_rust::manifold::Manifold)> {
    let parts: Vec<&BuiltPart> = model.parts.iter().collect();
    match opts.layout {
        Layout::Assembled => parts.iter().map(|p| (*p, p.solid.clone())).collect(),
        Layout::Exploded => {
            let all = parts
                .iter()
                .map(|p| Bbox::of(&p.solid))
                .filter(|b| b.is_valid())
                .reduce(|a, b| a.union(&b));
            let Some(all) = all else { return vec![] };
            let c = all.center();
            let size = all.size();
            parts
                .iter()
                .map(|p| {
                    let b = Bbox::of(&p.solid);
                    let d = sub(b.center(), c);
                    let dir = if dot(d, d) < 1e-6 {
                        [0.0, 0.0, 0.0]
                    } else {
                        normalize(d)
                    };
                    // spread along the direction away from the centre, proportional
                    // to the model's extent that way
                    let ext = (dir[0] * size[0]).abs()
                        + (dir[1] * size[1]).abs()
                        + (dir[2] * size[2]).abs();
                    let off = scale(dir, ext * 0.12 + 10.0);
                    (
                        *p,
                        p.solid
                            .translate(manifold_rust::linalg::Vec3::new(off[0], off[1], off[2])),
                    )
                })
                .collect()
        }
        Layout::Print => {
            let mut x = 0.0;
            let gap = 8.0;
            let posed: Vec<_> = parts.iter().map(|p| (*p, print_pose(p))).collect();
            let total: f64 = posed
                .iter()
                .map(|(_, m)| Bbox::of(m).size()[0] + gap)
                .sum::<f64>()
                - gap;
            let mut out = vec![];
            x -= total / 2.0;
            for (p, m) in posed {
                let s = Bbox::of(&m).size();
                out.push((
                    p,
                    m.translate(manifold_rust::linalg::Vec3::new(x + s[0] / 2.0, 0.0, 0.0)),
                ));
                x += s[0] + gap;
            }
            out
        }
    }
}

fn build_scene(model: &Model, opts: &RenderOptions) -> Scene {
    let mut tris = vec![];
    let mut edges = vec![];
    let mut bbox = Bbox {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    for (part, mut solid) in part_solids(model, opts) {
        let mut plane: Option<(usize, f64)> = None;
        if let Some((axis, at)) = opts.section {
            let i = match axis {
                'x' => 0,
                'y' => 1,
                _ => 2,
            };
            // keep the half facing away from the default cameras (front, right, top)
            // so the cut face is what they see
            let sign = if i == 1 { 1.0 } else { -1.0 };
            let mut n = [0.0; 3];
            n[i] = sign;
            solid = solid.trim_by_plane(
                manifold_rust::linalg::Vec3::new(n[0], n[1], n[2]),
                sign * at,
            );
            plane = Some((i, at));
        }
        if solid.is_empty() {
            continue;
        }
        let dim = opts.highlight.as_ref().is_some_and(|h| *h != part.id);
        let color = if dim {
            [0.86, 0.87, 0.89]
        } else {
            hex_color(&part.color)
        };
        let mesh = Mesh::of(&solid);
        let pos = |i: u32| -> V3 {
            let k = i as usize * 3;
            [
                mesh.positions[k] as f64,
                mesh.positions[k + 1] as f64,
                mesh.positions[k + 2] as f64,
            ]
        };
        let mut edge_map: HashMap<(u32, u32), Vec<V3>> = HashMap::new();
        for t in mesh.indices.chunks(3) {
            let p = [pos(t[0]), pos(t[1]), pos(t[2])];
            let n = normalize(cross(sub(p[1], p[0]), sub(p[2], p[0])));
            let cap = plane.is_some_and(|(i, at)| p.iter().all(|q| (q[i] - at).abs() < 1e-4));
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                edge_map.entry((a.min(b), a.max(b))).or_default().push(n);
            }
            for q in &p {
                for k in 0..3 {
                    bbox.min[k] = bbox.min[k].min(q[k]);
                    bbox.max[k] = bbox.max[k].max(q[k]);
                }
            }
            tris.push(Tri {
                p,
                n,
                color: if cap { [0.95, 0.35, 0.35] } else { color },
                cap,
            });
        }
        for ((a, b), ns) in edge_map {
            if ns.len() == 2 {
                edges.push((pos(a), pos(b), ns[0], ns[1]));
            }
        }
    }
    Scene { tris, edges, bbox }
}

struct Canvas {
    w: usize,
    h: usize,
    rgb: Vec<[f32; 3]>,
}

impl Canvas {
    fn new(w: usize, h: usize, bg: [f32; 3]) -> Canvas {
        Canvas {
            w,
            h,
            rgb: vec![bg; w * h],
        }
    }
    fn set(&mut self, x: i64, y: i64, c: [f32; 3]) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.rgb[y as usize * self.w + x as usize] = c;
        }
    }
    fn fill_rect(&mut self, x0: usize, y0: usize, w: usize, h: usize, c: [f32; 3]) {
        for y in y0..(y0 + h).min(self.h) {
            for x in x0..(x0 + w).min(self.w) {
                self.rgb[y * self.w + x] = c;
            }
        }
    }
    fn text(&mut self, x: usize, y: usize, s: &str, sc: usize, c: [f32; 3]) {
        let mut cx = x;
        for ch in s.chars() {
            if let Some(g) = font::glyph(ch) {
                for (row, bits) in g.iter().enumerate() {
                    for col in 0..5 {
                        if bits & (1 << (4 - col)) != 0 {
                            for dy in 0..sc {
                                for dx in 0..sc {
                                    self.set(
                                        (cx + col * sc + dx) as i64,
                                        (y + row * sc + dy) as i64,
                                        c,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            cx += 6 * sc;
        }
    }
    fn blit(&mut self, src: &Canvas, x0: usize, y0: usize) {
        for y in 0..src.h {
            for x in 0..src.w {
                self.set((x0 + x) as i64, (y0 + y) as i64, src.rgb[y * src.w + x]);
            }
        }
    }
}

const SS: usize = 2;

/// Render one view into a `size`x`size` canvas using a shared world scale.
fn render_view(scene: &Scene, view: View, size: usize, world_per_px: f64, center: V3) -> Canvas {
    let n = size * SS;
    let bg = [0.93, 0.94, 0.96];
    let mut img = Canvas::new(n, n, bg);
    let mut z = vec![f64::INFINITY; n * n];
    let (f, up0) = view.camera();
    let right = normalize(cross(f, up0));
    let up = cross(right, f);
    let s = 1.0 / world_per_px * SS as f64;
    let proj = |p: V3| -> (f64, f64, f64) {
        let d = sub(p, center);
        (
            n as f64 / 2.0 + dot(d, right) * s,
            n as f64 / 2.0 - dot(d, up) * s,
            dot(d, f),
        )
    };
    let light = normalize(add(
        add(scale(f, -0.75), scale(up, 0.5)),
        scale(right, -0.35),
    ));
    for t in &scene.tris {
        if dot(t.n, f) >= 0.0 {
            continue;
        }
        let shade = if t.cap {
            1.0
        } else {
            0.38 + 0.62 * dot(t.n, light).max(0.0) as f32
        };
        let col = [t.color[0] * shade, t.color[1] * shade, t.color[2] * shade];
        let a = proj(t.p[0]);
        let b = proj(t.p[1]);
        let c = proj(t.p[2]);
        let minx = a.0.min(b.0).min(c.0).floor().max(0.0) as usize;
        let maxx = (a.0.max(b.0).max(c.0).ceil() as usize).min(n - 1);
        let miny = a.1.min(b.1).min(c.1).floor().max(0.0) as usize;
        let maxy = (a.1.max(b.1).max(c.1).ceil() as usize).min(n - 1);
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area.abs() < 1e-12 || minx > maxx || miny > maxy {
            continue;
        }
        for y in miny..=maxy {
            for x in minx..=maxx {
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) / area;
                let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                    continue;
                }
                let depth = w0 * a.2 + w1 * b.2 + w2 * c.2;
                let k = y * n + x;
                if depth < z[k] {
                    z[k] = depth;
                    img.rgb[k] = col;
                }
            }
        }
    }
    // crease + silhouette edges
    let bias = world_per_px * 2.5;
    let crease = 25f64.to_radians().cos();
    for (a, b, n1, n2) in &scene.edges {
        let front1 = dot(*n1, f) < 0.0;
        let front2 = dot(*n2, f) < 0.0;
        let silhouette = front1 != front2;
        if !silhouette && (!front1 || dot(*n1, *n2) > crease) {
            continue;
        }
        let pa = proj(*a);
        let pb = proj(*b);
        let steps = ((pb.0 - pa.0).abs().max((pb.1 - pa.1).abs()).ceil() as usize).max(1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = pa.0 + (pb.0 - pa.0) * t;
            let y = pa.1 + (pb.1 - pa.1) * t;
            let d = pa.2 + (pb.2 - pa.2) * t;
            for (dx, dy) in [(0i64, 0i64), (1, 0), (0, 1), (1, 1)] {
                let xi = x as i64 + dx;
                let yi = y as i64 + dy;
                if xi < 0 || yi < 0 || xi as usize >= n || yi as usize >= n {
                    continue;
                }
                let k = yi as usize * n + xi as usize;
                if d <= z[k] + bias {
                    img.rgb[k] = [0.13, 0.14, 0.17];
                }
            }
        }
    }
    // axis triad
    let o = (36.0 * SS as f64, n as f64 - 36.0 * SS as f64);
    for (axis, color, label) in [
        ([1.0, 0.0, 0.0], [0.85, 0.15, 0.15], "X"),
        ([0.0, 1.0, 0.0], [0.15, 0.6, 0.2], "Y"),
        ([0.0, 0.0, 1.0], [0.15, 0.35, 0.9], "Z"),
    ] {
        let sx = dot(axis, right);
        let sy = -dot(axis, up);
        let len = 24.0 * SS as f64;
        if (sx * sx + sy * sy).sqrt() < 0.2 {
            continue;
        }
        for i in 0..(len as usize) {
            let t = i as f64 / len;
            for d in 0..SS * 2 {
                img.set(
                    (o.0 + sx * len * t) as i64 + d as i64 / 2,
                    (o.1 + sy * len * t) as i64 + d as i64 % 2,
                    color,
                );
            }
        }
        img.text(
            (o.0 + sx * len * 1.25 - 3.0 * SS as f64).max(0.0) as usize,
            (o.1 + sy * len * 1.25 - 4.0 * SS as f64).max(0.0) as usize,
            label,
            SS * 2,
            color,
        );
    }
    downsample(&img, SS)
}

fn downsample(src: &Canvas, f: usize) -> Canvas {
    let w = src.w / f;
    let h = src.h / f;
    let mut out = Canvas::new(w, h, [0.0; 3]);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            for dy in 0..f {
                for dx in 0..f {
                    let c = src.rgb[(y * f + dy) * src.w + x * f + dx];
                    for k in 0..3 {
                        acc[k] += c[k];
                    }
                }
            }
            let d = (f * f) as f32;
            out.rgb[y * w + x] = [acc[0] / d, acc[1] / d, acc[2] / d];
        }
    }
    out
}

fn fmt_mm(v: f64) -> String {
    pd_core::expr::fmt_num((v * 10.0).round() / 10.0)
}

/// Extent of the scene along the view's screen axes, as text.
fn view_dims(scene: &Scene, view: View) -> String {
    let s = scene.bbox.size();
    match view {
        View::Front | View::Back => format!("X {}  Z {} MM", fmt_mm(s[0]), fmt_mm(s[2])),
        View::Left | View::Right => format!("Y {}  Z {} MM", fmt_mm(s[1]), fmt_mm(s[2])),
        View::Top | View::Bottom => format!("X {}  Y {} MM", fmt_mm(s[0]), fmt_mm(s[1])),
        _ => format!("{} x {} x {} MM", fmt_mm(s[0]), fmt_mm(s[1]), fmt_mm(s[2])),
    }
}

/// Render the model to PNG bytes.
pub fn render_png(model: &Model, opts: &RenderOptions) -> Vec<u8> {
    let scene = build_scene(model, opts);
    let cell = opts.cell.max(160) as usize;
    let views = if opts.views.is_empty() {
        vec![View::Iso]
    } else {
        opts.views.clone()
    };
    let cols = if views.len() == 1 { 1 } else { 2 };
    let rows = views.len().div_ceil(cols);
    let header = 26;
    let footer = 22;
    let ch = cell + header + footer;
    let mut out = Canvas::new(
        cols * cell + (cols - 1) * 4,
        rows * ch + (rows - 1) * 4,
        [0.75, 0.77, 0.8],
    );
    let valid = scene.bbox.is_valid() && !scene.tris.is_empty();
    // one scale for every view so sizes compare directly
    let center = if valid { scene.bbox.center() } else { [0.0; 3] };
    let radius = if valid {
        (0..3)
            .map(|i| scene.bbox.size()[i])
            .map(|v| v * v)
            .sum::<f64>()
            .sqrt()
            / 2.0
    } else {
        1.0
    };
    let world_per_px = (radius * 2.0 * 1.12 / cell as f64).max(1e-6);
    for (i, v) in views.iter().enumerate() {
        let x0 = (i % cols) * (cell + 4);
        let y0 = (i / cols) * (ch + 4);
        out.fill_rect(x0, y0, cell, header, [0.16, 0.18, 0.22]);
        out.text(x0 + 8, y0 + 6, v.label(), 2, [1.0, 1.0, 1.0]);
        if valid {
            let img = render_view(&scene, *v, cell, world_per_px, center);
            out.blit(&img, x0, y0 + header);
        } else {
            out.fill_rect(x0, y0 + header, cell, cell, [0.93, 0.94, 0.96]);
            out.text(
                x0 + 20,
                y0 + header + cell / 2,
                "NOTHING TO SHOW",
                2,
                [0.6, 0.1, 0.1],
            );
        }
        out.fill_rect(x0, y0 + header + cell, cell, footer, [0.98, 0.98, 0.99]);
        if valid {
            out.text(
                x0 + 8,
                y0 + header + cell + 4,
                &view_dims(&scene, *v),
                2,
                [0.15, 0.15, 0.2],
            );
        }
    }
    encode_png(&out)
}

fn encode_png(c: &Canvas) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, c.w as u32, c.h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("png header");
        let data: Vec<u8> = c
            .rgb
            .iter()
            .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect();
        w.write_image_data(&data).expect("png data");
    }
    buf
}
