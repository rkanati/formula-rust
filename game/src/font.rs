use {
    crate::{
        gl::prelude::*,
        render::{Attrib, make_arrays, BasicShader},
    },
    std::collections::hash_map::{self, HashMap},
    ultraviolet as uv,
    anyhow::Result as Anyhow,
    lyon_tessellation::{self as lt, path as ltp, path::builder as ltpb},
};

pub struct Font {
    glyphs: HashMap<char, Glyph>,
    vao: GLuint,
    tex: GLuint,
}

struct Glyph {
    start: usize,
    count: usize,
    advance: f32,
}

impl Font {
    pub fn load(gl: &Gl, font: &bundle::ArchivedFont, blank_tex: GLuint) -> Anyhow<Font> {
        const TOLERANCE: f32 = 0.001;

        let new_builder = ||
            ltpb::NoAttributes::wrap(
                ltp::path::BuilderImpl::new()
            )
            .flattened(TOLERANCE);
            //.transformed(lt::geom::Scale::new(1.)); // TODO scale to height

        let points = &mut &font.points[..];
        let build_path = |(glyph_i, ch, path): (usize, char, &[bundle::ArchivedPathSeg])| {
            let mut builder = new_builder();
            let mut open = false;
            for seg in path.iter() {
                use bundle::ArchivedPathSeg as Ps;
                *points = match (seg, *points) {
                    (Ps::Start, &[p, ref rest@..]) => {
                        if open { builder.close(); }
                        builder.begin(p.into());
                        open = true;
                        rest
                    }

                    (Ps::Line, &[p, ref rest@..]) => {
                        builder.line_to(p.into()); rest
                    }

                    (Ps::Quadratic, &[c0, p, ref rest@..]) => {
                        builder.quadratic_bezier_to(c0.into(), p.into()); rest
                    }

                    (Ps::Cubic, &[c0, c1, p, ref rest@..]) => {
                        builder.cubic_bezier_to(c0.into(), c1.into(), p.into()); rest
                    }

                    _ => unreachable!()
                };
            }
            builder.close();
            (glyph_i, ch, builder.build())
        };

        let mut verts = Vec::new();
        let mut idxs  = Vec::new();

        let glyphs = bundle::GLYPH_RANGES.iter().cloned()
            .flatten()
            .zip(font.glyphs.iter())
            .enumerate()
            .filter_map(|(glyph_i, (ch, glyph))| {
                let start = glyph.start as usize;
                let end = font.glyphs.get(glyph_i + 1)
                    .map(|g| g.start as usize)
                    .unwrap_or(font.paths.len());
                if start == end {return None};
                Some((glyph_i, ch, &font.paths[glyph.start as usize .. end]))
            })
            .map(build_path)
            .map({
                let opts = lt::FillOptions::default()
                    .with_fill_rule(lt::FillRule::NonZero)
                    .with_tolerance(TOLERANCE);

                let verts = &mut verts;
                let idxs  = &mut idxs;

                move |(glyph_i, ch, path)| {
                    let mut tess = lt::FillTessellator::new();
                    let mut mesh_builder = mesh_builder::MeshBuilder::default();
                    tess.tessellate_path(&path, &opts, &mut mesh_builder).unwrap();
                    let idx_range = mesh_builder.bake(verts, idxs);
                    let start = idx_range.start;
                    let count = idx_range.end - start;
                    let advance = font.glyphs[glyph_i].advance;
                    (ch, Glyph{start, count, advance})
                }
            })
            .collect();

        let vao = make_arrays(
            gl,
            &verts,
            &idxs,
            &[  Attrib::Float{n: 3, off: 0},
                Attrib::ConstFloat3(uv::Vec3::new(1., 1., 0.)),
                Attrib::ConstFloat3(uv::Vec3::one()),
            ]
        );

        Ok(Font{glyphs, vao, tex: blank_tex})
    }

    pub fn draw_glyphs_test(&self, gl: &Gl, mut pos: uv::Vec3) {
        let scale = 10.;

        let test = "\
            abcdefghijklmnopqrstuvwxyz\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\
            0123456789!\"£$%^&*()-_=+[]{};'#:@~,./<>?\\`\
            ©®™\
            ";

        unsafe {
            gl.Uniform3f(5, scale * 10., scale * -10., scale);
            gl.BindVertexArray(self.vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);
            for ch in test.chars() {
                if let Some(glyph) = self.glyphs.get(&ch) {
                    gl.Uniform3f(4, pos.x, pos.y, pos.z);
                    gl.DrawElements(
                        gl::TRIANGLES,
                        glyph.count as _,
                        gl::UNSIGNED_INT,
                        (glyph.start * 4) as _
                    );
                    pos.x += scale * 10. * glyph.advance;
                }
            }
        }
    }

