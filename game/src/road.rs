use {
    crate::{
        gl::prelude::*,
        render,
        atlas::Atlas,
    },
    util::unorm::*,
    ultraviolet as uv,
};

pub struct RoadMesh {
    vao: GLuint,
    n_idxs: u32,
}

impl RoadMesh {
    pub fn build(gl: &Gl, model: &bundle::ArchivedRoadModel, atlas: &Atlas) -> Self {
        let n_faces = model.f_verts.len();

        let verts = (0 .. n_faces)
            .flat_map(|face_i| {
                let verts = model.f_verts[face_i];
                let tex   = model.f_tex  [face_i];
                let flags = model.f_flags[face_i];
                let rgb   = model.f_rgb  [face_i];

                let uvs: [f32; 4] = atlas.lookup_rect(tex).into();
                let [u0, v0, u1, v1]: [un16; 4] = uvs.map(un16::new);
                let uvs = [[u1, v0], [u0, v0], [u0, v1], [u1, v1]];
                let uvis =
                    if flags & 4 == 0 {[0, 1, 2, 3]}
                    else              {[1, 0, 3, 2]};
                let uvs = uvis.map(|i| uvs[i]);

                let rgb = {
                    let [r, g, b] = rgb.map(|x| x.0);
                    [r, g, b, 255].map(UNorm8)
                };

                verts.map(|vi| model.verts[vi as usize])
                    .zip(uvs)
                    .map(|(xyz, uv)| bundle::MeshVert{xyz, rgb, uv})
            })
            .collect::<Vec<_>>();

        let idxs = (0 .. n_faces as u32)
            .flat_map(|face_i| [0, 1, 2, 0, 2, 3].map(|i| i + face_i * 4))
            .collect::<Vec<_>>();

        let vao = render::make_arrays(
            gl,
            &verts,
            &idxs,
            &[  render::Attrib::Float{n: 3, off: 0},
                render::Attrib::Norm{n: 4, off: 12, ty: gl::UNSIGNED_BYTE},
                render::Attrib::Norm{n: 2, off: 16, ty: gl::UNSIGNED_SHORT},
            ]
        );

        RoadMesh{vao, n_idxs: idxs.len() as u32}
    }

    pub fn draw(&self, gl: &Gl, shader: &render::BasicShader) {
        shader.setup(gl, |params| params.3 = true);
        unsafe {
            gl.BindVertexArray(self.vao);
            gl.DrawElements(
                gl::TRIANGLES,
                self.n_idxs as _,
                gl::UNSIGNED_INT,
                std::ptr::null(),
            );
        }
    }
}

