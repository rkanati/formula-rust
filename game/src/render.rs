mod model_set;
pub use model_set::ModelSet;

use {
    crate::gl::prelude::*,
    ultraviolet as uv,
    pixmap::{Pixmap, Rgba},
    bytemuck as bm,
    util::{un8, un16},
};

pub struct BasicShader {
    prog: GLuint,
}

impl BasicShader {
    pub fn create(gl: &Gl) -> Self {
        let vs = make_shader_unit(&gl, gl::VERTEX_SHADER,   "v", include_bytes!("basic-v.glsl"));
        let fs = make_shader_unit(&gl, gl::FRAGMENT_SHADER, "f", include_bytes!("basic-f.glsl"));

        let prog = unsafe {
            let prog = gl.CreateProgram();
            gl.AttachShader(prog, vs);
            gl.AttachShader(prog, fs);
            gl.LinkProgram(prog);

            let mut log_len = 0;
            gl.GetProgramiv(prog, gl::INFO_LOG_LENGTH, &mut log_len as _);
            if log_len > 0 {
                let mut buf = Vec::with_capacity(log_len as usize);
                buf.resize(log_len as usize + 1, 0u8);
                gl.GetProgramInfoLog(prog, log_len, std::ptr::null_mut(), buf.as_ptr() as _);
                log::debug!(target: "make_shader_prog", "program link log:");
                buf.split(|&ch| ch == b'\n')
                    .for_each(|line| log::debug!(target: "make_shader_prog", "> {}", std::str::from_utf8(line).unwrap_or("???")));
            }

            gl.DeleteShader(vs);
            gl.DeleteShader(fs);
            prog
        };

        Self{prog}
    }

    pub fn select(&self, gl: &Gl, world_to_clip: uv::Mat4) {
        unsafe {
            gl.UseProgram(self.prog);
            gl.UniformMatrix4fv(0, 1, gl::FALSE, world_to_clip.as_ptr() as _);
        }
        self.setup(gl, |_| { });
    }

    pub fn setup(&self, gl: &Gl, f: impl FnOnce(&mut (uv::Vec3, uv::Vec3, uv::Mat3, bool))) {
        let params = &mut (uv::Vec3::zero(), uv::Vec3::one(), uv::Mat3::identity(), true);
        f(params);
        self.set_translate(gl, params.0);
        self.set_scale(gl, params.1);
        self.set_rotate(gl, params.2);
        self.set_alpha_test(gl, params.3);
    }

    pub fn set_translate(&self, gl: &Gl, translate: uv::Vec3) {
        unsafe { gl.Uniform3fv(4, 1, translate.as_ptr() as _); }
    }

    pub fn set_scale(&self, gl: &Gl, scale: uv::Vec3) {
        unsafe { gl.Uniform3fv(5, 1, scale.as_ptr() as _); }
    }

    pub fn set_rotate(&self, gl: &Gl, rotate: uv::Mat3) {
        unsafe { gl.UniformMatrix3fv(6, 1, gl::FALSE, rotate.as_ptr() as _); }
    }

    pub fn set_alpha_test(&self, gl: &Gl, enable: bool) {
        unsafe { gl.Uniform1i(100, enable as i32); }
    }
}

pub fn make_shader_unit(gl: &Gl, ty: GLenum, label: &str, src: &[u8]) -> GLuint {
    unsafe {
        let unit = gl.CreateShader(ty);
        gl.ShaderSource(
            unit,
            1,
            [src.as_ptr().cast()].as_ptr(),
            [src.len() as i32].as_ptr(),
        );
        gl.CompileShader(unit);
        let mut log_len = 0;
        gl.GetShaderiv(unit, gl::INFO_LOG_LENGTH, &mut log_len as _);
        if log_len > 0 {
            let mut buf = Vec::with_capacity(log_len as usize);
            buf.resize(log_len as usize + 1, 0u8);
            gl.GetShaderInfoLog(unit, log_len, std::ptr::null_mut(), buf.as_ptr() as _);
            eprintln!("shader '{label}' compile log:");
            buf.split(|&ch| ch == b'\n')
                .for_each(|line| eprintln!("> {}", std::str::from_utf8(line).unwrap_or("???")));
            eprintln!();
        }
        unit
    }
}