    pub fn bake_run(&self, anchor: Anchor, text: &str) -> TextRun {
        let run = text.chars()
            .filter_map(|ch| {
                let glyph = self.glyphs.get(&ch)?;
                Some((glyph.start, glyph.count, glyph.advance))
            })
            .collect::<Vec<_>>();
        let w: f32 = run.iter().copied()
            .map(|(_, _, adv)| adv)
            .sum();
        let off = match anchor {
            Anchor::LowerLeft => uv::Vec2::zero(),
            Anchor::Center    => uv::Vec2::new(-w * 0.5, -0.5),
        };
        TextRun{vao: self.vao, tex: self.tex, run, off}
    }
}

mod mesh_builder {
    use {super::*, lt::VertexId as VId};

    #[derive(Default)]
    pub struct MeshBuilder {
        verts: Vec<PreVertex>,
        tris:  Vec<[u32; 3]>,
        edges: HashMap<(u32, u32), Edge>,
    }

    #[derive(Clone, Copy)]
    struct PreVertex {
        position: uv::Vec2,
        on_curve: bool,
    }

    #[derive(Clone, Copy)]
    struct Edge(u32, u32);

    impl Edge {
        fn key(self) -> (u32, u32) {
            (self.0.min(self.1), self.0.max(self.1))
        }
    }

    #[repr(C)]
    pub struct Vertex {
        pub position: [f32; 3],
        pub normal:   [f32; 3],
    }

    impl MeshBuilder {
        pub fn bake(self, verts_out: &mut Vec<Vertex>, idxs_out: &mut Vec<u32>)
            -> std::ops::Range<usize>
        {
            let first_idx = idxs_out.len();

            let make_vertex = |z| move |pv: &PreVertex| {
                let [x, y]: [f32; 2] = pv.position.into();
                let position = [x, y, 0.5*z];
                let normal = [0., 0., z];
                Vertex{position, normal}
            };

            // front face first
            let first_front_vert = self.verts.len() as u32;
            self.verts.iter()
                .map(make_vertex(1.))
                .collect_into(verts_out);

            self.tris.iter()
                .flat_map(|&[a, b, c]| [a, b, c].map(|i| first_front_vert + i))
                .collect_into(idxs_out);
            let front_end = idxs_out.len();

            // back face
            let first_back_vert = self.verts.len() as u32;
            self.verts.iter()
                .map(make_vertex(-1.))
                .collect_into(verts_out);

            self.tris.iter()
                .flat_map(|&[a, b, c]| [c, b, a].map(|i| first_back_vert + i))
                .collect_into(idxs_out);

            let first_sides_vert = self.verts.len() as u32;

            // sides
            let mut smooth_pairs = HashMap::<u32, u32>::new();
            for &Edge(ai, bi) in self.edges.values() {
                let normal = {
                    let a = self.verts[ai as usize];
                    let b = self.verts[bi as usize];
                    let [dx, dy]: [f32; 2] = (b.position - a.position).into();
                    [-dy, dx, 0.]
                };

                let [ai, bi] = [ai, bi].map(|pi| {
                    let p = self.verts[pi as usize];
                    let existing_pair =
                        if p.on_curve { smooth_pairs.get(&pi).copied() }
                        else          { None };

                    existing_pair
                        .map(|i| {
                            let [vf, vb] = verts_out
                                .get_many_mut([0, 1].map(|d| i as usize + d))
                                .unwrap(); // TODO
                            let sum = uv::Vec3::from(vf.normal) + uv::Vec3::from(normal);
                            let normal = sum.normalized().into();
                            vf.normal = normal;
                            vb.normal = normal;
                            i
                        })
                        .unwrap_or_else(|| {
                            let i = verts_out.len() as u32;
                            let [x, y]: [f32; 2] = p.position.into();
                            verts_out.push(Vertex{position: [x, y,  0.5], normal});
                            verts_out.push(Vertex{position: [x, y, -0.5], normal});
                            if p.on_curve { smooth_pairs.insert(pi, i); }
                            i
                        })
                });

                idxs_out.extend_from_slice(&[ai, bi, bi+1, ai, bi+1, ai+1]);
            }

            let end_idx = idxs_out.len();
            first_idx .. front_end//end_idx
        }
    }

    impl lt::FillGeometryBuilder for MeshBuilder {
        fn add_fill_vertex(&mut self, vertex: lt::FillVertex)
            -> Result<VId, lt::GeometryBuilderError>
        {
            let id = self.verts.len() as u32;
            let xy: [f32; 2] = vertex.position().into();
            let position = uv::Vec2::from(xy);
            let on_curve = vertex.as_endpoint_id().is_none();
            self.verts.push(PreVertex{position, on_curve});
            Ok(id.into())
        }
    }

