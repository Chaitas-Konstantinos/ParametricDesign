//! Exporters: binary STL and 3MF (multi-part, colours, arranged on the
//! plate, with the source design embedded so the file can be reopened and
//! edited).

use pd_core::build::{BuiltPart, Model};
use pd_core::math::Bbox;
use pd_core::mesh::{print_pose, Mesh};
use serde_json::Value as Json;
use std::io::Write;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pose {
    /// Where the parts sit in the assembled design.
    Assembled,
    /// Each part rotated for printing and resting on z = 0.
    Print,
}

fn posed(p: &BuiltPart, pose: Pose) -> manifold_rust::manifold::Manifold {
    match pose {
        Pose::Assembled => p.solid.clone(),
        Pose::Print => print_pose(p),
    }
}

/// Binary STL of the given parts merged into one file.
pub fn stl(parts: &[&BuiltPart], pose: Pose) -> Vec<u8> {
    let meshes: Vec<Mesh> = if pose == Pose::Print && parts.len() > 1 {
        arrange(parts, [1e9, 1e9])
            .into_iter()
            .map(|(m, _, _)| m)
            .collect()
    } else {
        parts.iter().map(|p| Mesh::of(&posed(p, pose))).collect()
    };
    let count: usize = meshes.iter().map(|m| m.triangle_count()).sum();
    let mut out = Vec::with_capacity(84 + count * 50);
    let mut header = [0u8; 80];
    let title = b"ParametricDesign export";
    header[..title.len()].copy_from_slice(title);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(count as u32).to_le_bytes());
    for m in &meshes {
        let p = |i: u32| -> [f32; 3] {
            let k = i as usize * 3;
            [m.positions[k], m.positions[k + 1], m.positions[k + 2]]
        };
        for t in m.indices.chunks(3) {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let mut n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 0.0 {
                n = [n[0] / l, n[1] / l, n[2] / l];
            }
            for vtx in [n, a, b, c] {
                for f in vtx {
                    out.extend_from_slice(&f.to_le_bytes());
                }
            }
            out.extend_from_slice(&[0, 0]);
        }
    }
    out
}

/// Lay parts out in rows on a bed of `bed` (x, y) mm, 6 mm apart, each in its
/// print pose. Returns (mesh centred on its slot, slot centre, plate index).
pub fn arrange(parts: &[&BuiltPart], bed: [f64; 2]) -> Vec<(Mesh, [f64; 2], usize)> {
    let gap = 6.0;
    let mut items: Vec<(usize, manifold_rust::manifold::Manifold, [f64; 3])> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let m = print_pose(p);
            let s = Bbox::of(&m).size();
            (i, m, s)
        })
        .collect();
    // tallest-in-y first gives tidier rows
    items.sort_by(|a, b| b.2[1].partial_cmp(&a.2[1]).unwrap());
    let mut placed: Vec<(usize, Mesh, [f64; 2], usize)> = vec![];
    let (mut x, mut y, mut row_h, mut plate) = (0.0, 0.0, 0.0f64, 0usize);
    for (i, m, s) in items {
        if x > 0.0 && x + s[0] > bed[0] {
            x = 0.0;
            y += row_h + gap;
            row_h = 0.0;
        }
        if y > 0.0 && y + s[1] > bed[1] {
            plate += 1;
            x = 0.0;
            y = 0.0;
            row_h = 0.0;
        }
        let cx = x + s[0] / 2.0;
        let cy = y + s[1] / 2.0;
        placed.push((i, Mesh::of(&m), [cx, cy], plate));
        x += s[0] + gap;
        row_h = row_h.max(s[1]);
    }
    placed.sort_by_key(|p| p.0);
    placed.into_iter().map(|(_, m, c, pl)| (m, c, pl)).collect()
}

