use {
    crate::gl::prelude::*,
    crate::render::{Attrib, make_arrays, BasicShader},
    ultraviolet as uv,
};

pub struct Font<'a> {
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
}

#[allow(dead_code)]
pub enum Anchor {
    LowerLeft,
    Center,
}

pub struct TextRun {
    vao: GLuint,
    tex: GLuint,
    run: Vec<(u32, u32, f32)>,
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