    impl lt::GeometryBuilder for MeshBuilder {
        fn add_triangle(&mut self, VId(a): VId, VId(b): VId, VId(c): VId) {
            self.tris.push([a, b, c]);
            for &[pi, qi] in [a, b, c, a].array_windows() {
                let edge = Edge(pi, qi);
                match self.edges.entry(edge.key()) {
                    hash_map::Entry::Occupied(e) => { e.remove(); }
                    hash_map::Entry::Vacant(e)   => { e.insert(edge); }
                }
            }
        }
    }
}

/*pub struct Font<'a> {
    font: &'a bundle::ArchivedFont,
    vao: GLuint,
    tex: GLuint,
    test: String,
}

impl<'a> Font<'a> {
    pub fn load(gl: &Gl, name: &str, font: &'a bundle::ArchivedFont, blank_tex: GLuint) -> Self {
        let vao = make_arrays(
            gl,
            &font.verts[..],
            &font.idxs[..],
            &[  Attrib::Float{n: 3, off:  0},
                Attrib::ConstFloat3(uv::Vec3::new(1., 1., 0.)),
                Attrib::ConstFloat3(uv::Vec3::one())
            ]
        );

        let chars = "\
            abcdefghijklmnopqrstuvwxyz\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\
            0123456789!\"£$%^&*()-_=+[]{};'#:@~,./<>?\\`\
            ©®™\
            ";
        let test = format!("{name:15}: {chars}");

        Font{font, vao, tex: blank_tex, test}
    }

    pub fn bake_run(&self, anchor: Anchor, text: &str) -> TextRun {
        let run = text.chars()
            .filter_map(|ch| {
                let glyph = self.font.glyphs.get(&ch)?;
                Some((glyph.start, glyph.count, glyph.advance))
            })
            .collect::<Vec<_>>();
        let w: f32 = run.iter().copied()
            .map(|(_, _, adv)| adv)
            .sum();
        let off = match anchor {
            Anchor::LowerLeft => uv::Vec2::zero(),
            Anchor::Center    => uv::Vec2::new(-w * 0.5, -0.5),
        };
        TextRun{vao: self.vao, tex: self.tex, run, off}
    }

    /*
    fn draw_text(&self, gl: &Gl, scale: f32, mut pos: uv::Vec3, text: &str) {
        unsafe {
            gl.Uniform3f(5, scale, scale, scale * 0.2);
            gl.BindVertexArray(self.vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);
            for ch in text.chars() {
                if let Some(glyph) = self.font.glyphs.get(&ch) {
                    gl.Uniform3f(4, pos.x, pos.y, pos.z);
                    gl.DrawElements(
                        gl::TRIANGLES,
                        glyph.count as _,
                        gl::UNSIGNED_INT,
                        (glyph.start * 4) as _
                    );
                    pos.x += scale * glyph.advance;
                }
                else {
                    pos.x += scale;
                }
            }
        }
    }
    */

    pub fn draw_glyphs_test(&self, gl: &Gl, mut pos: uv::Vec3) {
        let scale = 10.;

        unsafe {
            gl.Uniform3f(5, scale * 10., scale * -10., scale);
            gl.BindVertexArray(self.vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);
            for ch in self.test.chars() {
                if let Some(glyph) = self.font.glyphs.get(&ch) {
                    gl.Uniform3f(4, pos.x, pos.y, pos.z);
                    gl.DrawElements(
                        gl::TRIANGLES,
                        glyph.count as _,
                        gl::UNSIGNED_INT,
                        (glyph.start * 4) as _
                    );
                    pos.x += scale * 10. * glyph.advance;
                }
            }
        }
    }
}*/

#[allow(dead_code)]
pub enum Anchor {
    LowerLeft,
    Center,
}

pub struct TextRun {
    vao: GLuint,
    tex: GLuint,
    run: Vec<(usize, usize, f32)>,
    off: uv::Vec2,
}

impl TextRun {
    pub fn draw(&self, gl: &Gl, shader: &BasicShader, scale: f32, z_scale: f32, mut pos: uv::Vec3) {
        shader.setup(gl, |params| params.1 = [scale, scale, z_scale * scale].into());
        pos += self.off.xyz() * scale;

        unsafe {
            gl.BindVertexArray(self.vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);

            for &(start, count, advance) in &self.run {
                shader.set_translate(gl, pos);
                gl.DrawElements(
                    gl::TRIANGLES,
                    count as _,
                    gl::UNSIGNED_INT,
                    (start * 4) as _
                );
                pos.x += scale * advance;
            }
        }
    }
}

