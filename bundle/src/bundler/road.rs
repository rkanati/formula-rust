use {
    crate::{bundler::atlas::Atlas, un16, be::Be},
    std::rc::Rc,
    anyhow::Result as Anyhow,
    bytemuck as bm,
    ultraviolet as uv,
};

pub fn make_road(trv: &[u8], trf: &[u8], trs: &[u8], atlas: &Atlas)
    -> Anyhow<(Rc<crate::Mesh>, crate::TrackGraph)>
{
    let vs = bm::try_cast_slice(trv)?;
    let fs = bm::try_cast_slice(trf)?;
    let mesh = make_mesh(&vs, &fs, &atlas)?;
    let graph = make_graph(&trs, &vs, &fs);
    Ok((mesh, graph))
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, bm::AnyBitPattern)]
struct RawVert([Be<i32>; 4]);

impl From<RawVert> for [f32; 3] {
    fn from(RawVert([x,y,z,_]): RawVert) -> Self {
        [x,y,z].map(|x| x.get() as f32)
    }
}

impl From<RawVert> for uv::Vec3 {
    fn from(v: RawVert) -> Self {
        let xyz: [f32; 3] = v.into();
        xyz.into()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bm::AnyBitPattern)]
struct RawFace {
    verts:  [Be<u16>; 4],
    _normal: [Be<i16>; 3],
    tex:    u8,
    flags:  u8,
    colour: super::RawRgbx,
}

fn make_mesh(verts: &[RawVert], faces: &[RawFace], atlas: &Atlas) -> Anyhow<Rc<crate::Mesh>> {
    let verts = faces.iter()
        .flat_map(|face| {
            let rgb = face.colour.into();

            let uvs: [f32; 4] = atlas.lookup_rect(face.tex as usize).into();
            let [u0, v0, u1, v1]: [un16; 4] = uvs.map(un16::new);
            let uvs = [[u1, v0], [u0, v0], [u0, v1], [u1, v1]];
            let uvis =
                if face.flags & 4 == 0 { [0, 1, 2, 3] }
                else                   { [1, 0, 3, 2] };
            let uvs = uvis.map(|i| uvs[i]);

            face.verts
                .map(|vi| verts[vi.get() as usize])
                .zip(uvs)
                .map(|(xyz, uv)| crate::MeshVert{xyz: xyz.into(), rgb, uv})
        })
        .collect();

    let idxs = (0 .. faces.len() as u32)
        .flat_map(|face_i| [0, 1, 2, 0, 2, 3].map(|i| i + face_i * 4))
        .collect();

    //let mesh = bundle_out.meshes.add(asset_path, crate::Mesh{verts, idxs});
    Ok(Rc::new(crate::Mesh{verts, idxs}))
}

#[repr(C)]
#[derive(Clone, Copy, bm::AnyBitPattern)]
struct RawSection {
    junction: Be<u32>,      //    0 ..   4
    prev:     Be<u32>,      //    4 ..   8
    next:     Be<u32>,      //    8 ..  12
    centre:   [Be<i32>; 3], //   12 ..  24
    _pad0:    [u8; 116],    //   24 .. 140
    face_i0:  Be<u32>,      //  140 .. 144
    n_faces:  Be<u16>,      //  144 .. 146
    _pad1:    [u8; 4],      //  146 .. 150
    flags:    Be<u16>,      //  150 .. 152
    _pad3:    [u8; 4],      //  152 .. 156
}

fn make_graph(trs: &[u8], verts: &[RawVert], faces: &[RawFace]) -> crate::TrackGraph {
    let raw_sections: &[RawSection] = bm::cast_slice(trs);
    raw_sections.iter()
        .map(|rs| {
            let i0 = rs.face_i0.get() as usize;
            let nf = rs.n_faces.get() as usize;

            /*let mut n = 0;
            let mut p = uv::Vec3::zero();
            for i in i0 .. i0+nf {
                let face = &faces[i as usize];
                if face.flags & 1 == 0 {continue;}
                let q = face.verts.into_iter()
                    .map(|vi| verts[vi.get() as usize].into())
                    .sum::<uv::Vec3>();
                p += q;
                n += 4;
            }
            let center = p / n as f32;*/

            let center = (i0 .. i0+nf)
                .filter_map(|i| {
                    let face = &faces[i];
                    if (face.flags & 1) == 0 {return None}
                    let p = face.verts.into_iter()
                        .map(|vi| {
                            let p: uv::Vec3 = verts[vi.get() as usize].into();
                            p.into_homogeneous_point()
                        })
                        .sum::<uv::Vec4>();
                    Some(p)
                })
                .sum::<uv::Vec4>()
                .normalized_homogeneous_point()
                .xyz()
                .into();

            let prev = rs.prev.get();
            let junc = rs.junction.get();
            let junc = if junc != !0u32 && (raw_sections[junc as usize].flags.get() & 0x10 != 0) {
                junc
            }
            else { !0u32 };
            let next = [rs.next.get(), junc];
            crate::TrackNode{prev, next, center}
        })
        .collect()
}