pub trait GlPodType { const ENUM: GLenum; }
impl GlPodType for u32 { const ENUM: GLenum = gl::UNSIGNED_INT; }
impl GlPodType for u16 { const ENUM: GLenum = gl::UNSIGNED_SHORT; }
impl GlPodType for u8 { const ENUM: GLenum = gl::UNSIGNED_BYTE; }
impl GlPodType for i32 { const ENUM: GLenum = gl::INT; }
impl GlPodType for i16 { const ENUM: GLenum = gl::SHORT; }
impl GlPodType for i8 { const ENUM: GLenum = gl::BYTE; }
impl GlPodType for f32 { const ENUM: GLenum = gl::FLOAT; }

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Attrib {
    Float{n: u8, off: usize},
    Int  {n: u8, off: usize, ty: GLenum},
    Norm {n: u8, off: usize, ty: GLenum},
    ConstFloat3(uv::Vec3),
}

pub fn make_arrays<E, I: GlPodType> (
    gl: &Gl,
    elements: &[E],
    indices: &[I],
    attribs: &[Attrib],
) -> GLuint {
    unsafe {
        let mut vao = 0u32;
        gl.GenVertexArrays(1, &mut vao as _);
        gl.BindVertexArray(vao);

        let mut array_vbo = 0u32;
        gl.GenBuffers(1, &mut array_vbo as _);
        gl.BindBuffer(gl::ARRAY_BUFFER, array_vbo);
        gl.BufferData(
            gl::ARRAY_BUFFER,
            std::mem::size_of_val(elements) as _,
            elements.as_ptr() as _,
            gl::STATIC_DRAW,
        );

        let mut index_vbo = 0u32;
        gl.GenBuffers(1, &mut index_vbo as _);
        gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_vbo);
        gl.BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            std::mem::size_of_val(indices) as _,
            indices.as_ptr() as _,
            gl::STATIC_DRAW,
        );

        let stride = std::mem::size_of_val(&elements[0]) as _;
        for (i, &attrib) in attribs.into_iter().enumerate() {
            let i = i as u32;
            match attrib {
                Attrib::Float{n, off} => {
                    gl.VertexAttribPointer(i, n as _, gl::FLOAT, gl::FALSE, stride, off as _);
                }

                Attrib::Int{n, ty, off} => {
                    gl.VertexAttribPointer(i, n as _, ty, gl::FALSE, stride, off as _);
                }

                Attrib::Norm{n, ty, off} => {
                    gl.VertexAttribPointer(i, n as _, ty, gl::TRUE, stride, off as _);
                }

                Attrib::ConstFloat3(v) => {
                    gl.VertexAttrib3f(i, v.x, v.y, v.z);
                    gl.DisableVertexAttribArray(i);
                    continue;
                }
            }

            gl.EnableVertexAttribArray(i);
        }

        vao
    }
}

/*pub fn make_sprites(gl: &Gl, sprites: &[bundle::ArchivedSprite]) -> GLuint {
    #[repr(C)] struct V([f32; 2], [bundle::ArchivedUNorm16; 2]);

    let data = sprites.iter()
        .flat_map(|&bundle::ArchivedSprite{uvs, ..}| [
            V([-0.5f32, -0.5f32], [uvs[0], uvs[1]]),
            V([-0.5f32,  0.5f32], [uvs[0], uvs[3]]),
            V([ 0.5f32,  0.5f32], [uvs[2], uvs[3]]),
            V([ 0.5f32, -0.5f32], [uvs[2], uvs[1]]),
        ])
        .collect::<Vec<_>>();

    make_arrays(
        gl,
        &data[..],
        &[0u32; 0],
        &[  Attrib::Float{n: 2, off: 0},
            Attrib::ConstFloat3(uv::Vec3::one()),
            Attrib::Norm{n: 2, off: 8, ty: gl::UNSIGNED_SHORT},
        ]
    )
}*/

