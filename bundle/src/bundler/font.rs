pub enum Error {
    NotAFont,
    Other(anyhow::Error)
}

pub fn make_font(ttf: &[u8], dbg_name: &str) -> Result<crate::Font, Error> {
    let font = ttf_parser::Face::parse(ttf, 0);
    if let Err(ttf_parser::FaceParsingError::UnknownMagic) = font {
        return Err(Error::NotAFont);
    }

    log::info!(target: "font", "making '{dbg_name}'");

    let font = font.map_err(|e| Error::Other(e.into()))?;

    let font_names = font.names();
    for s in font_names {
        let Some(s) = s.to_string() else {continue};
        log::debug!(target: "font", "font name string: {s}");
    }

    let gen = trianglyph::MeshGenerator::new_with_config(
        &font,
        trianglyph::Config{tolerance: 0.001, extrude: true}
    );

    let codepoints = (' ' ..= '~')
        .chain(['©', '®', '™']);

    let mut verts = Vec::new();
    let mut idxs  = Vec::new();

    let glyphs = codepoints
        .filter_map(|ch| {
            let glyph = font.glyph_index(ch)?;
            log::trace!(target: "font", "'{ch}' => {glyph:?}");

            let mut mesh = match gen.generate_mesh(glyph) {
                Ok(mesh) => mesh,
                Err(e) => {
                    log::error!(target: "font", "meshgen: '{ch}' ({glyph:?}): {e}");
                    return None;
                }
            };
            let v_base = verts.len() as u32;
            let start = idxs.len() as u32;
            let count = mesh.indices.len() as u32;
            verts.append(&mut mesh.vertices);
            idxs.extend(mesh.indices.into_iter().map(|i| v_base + i));

            let offset = [0.; 2];
            let scale = 1. / font.units_per_em() as f32;
            let advance = font.glyph_hor_advance(glyph)
                .map_or(1., |ha| ha as f32 * scale);

            Some((ch, crate::Glyph{start, count, offset, advance}))
        })
        .collect();

    Ok(crate::Font{verts, idxs, glyphs})
}

