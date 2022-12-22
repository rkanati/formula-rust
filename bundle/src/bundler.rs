mod prm;

use {
    crate::bundle,
    anyhow::Result as Anyhow,
    bytemuck::pod_read_unaligned as read,
    camino::Utf8Path as Path,
    image::GenericImage,
    pack_rects::prelude::*,
    ultraviolet as uv,
    util::row_major,
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

struct MipMapping {
    hi: [u16; 16],
  /*md: [u16;  4],
    lo: [u16;  1],*/
}

fn parse_ttf(ttf: &[u8]) -> Vec<MipMapping> {
    ttf.array_chunks().copied()
        .map(u16::from_be_bytes)
        .array_chunks()
        .map(|is: [u16; 21]| MipMapping {
            hi: is[0..16].try_into().unwrap(),
          /*md: is[16..20].try_into().unwrap(),
            lo: [is[20]],*/
        })
        .collect()
}

fn make_atlas(
    images: &[image::RgbaImage],
    mip_mappings: &[MipMapping],
) -> Anyhow<(bundle::Image, Vec<[f32; 4]>)>
{
    // TODO generate mipmaps; padding etc

    // allocate atlas regions for textures
    let mut packer = RectPacker::new([1024; 2]);
    let rects = mip_mappings.iter()
        .map(|mip_mapping| {
            let fragments = mip_mapping.hi.map(|i| &images[i as usize]);
            assert!(fragments.iter().all(|frag| frag.dimensions() == fragments[0].dimensions()));
            let w = fragments[0].width() as i32 * 4;
            let h = fragments[0].height() as i32 * 4;
            let rect = packer.pack_now([w, h]).unwrap();
            rect
        })
        .collect::<Vec<_>>();

    // make atlas no larger than necessary
    let [wide, high] = rects.iter()
        .map(|rect| rect.maxs())
        .reduce(|a, b| a.zip(b).map(|(a, b)| a.max(b)))
        .unwrap()
        .map(|x| x as u32);

    // uv-coordinate scales based on atlas size
    let uv_scale = uv::Vec4::from([wide, high, wide, high].map(|x| 1. / x as f32));
    let mut atlas = image::RgbaImage::from_pixel(wide, high, image::Rgba([255,0,255,255]));

    // blit texture fragments into atlas
    let uvs = rects.iter().enumerate()
        .map(|(texture_i, &rect)| {
            let dims = uv::IVec2::from(rect.dims()) / 4;
            let mins = uv::IVec2::from(rect.mins());

            row_major(0..4, 0..4)
                .map(|(i, j)| uv::IVec2::new(i, j) * dims + mins)
                .zip(mip_mappings[texture_i].hi)
                .for_each(|(pos, image_i)| {
                    let src = &images[image_i as usize];
                    atlas.copy_from(src, pos.x as u32, pos.y as u32);
                });

            let rect = uv::IVec4::from(rect.corners());
            (uv::Vec4::from(rect) * uv_scale).into()
        })
        .collect::<Vec<_>>();

    let image = bundle::Image {
        wide: wide as u16,
        high: high as u16,
        levels: vec![atlas.into_raw()],
    };

    Ok((image, uvs))
}

fn make_mesh(
    uvs: &[[f32; 4]],
    verts: &[[i32; 3]],
    faces: &[RawFace],
) -> Anyhow<bundle::Mesh>
{
    let verts = faces.iter()
        .flat_map(|face| {
            let [r, g, b] = face.colour;
            let rgb = [r, g, b, 255];

            let [u0, v0, u1, v1] = uvs[face.tex as usize];
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

fn make_track(
    bundle_out: &mut bundle::Root,
    track_asset_path: &str,
    trv: &[u8],
    trf: &[u8],
    lib_cmp: &[u8],
    lib_ttf: &[u8],
) -> Anyhow<bundle::Id/*<bundle::Track>*/>
{
    let start = std::time::Instant::now();

    let lib_images = formats::load_cmp(lib_cmp)?;
    let lib_ttf = parse_ttf(lib_ttf);
    let (image, uvs) = make_atlas(&lib_images, &lib_ttf)?;

    let atlas_done = std::time::Instant::now();

    let verts = parse_trv(trv);
    let faces = parse_trf(trf);
    let mesh = make_mesh(&uvs, &verts, &faces)?;

    let mesh_done = std::time::Instant::now();

    let atlas_path = format!("{track_asset_path}/library");
    let image = bundle_out.images.add(&atlas_path, image);
    let atlas = bundle_out.atlases.add(&atlas_path, bundle::Atlas{image, uvs});
    let mesh = bundle_out.meshes.add(track_asset_path, mesh);
    let track = bundle_out.tracks.add(track_asset_path, bundle::Track{atlas, mesh});

    eprintln!("BUNDLER: atlas built in {:.3}s", (atlas_done - start).as_secs_f32());
    eprintln!("BUNDLER: mesh built in {:.3}s", (mesh_done - atlas_done).as_secs_f32());

    Ok(track)
}

pub fn make_bundle(wipeout_dir: &Path) -> Anyhow<Vec<u8>> {
    let mut bundle = bundle::Root::new();

    let bundle_start = std::time::Instant::now();

    // tracks
    for track_i in 1..=15 {
        let track_name = format!("track{track_i:02}");
        let track_dir = wipeout_dir.join(&track_name);
        eprintln!("BUNDLER: bundling {track_dir}");

        let start = std::time::Instant::now();

        let verts = std::fs::read(track_dir.join("track.trv"))?;
        eprintln!("BUNDLER: trv hash {:08x}", util::fnv1a_64(&verts));
        let faces = std::fs::read(track_dir.join("track.trf"))?;
        eprintln!("BUNDLER: trf hash {:08x}", util::fnv1a_64(&faces));
        let lib_cmp = std::fs::read(track_dir.join("library.cmp"))?;
        eprintln!("BUNDLER: library.cmp hash {:08x}", util::fnv1a_64(&lib_cmp));
        let lib_ttf = std::fs::read(track_dir.join("library.ttf"))?;
        eprintln!("BUNDLER: library.ttf hash {:08x}", util::fnv1a_64(&lib_ttf));
        make_track(&mut bundle, &track_name, &verts, &faces, &lib_cmp, &lib_ttf)?;

        let bundled = std::time::Instant::now();

        eprintln!("BUNDLER: took {:.3}s", (bundled - start).as_secs_f32());
    }

    let bundle_end = std::time::Instant::now();
    eprintln!("BUNDLER: all tracks in {:.3}s", (bundle_end - bundle_start).as_secs_f32());

    // finalize
    let mut buffer = Vec::with_capacity(16 << 20);
    bundle.bake(&mut buffer)?;
    let bake_end = std::time::Instant::now();
    eprintln!("BUNDLER: baked in {:.3}s", (bake_end - bundle_end).as_secs_f32());

    let compressed = lz4_flex::compress_prepend_size(&buffer);
    eprintln!("BUNDLER: compressed in {:.3}s", bake_end.elapsed().as_secs_f32());

    Ok(compressed)
}

