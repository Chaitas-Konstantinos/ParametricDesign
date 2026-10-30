//! Triangle meshes for viewers and exporters.

use crate::build::BuiltPart;
use crate::math::{Bbox, Xf};
use manifold_rust::linalg::Vec3;
use manifold_rust::manifold::Manifold;

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    /// x, y, z per vertex.
    pub positions: Vec<f32>,
    /// 3 vertex indices per triangle, counter-clockwise seen from outside.
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn of(m: &Manifold) -> Mesh {
        let g = m.get_mesh_gl(-1);
        let np = g.num_prop as usize;
        let nv = g.vert_properties.len() / np.max(1);
        let mut positions = Vec::with_capacity(nv * 3);
        for v in 0..nv {
            positions.extend_from_slice(&g.vert_properties[v * np..v * np + 3]);
        }
        Mesh {
            positions,
            indices: g.tri_verts.clone(),
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// The part as it sits on the build plate: print rotation applied, lowest
/// point on z = 0, centred on X/Y.
pub fn print_pose(p: &BuiltPart) -> Manifold {
    let r = Xf::rotate_xyz(p.print_rotate);
    let m = r.apply(&p.solid);
    let b = Bbox::of(&m);
    if !b.is_valid() {
        return m;
    }
    let c = b.center();
    m.translate(Vec3::new(-c[0], -c[1], -b.min[2]))
}
