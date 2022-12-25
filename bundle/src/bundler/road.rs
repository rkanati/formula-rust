use {
    crate::{bundle, bundler::atlas::Atlas},
    anyhow::Result as Anyhow,
    bytemuck::pod_read_unaligned as read,
};

fn parse_trv(trv: &[u8]) -> Vec<[i32; 3]> {
    trv.array_chunks().copied()
        .map(i32::from_be_bytes)
        .array_chunks()
        .map(|[x, y, z, _]| [x, y, z])
        .collect()
}

#[derive(Debug)]
struct RawFace {
    verts:  [u16; 4],
    _normal: [i16; 3],
    tex:    u8,
    flags:  u8,
    colour: [u8; 3],
}

fn parse_trf(trf: &[u8]) -> Vec<RawFace> {
    trf.array_chunks()
        .map(|raw: &[u8; 20]| {
            //let verts = bytemuck::cast_slice(&bs[0..8]).map(u16::from_be);
            let verts = read::<[u16; 4]>(&raw[0..8]).map(u16::from_be);
            let _normal = read::<[i16; 3]>(&raw[8..14]).map(i16::from_be);
            let tex = raw[14];
            let flags = raw[15];
            let colour = read(&raw[16..19]);
            RawFace{verts, _normal, tex, flags, colour}
        })
        .collect()
}


pub fn make_mesh(trv: &[u8], trf: &[u8], atlas: &Atlas) -> Anyhow<bundle::Mesh> {
    let verts = parse_trv(&trv);
    let faces = parse_trf(&trf);

    let verts = faces.iter()
        .flat_map(|face| {
            let [r, g, b] = face.colour;
            let rgb = [r, g, b, 255];

            let [u0, v0, u1, v1]: [f32; 4] = atlas.lookup_rect(face.tex as usize).into();
            let uvs = [[u1, v0], [u0, v0], [u0, v1], [u1, v1]];
            let uvis =
                if face.flags & 4 == 0 { [0, 1, 2, 3] }
                else                   { [1, 0, 3, 2] };
            let uvs = uvis.map(|i| uvs[i]);

            face.verts
                .map(|vi| verts[vi as usize])
                .zip(uvs)
                .map(|(xyz, uv)| bundle::MeshVert{xyz, rgb, uv})
        })
        .collect();

    let idxs = (0 .. faces.len() as u32)
        .flat_map(|face_i| [0, 1, 2, 0, 2, 3].map(|i| i + face_i * 4))
        .collect();

    //let mesh = bundle_out.meshes.add(asset_path, bundle::Mesh{verts, idxs});
    Ok(bundle::Mesh{verts, idxs})
}