pub fn make_blank_texture(gl: &Gl) -> GLuint {
    unsafe {
        let mut tex = 0u32;
        gl.GenTextures(1, &mut tex as _);
        gl.BindTexture(gl::TEXTURE_2D, tex);
        let params = [
            (gl::TEXTURE_MIN_FILTER, gl::NEAREST),
            (gl::TEXTURE_MAG_FILTER, gl::NEAREST),
            (gl::TEXTURE_MAX_LEVEL,  0),
            (gl::TEXTURE_WRAP_S,     gl::CLAMP_TO_EDGE),
            (gl::TEXTURE_WRAP_T,     gl::CLAMP_TO_EDGE),
        ];
        for (pn, pv) in params { gl.TexParameteri(gl::TEXTURE_2D, pn, pv as i32); }
        gl.TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA8 as i32, 1, 1, 0, gl::RGBA, gl::UNSIGNED_BYTE,
            [0xff_u8; 4].as_ptr() as _
        );
        tex
    }
}

fn reduce(src: &[u8], sw: usize, sh: usize) -> Option<(Vec<u8>, usize, usize)> {
    if sw % 2 != 0 || sh % 2 != 0 { return None; }

    let dw = sw / 2;
    let dh = sh / 2;

    let pairs: &[[[u8; 4]; 2]] = bytemuck::cast_slice(src);
    let pixels = pairs.chunks(dw as usize)
        .array_chunks()
        .flat_map(|[even_row, odd_row]| {
            even_row.into_iter().zip(odd_row.into_iter())
                .map(|([a, b], [c, d])| {
                    [a, b, c, d].into_iter()
                        .fold(
                            [0u32; 4],
                            |a, &p| a.zip(p).map(|(a, p)| a + p as u32)
                        )
                        .map(|x| (x / 4) as u8)
                })
        })
        .flatten()
        .collect::<Vec<_>>();

    Some((pixels, dw, dh))
}


pub fn make_texture(gl: &Gl, image: &Pixmap<&[Rgba]>) -> GLuint {
    make_texture_raw(gl,
        image.wide() as u16,
        image.high() as u16,
        bm::cast_slice(image.try_as_slice().unwrap()),
    )
    /*let qoi = bundle::rapid_qoi::Qoi {
        width:  image.wide as u32,
        height: image.high as u32,
        colors: bundle::rapid_qoi::Colors::Rgba,
    };
    let mut buf = Vec::new();
    buf.resize(qoi.decoded_size(), 0u8);
    let (s0, s1, s2) = &mut ([[0u8; 4]; 64], [0u8, 0u8, 0u8, 0xff_u8], 0_usize);
    bundle::rapid_qoi::Qoi::decode_range(s0, s1, s2, &image.data, &mut buf).unwrap();

    make_texture_raw(gl, image.wide, image.high, &buf)*/
}

pub fn make_texture_raw(gl: &Gl, w: u16, h: u16, pixels: &[u8]) -> GLuint {
    const N_REDUCTIONS: usize = 4;

    let tex = unsafe {
        let mut tex = 0u32;
        gl.GenTextures(1, &mut tex as _);
        gl.BindTexture(gl::TEXTURE_2D, tex);
        let params = [
            (gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR),
            (gl::TEXTURE_MAG_FILTER, gl::NEAREST),
            (gl::TEXTURE_MAX_LEVEL,  N_REDUCTIONS as u32),
            (gl::TEXTURE_WRAP_S,     gl::CLAMP_TO_EDGE),
            (gl::TEXTURE_WRAP_T,     gl::CLAMP_TO_EDGE),
            (gl::TEXTURE_MAX_ANISOTROPY_EXT, 8),
        ];
        for (pn, pv) in params { gl.TexParameteri(gl::TEXTURE_2D, pn, pv as i32); }
        tex
    };

    let mut pixels = pixels;
    let mut buf = Vec::new();
    let mut w = w as usize;
    let mut h = h as usize;
    for level_i in 0..=N_REDUCTIONS {
        if level_i > 0 {
            (buf, w, h) = reduce(pixels, w as usize, h as usize).unwrap();
            pixels = &buf[..];
        }

        unsafe {
            gl.TexImage2D(
                gl::TEXTURE_2D, level_i as _,
                gl::RGBA8 as i32,
                w as i32, h as i32, 0,
                gl::RGBA, gl::UNSIGNED_BYTE,
                pixels.as_ptr() as _
            );
        }
    }

    tex
}