pub struct ThreeMfOptions {
    pub title: String,
    /// Bed size (x, y, z) used to arrange parts.
    pub bed: [f64; 3],
    /// The source design, embedded so the file reopens editable.
    pub design: Option<Json>,
    /// Parameter values used for this export.
    pub params: Option<Json>,
    /// PNG shown as the file's thumbnail by slicers and file browsers.
    pub thumbnail_png: Option<Vec<u8>>,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 3MF package: one object per part with its colour, arranged on the plate
/// in print orientation. Plates beyond the first are laid out to the right.
pub fn three_mf(model: &Model, opts: &ThreeMfOptions) -> Result<Vec<u8>, String> {
    let parts: Vec<&BuiltPart> = model.parts.iter().filter(|p| !p.solid.is_empty()).collect();
    let layout = arrange(&parts, [opts.bed[0], opts.bed[1]]);
    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n");
    x.push_str(&format!(
        " <metadata name=\"Title\">{}</metadata>\n",
        xml_escape(&opts.title)
    ));
    x.push_str(" <metadata name=\"Application\">ParametricDesign</metadata>\n");
    x.push_str(" <resources>\n  <basematerials id=\"1\">\n");
    for p in &parts {
        x.push_str(&format!(
            "   <base name=\"{}\" displaycolor=\"{}FF\"/>\n",
            xml_escape(&p.id),
            p.color.to_uppercase()
        ));
    }
    x.push_str("  </basematerials>\n");
    for (i, (p, (mesh, _, _))) in parts.iter().zip(layout.iter()).enumerate() {
        x.push_str(&format!(
            "  <object id=\"{}\" type=\"model\" name=\"{}\" pid=\"1\" pindex=\"{}\">\n   <mesh>\n    <vertices>\n",
            i + 2,
            xml_escape(p.name.as_deref().unwrap_or(&p.id)),
            i
        ));
        for v in mesh.positions.chunks(3) {
            x.push_str(&format!(
                "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                v[0], v[1], v[2]
            ));
        }
        x.push_str("    </vertices>\n    <triangles>\n");
        for t in mesh.indices.chunks(3) {
            x.push_str(&format!(
                "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
                t[0], t[1], t[2]
            ));
        }
        x.push_str("    </triangles>\n   </mesh>\n  </object>\n");
    }
    x.push_str(" </resources>\n <build>\n");
    for (i, (_, c, plate)) in layout.iter().enumerate() {
        let ox = *plate as f64 * (opts.bed[0] + 30.0);
        x.push_str(&format!(
            "  <item objectid=\"{}\" transform=\"1 0 0 0 1 0 0 0 1 {} {} 0\"/>\n",
            i + 2,
            c[0] + ox,
            c[1]
        ));
    }
    x.push_str(" </build>\n</model>\n");

    let mut rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n");
    if opts.thumbnail_png.is_some() {
        rels.push_str(" <Relationship Target=\"/Metadata/thumbnail.png\" Id=\"rel1\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail\"/>\n");
    }
    rels.push_str("</Relationships>\n");
    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n <Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\n <Default Extension=\"png\" ContentType=\"image/png\"/>\n <Default Extension=\"json\" ContentType=\"application/json\"/>\n</Types>\n";

    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut put = |name: &str, data: &[u8]| -> Result<(), String> {
            z.start_file(name, o).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())
        };
        put("[Content_Types].xml", types.as_bytes())?;
        put("_rels/.rels", rels.as_bytes())?;
        put("3D/3dmodel.model", x.as_bytes())?;
        if let Some(png) = &opts.thumbnail_png {
            put("Metadata/thumbnail.png", png)?;
        }
        if let Some(d) = &opts.design {
            let embedded = serde_json::json!({ "design": d, "params": opts.params.clone().unwrap_or(Json::Null) });
            put(
                "Metadata/parametric_design.json",
                serde_json::to_string_pretty(&embedded).unwrap().as_bytes(),
            )?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

/// Read the design embedded in a 3MF written by [`three_mf`].
pub fn embedded_design(bytes: &[u8]) -> Option<Json> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut f = z.by_name("Metadata/parametric_design.json").ok()?;
    let mut s = String::new();
    std::io::Read::read_to_string(&mut f, &mut s).ok()?;
    serde_json::from_str(&s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        let j = serde_json::json!({
            "format": "pd/0.1", "meta": {"name": "t"},
            "parts": [
                {"id": "a", "shape": {"type": "box", "size": [10, 10, 10]}},
                {"id": "b", "shape": {"type": "cylinder", "d": 8, "h": 5}, "place": {"on": "a.top"}}
            ]
        });
        let (d, issues) = pd_core::parse(&j);
        assert!(issues.is_empty());
        pd_core::build(&d.unwrap(), &Default::default())
    }

    #[test]
    fn stl_has_all_triangles() {
        let m = model();
        let parts: Vec<&BuiltPart> = m.parts.iter().collect();
        let bytes = stl(&parts, Pose::Assembled);
        let n = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 84 + n * 50);
        assert!(n >= 12 + 20);
    }

    #[test]
    fn three_mf_roundtrips_design() {
        let m = model();
        let design = serde_json::json!({"hello": 1});
        let bytes = three_mf(
            &m,
            &ThreeMfOptions {
                title: "t".into(),
                bed: [200.0, 200.0, 200.0],
                design: Some(design.clone()),
                params: None,
                thumbnail_png: None,
            },
        )
        .unwrap();
        assert_eq!(embedded_design(&bytes).unwrap()["design"], design);
    }
}
