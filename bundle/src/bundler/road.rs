use {
    crate::be::Be,
    anyhow::Result as Anyhow,
    bytemuck as bm,
    ultraviolet as uv,
};

pub fn make_road(trv: &[u8], trf: &[u8], trs: &[u8])
    -> Anyhow<(crate::RoadModel, crate::TrackGraph)>
{
    let vs = bm::try_cast_slice(trv)?;
    let fs = bm::try_cast_slice(trf)?;
    let model = make_model(&vs, &fs)?;
    let graph = make_graph(&trs, &vs, &fs);
    Ok((model, graph))
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

fn make_model(verts: &[RawVert], faces: &[RawFace]) -> Anyhow<crate::RoadModel> {
    let verts = verts.iter().copied().map(Into::into).collect();

    let mut f_verts = Vec::with_capacity(faces.len());
    let mut f_tex   = Vec::with_capacity(faces.len());
    let mut f_flags = Vec::with_capacity(faces.len());
    let mut f_rgb   = Vec::with_capacity(faces.len());

    for face in faces {
        f_verts.push(face.verts.map(Be::get));
        f_tex.push(face.tex);
        f_flags.push(face.flags);
        f_rgb.push(face.colour.into());
    }

    Ok(crate::RoadModel{verts, f_verts, f_tex, f_flags, f_rgb})
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