/*pub struct Scene<'a> {
    objs: &'a [bundle::ArchivedSceneObject],
    sprs: &'a [bundle::ArchivedSprite],
    vao: GLuint,
    spr_vao: Option<GLuint>,
    tex: GLuint,
}

impl<'a> Scene<'a> {
    pub fn load(gl: &Gl, scene: &'a bundle::ArchivedScene, image: &'a bundle::ArchivedImage) -> Self {
        let objs = &scene.objects[..];
        let sprs = &scene.sprites[..];
        let vao = make_mesh(&gl, &scene.mesh);
        let spr_vao = (!sprs.is_empty()).then(|| make_sprites(&gl, &scene.sprites));
        let tex = make_texture(&gl, &image);
        Scene{objs, sprs, vao, spr_vao, tex}
    }

    pub fn n_objects(&self) -> usize {
        self.objs.len()
    }

    pub fn draw_objects(&self,
        gl: &Gl, shader: &BasicShader,
        instances: impl IntoIterator<Item = (usize, uv::Vec3)>,
    ) {
        shader.setup(gl, |params| params.3 = true);
        let instances = instances.into_iter();
        unsafe {
            gl.BindVertexArray(self.vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);
            for (obj_i, translate) in instances {
                let bundle::ArchivedSceneObject{xyz, start, count} = self.objs[obj_i];
                shader.set_translate(gl, translate + uv::Vec3::from(xyz.map(|x| x as f32)));
                gl.DrawElements(
                    gl::TRIANGLES,
                    count as _,
                    gl::UNSIGNED_INT,
                    (start * 4) as _
                );
            }
        }
    }

    /*fn draw_object(&self, gl: &Gl, index: usize, translate: uv::Vec3) {
        self.draw_objects(gl, [(index, translate)])
    }*/

    fn draw_sprites(&self, gl: &Gl, shader: &BasicShader, eye_pos: uv::Vec3) {
        let Some(vao) = self.spr_vao else {return};
        shader.setup(gl, |params| params.3 = true);
        unsafe {
            gl.BindVertexArray(vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);
            for (i, sprite) in self.sprs.iter().enumerate() {
                let eye_xz = uv::Vec3::new(eye_pos.x,     0., eye_pos.z);
                let spr_xz = uv::Vec3::new(sprite.xyz[0], 0., sprite.xyz[2]);
                let rotate = uv::Rotor3::from_rotation_between(
                    uv::Vec3::unit_z(),
                    (spr_xz - eye_xz).normalized(),
                ).into_matrix();
                shader.set_rotate(gl, rotate);
                shader.set_translate(gl, sprite.xyz.into());
                shader.set_scale(gl, [sprite.wh[0], sprite.wh[1], 1.].into());
                let [r,g,b,a] = sprite.rgb.map(Into::into);
                gl.VertexAttrib4f(1, r, g, b, a);
                gl.DrawArrays(gl::TRIANGLE_FAN, (i*4) as i32, 4);
            }
        }
    }

    pub fn draw(&self, gl: &Gl, shader: &BasicShader, eye_pos: uv::Vec3) {
        let all = (0..self.objs.len())
            .map(|i| (i, uv::Vec3::zero()));
        self.draw_objects(gl, shader, all);
        self.draw_sprites(gl, shader, eye_pos);
    }
}*/


#[repr(C)]
pub struct MeshElement {
    pub xyz: [f32; 3],
    pub rgb: [un8; 3],
    pub uv:  [un16; 2],
}

impl MeshElement {
    pub const ATTRIBS: &[Attrib] = &[
        Attrib::Float{n: 3, off: 0},
        Attrib::Norm{n: 3, off: 12, ty: gl::UNSIGNED_BYTE},
        Attrib::Norm{n: 2, off: 16, ty: gl::UNSIGNED_SHORT},
    ];
}

