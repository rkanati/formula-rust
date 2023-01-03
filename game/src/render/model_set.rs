use {
    crate::{
        gl::prelude::*,
        atlas::Atlas,
        render::{Gl, make_arrays, BasicShader, MeshElement},
    },
    ultraviolet as uv,
    anyhow::Result as Anyhow,
    util::{un16, UNorm8},
};

pub struct ModelSet {
    vao: GLuint,
    tex: GLuint,
    objs: Vec<Obj>,
}

struct Obj {
    pos: uv::Vec3,
    face_0: i32,
    face_n: i32,
}

impl ModelSet {
    pub fn load(gl: &Gl, mset: &bundle::ArchivedModelSet, atlas: Atlas) -> Anyhow<Self> {
        let elements = build_elements(mset, &atlas);
        let vao = make_arrays(
            gl,
            &elements,
            &[0u32; 0][..],
            MeshElement::ATTRIBS,
        );

        let objs = mset.obj_xyz.iter().copied()
            .enumerate()
            .map(|(obj_i, xyz)| {
                let pos = xyz.into();
                let face_0 = mset.obj_face_0[obj_i].into();
                let face_n = mset.obj_face_0.get(obj_i + 1).copied()
                    .map(i32::from)
                    .unwrap_or(mset.face_vis.len() as i32)
                    - face_0;
                Obj{pos, face_0, face_n}
            })
            .collect();

        let tex = atlas.into_texture();
        Ok(ModelSet{vao, tex, objs})
    }

    pub fn object_count(&self) -> usize {
        self.objs.len()
    }

    pub fn draw(&self, gl: &Gl, shader: &BasicShader) {
        self.draw_objects(gl, shader, (0..self.objs.len()).map(|i| (i, uv::Vec3::zero())));
    }

    pub fn draw_objects(&self,
        gl: &Gl,
        shader: &BasicShader,
        params: impl IntoIterator<Item = (usize, uv::Vec3)>,
    ) {
        shader.setup(gl, |params| params.3 = true);
        unsafe {
            gl.BindVertexArray(self.vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);
            for (obj_i, translate) in params {
                let Obj{pos, face_0, face_n} = self.objs[obj_i];
                shader.set_translate(gl, translate + pos);
                gl.DrawArrays(gl::TRIANGLES, (face_0 * 3).into(), (face_n * 3).into());
            }
        }
    }
}

fn build_elements(mset: &bundle::ArchivedModelSet, atlas: &Atlas) -> Vec<MeshElement> {
    (0 .. mset.face_vis.len())
        .flat_map(|face_i| {
            let vis = mset.face_vis[face_i];
            let xyz = vis.map(|vi| mset.verts[vi as usize]);

            let rgb = mset.face_rgb[face_i];

            let ruv = mset.face_ruv[face_i];
            let tex = mset.face_tex[face_i];

            [0, 1, 2].map(|i| {
                let xyz = xyz[i];
                let rgb = rgb[i].map(|c| UNorm8(c.0));
                let uv: [f32; 2] = atlas.lookup(tex as usize, ruv[i]).into();
                let uv = uv.map(un16::new);
                MeshElement{xyz, rgb, uv}
            })
        })
        .collect()
}

