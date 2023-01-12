use crate::PathSeg;

pub enum Error {
    NotAFont,
    Other(anyhow::Error)
}

impl From<anyhow::Error> for Error {
    fn from(v: anyhow::Error) -> Self {
        Self::Other(v)
    }
}

pub fn make_font(ttf: &[u8]) -> Result<crate::Font, Error> {
    let font = ttf_parser::Face::parse(ttf, 0);
    if let Err(ttf_parser::FaceParsingError::UnknownMagic) = font {
        return Err(Error::NotAFont);
    }
    let font = font.map_err(|e| Error::Other(e.into()))?;

    let mut points = Vec::new();
    let mut paths  = Vec::new();
    let mut glyphs = Vec::new();

    for ch in crate::GLYPH_RANGES.iter().cloned().flatten() {
        glyphs.push(crate::Glyph {
            start: paths.len() as u32,
            offset: [0.; 2],
            advance: 0.,
        });

        let Some(glyph_id) = font.glyph_index(ch) else {continue};
        let mut outliner = Outliner {
            scale:  1. / font.height() as f32,
            points: &mut points,
            paths:  &mut paths
        };

        let Some(_bbox) = font.outline_glyph(glyph_id, &mut outliner) else {continue};

        let scale = 1. / font.units_per_em() as f32;
        let advance = font.glyph_hor_advance(glyph_id)
            .map_or(1., |ha| ha as f32 * scale);

        glyphs.last_mut().unwrap().advance = advance;
    }

    Ok(crate::Font{points, paths, glyphs})
}

struct Outliner<'a> {
    scale: f32,
    points: &'a mut Vec<[f32; 2]>,
    paths:  &'a mut Vec<PathSeg>,
}

impl Outliner<'_> {
    fn add(&mut self, ps: &[[f32; 2]]) {
        self.points.extend(ps.iter().copied().map(|[x, y]| [x * self.scale, y * self.scale]));
    }
}

impl<'a> ttf_parser::OutlineBuilder for Outliner<'a> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.add(&[[x, y]]);
        self.paths.push(PathSeg::Start);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.add(&[[x, y]]);
        self.paths.push(PathSeg::Line);
    }

    fn quad_to(&mut self, xc: f32, yc: f32, x: f32, y: f32) {
        self.add(&[[xc, yc], [x, y]]);
        self.paths.push(PathSeg::Quadratic);
    }

    fn curve_to(&mut self, xc0: f32, yc0: f32, xc1: f32, yc1: f32, x: f32, y: f32) {
        self.add(&[[xc0, yc0], [xc1, yc1], [x, y]]);
        self.paths.push(PathSeg::Cubic);
    }

    fn close(&mut self) { }
}

/*pub enum Error {
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
}*/

