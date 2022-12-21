#![feature(array_zip)]

mod gl;

use {
    gl::{Gl, types::{GLenum, GLuint}},
    winit::{
        event_loop::EventLoop,
        window::WindowBuilder,
    },
    glutin::{
        prelude::*,
        config::{Api, ConfigTemplateBuilder},
        context::{ContextAttributesBuilder, ContextApi},
        display::GetGlDisplay as _,
        surface::{SurfaceAttributesBuilder, WindowSurface, SwapInterval},
    },
    glutin_winit::DisplayBuilder,
    raw_window_handle::HasRawWindowHandle as _,
    ultraviolet as uv,
};

#[derive(Default, Clone, Copy)]
struct Bipole(i32, i32);

impl Bipole {
    fn pos(&mut self, go: bool) {
        if go {self.1 = self.0 + 1}
        else  {self.1 = 0};
    }

    fn neg(&mut self, go: bool) {
        if go {self.0 = self.1 + 1}
        else  {self.0 = 0};
    }

    fn eval(self) -> i32 {
        (self.1 - self.0).signum()
    }
}

#[derive(Default)]
struct Control {
    fore_back: Bipole,
    rigt_left: Bipole,
    fall_rise: Bipole,
}

fn main() {
    let bundle = unsafe {
        let compressed = include_bytes!(env!("BUNDLE_PATH"));
        let bytes = bundle::lz4_flex::decompress_size_prepended(compressed).unwrap();
        /*#[repr(C)] struct W<A, B: ?Sized> { _a: [A; 0], b: B }
        static BYTES: &'static W<u128, [u8]> 
           = &W{_a: [], b: *include_bytes!(env!("BUNDLE_PATH"))};
        let bytes = &BYTES.b;*/
        eprintln!("bundle size: {} MiB compressed, {} MiB expanded",
            compressed.len() >> 20, bytes.len() >> 20);
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        rkyv::archived_root::<bundle::Root>(&bytes)
    };

    let track = &bundle.tracks[bundle::asset_id("track11")];
    let mesh = &bundle.meshes[track.mesh];
    let atlas = &bundle.atlases[track.atlas];
    let atlas_image = &bundle.images[atlas.image];
    let atlas_uvs = &atlas.uvs;

    let eloop = EventLoop::new();

    let (win, cfg) = {
        let win_build = WindowBuilder::new()
            .with_title("Formula RS");
        let ct_build = ConfigTemplateBuilder::new()
            .with_api(Api::GLES3)
            .prefer_hardware_accelerated(Some(true));

        DisplayBuilder::new()
            .with_window_builder(Some(win_build))
            .build(&eloop, ct_build, |cfgs| {
                use glutin::config::GlConfig as _;
                cfgs.max_by_key(|c| c.depth_size())
                    .unwrap()
            })
            .map(|(win, cfg)| (win.unwrap(), cfg))
            .unwrap()
    };

    let win_handle = win.raw_window_handle();
    let display = cfg.display();

    let surf = {
        let (w, h): (u32, u32) = win.inner_size().into();
        let surf_attrs = SurfaceAttributesBuilder::<WindowSurface>::new()
            .build(win_handle, w.try_into().unwrap(), h.try_into().unwrap());
        unsafe {display.create_window_surface(&cfg, &surf_attrs).unwrap()}
    };

    let ctx = {
        let ctx_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .with_debug(true)
            .build(Some(win_handle));
        let ctx = unsafe {display.create_context(&cfg, &ctx_attrs).unwrap()};
        ctx.make_current(&surf).unwrap()
    };

    surf.set_swap_interval(&ctx, SwapInterval::Wait(1.try_into().unwrap())).unwrap();

    let gl = gl::Gl::load_with(|sym|
        display.get_proc_address(&std::ffi::CString::new(sym).unwrap()).cast()
    );

    unsafe {
        gl.Enable(gl::CULL_FACE);
        //gl.Enable(gl::BLEND);
        gl.Enable(gl::DEPTH_TEST);
        gl.BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
        gl.PixelStorei(gl::UNPACK_ALIGNMENT, 0);
    }

    let shader_prog = make_shader_prog(&gl);
    let vao = make_mesh(&gl, mesh);
    let tex = make_atlas(&gl, atlas_image, atlas_uvs);

    let fov_deg = 100_f32;
    let aspect = 16. / 9.;
    let gl_eye_to_clip = uv::projection::perspective_gl(
        (fov_deg / aspect).to_radians(),
        aspect,
        10.0,
        1_000_000.0
    );

    let wo_eye_to_gl_eye = uv::Mat4::from_nonuniform_scale(uv::Vec3::new(1., -1., -1.));

    let eye_to_clip = gl_eye_to_clip * wo_eye_to_gl_eye;


    let mut ctrl = Control::default();
    let mut ctrl_pan = 0.;
    let mut ctrl_tilt = 0.;
    let mut ctrl_turning = false;

    let mut cam_pos = uv::Vec3::new(0., 0., 0.);
    let mut cam_tilt = 0.;
    let mut cam_pan = 0.;
    //let mut cam_turn = uv::Rotor3::default();

    eloop.run(move |ev, _, flow| {
        flow.set_poll();

        use winit::event::{Event as Ev, WindowEvent as WinEv, DeviceEvent as DevEv};
        match ev {
            Ev::WindowEvent{event: win_ev, ..} => match win_ev {
                WinEv::CloseRequested => flow.set_exit(),
                WinEv::Resized(size) => surf.resize(
                    &ctx,
                    size.width.try_into().unwrap(),
                    size.height.try_into().unwrap(),
                ),
                WinEv::KeyboardInput{input, ..} => {
                    use winit::event::VirtualKeyCode as Vk;
                    let pressed = input.state == winit::event::ElementState::Pressed;
                    match input.virtual_keycode {
                        Some(Vk::W) => ctrl.fore_back.pos(pressed),
                        Some(Vk::S) => ctrl.fore_back.neg(pressed),

                        Some(Vk::D) => ctrl.rigt_left.pos(pressed),
                        Some(Vk::A) => ctrl.rigt_left.neg(pressed),

                        Some(Vk::LControl) => ctrl.fall_rise.pos(pressed),
                        Some(Vk::Space)    => ctrl.fall_rise.neg(pressed),

                        _ => { }
                    }
                }
                WinEv::MouseInput{button, state, ..} => {
                    use winit::event::MouseButton as Mb;
                    let pressed = state == winit::event::ElementState::Pressed;
                    if let Mb::Left | Mb::Right = button {
                        ctrl_turning = pressed;
                        win.set_cursor_visible(!ctrl_turning);
                        use winit::window::CursorGrabMode as Grab;
                        if ctrl_turning {
                            win.set_cursor_grab(Grab::Locked)
                                .or_else(|_| win.set_cursor_grab(Grab::Confined))
                                .unwrap();
                        }
                        else {
                            win.set_cursor_grab(Grab::None).unwrap();
                        }
                    }
                }
                _ => { }
            }

            Ev::DeviceEvent{event, ..} => match event {
                DevEv::MouseMotion{delta: (dx, dy)} if ctrl_turning => {
                    ctrl_pan  += dx;
                    ctrl_tilt += dy;
                }
                _ => { }
            }

            Ev::RedrawEventsCleared => {
                const SPEED: f32 = 0.00015;
                let pan_intent  = -ctrl_pan as f32 * SPEED;
                let tilt_intent = -ctrl_tilt as f32 * SPEED;
                ctrl_pan  = 0.;
                ctrl_tilt = 0.;
                cam_pan = (cam_pan + pan_intent).fract();
                cam_tilt = (cam_tilt + tilt_intent).clamp(-0.245, 0.245);
                let pan  = uv::Rotor3::from_rotation_xz(cam_pan  * std::f32::consts::TAU);
                let tilt = uv::Rotor3::from_rotation_yz(cam_tilt * std::f32::consts::TAU);
                let cam_turn = pan * tilt;

                let dx = ctrl.rigt_left.eval() as f32;
                let dy = ctrl.fall_rise.eval() as f32;
                let dz = ctrl.fore_back.eval() as f32;
                let move_intent = cam_turn * uv::Vec3::new(dx, 0., dz) + uv::Vec3::new(0., dy, 0.);
                cam_pos += move_intent * 200.;

                let world_to_clip
                    = eye_to_clip
                    * cam_turn.reversed().into_matrix().into_homogeneous()
                    * uv::Mat4::from_translation(-cam_pos);

                unsafe {
                    gl.ClearColor(0.04, 0.0, 0.08, 1.);
                    gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

                    let size = win.inner_size();
                    let w = size.width as i32;//surf.width().unwrap() as i32;
                    let h = size.height as i32;//surf.height().unwrap() as i32;
                    gl.Viewport(0, 0, w, h);

                    gl.UseProgram(shader_prog);
                    gl.UniformMatrix4fv(0, 1, gl::FALSE, world_to_clip.as_ptr() as _);

                    gl.BindVertexArray(vao);
                    gl.DrawElements(
                        gl::TRIANGLES,
                        mesh.idxs.len() as _,
                        gl::UNSIGNED_INT,
                        std::ptr::null(),
                    );
                }

                win.request_redraw();
                surf.swap_buffers(&ctx).unwrap();
            }

            _ => { }
        }
    });
}

fn make_shader_prog(gl: &Gl) -> GLuint {
    let vs = make_shader_unit(&gl, gl::VERTEX_SHADER,   "v", include_bytes!("basic-v.glsl"));
    let fs = make_shader_unit(&gl, gl::FRAGMENT_SHADER, "f", include_bytes!("basic-f.glsl"));

    unsafe {
        let prog = gl.CreateProgram();
        gl.AttachShader(prog, vs);
        gl.AttachShader(prog, fs);
        gl.LinkProgram(prog);
        gl.DeleteShader(vs);
        gl.DeleteShader(fs);
        prog
    }
}

fn make_shader_unit(gl: &Gl, ty: GLenum, label: &str, src: &[u8]) -> GLuint {
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

fn make_mesh(gl: &Gl, mesh: &bundle::ArchivedMesh) -> GLuint {
    unsafe {
        let mut vao = 0u32;
        gl.GenVertexArrays(1, &mut vao as _);
        gl.BindVertexArray(vao);

        let mut array_vbo = 0u32;
        gl.GenBuffers(1, &mut array_vbo as _);
        gl.BindBuffer(gl::ARRAY_BUFFER, array_vbo);
        gl.BufferData(
            gl::ARRAY_BUFFER,
            std::mem::size_of_val(&mesh.verts[..]) as _,
            mesh.verts.as_ptr() as _,
            gl::STATIC_DRAW,
        );

        let mut index_vbo = 0u32;
        gl.GenBuffers(1, &mut index_vbo as _);
        gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, index_vbo);
        gl.BufferData(
            gl::ELEMENT_ARRAY_BUFFER,
            std::mem::size_of_val(&mesh.idxs[..]) as _,
            mesh.idxs.as_ptr() as _,
            gl::STATIC_DRAW,
        );

        let attrs = [
            (3, gl::INT,           false,  0),
            (3, gl::UNSIGNED_BYTE, true,  12),
            (2, gl::FLOAT,         false, 16),
        ];

        let stride = std::mem::size_of_val(&mesh.verts[0]);
        for (i, (dim, ty, norm, off)) in attrs.into_iter().enumerate() {
            gl.VertexAttribPointer(i as u32, dim, ty, norm as _, stride as _, off as _);
            gl.EnableVertexAttribArray(i as u32);
        }

        vao
    }
}

fn make_atlas(gl: &Gl, image: &bundle::ArchivedImage, uvs: &[[f32; 4]]) -> GLuint {
    unsafe {
        let mut tex = 0u32;
        gl.GenTextures(1, &mut tex as _);
        gl.BindTexture(gl::TEXTURE_2D, tex);
        gl.TexImage2D(
            gl::TEXTURE_2D, 0,
            gl::RGBA8 as i32,
            image.wide as i32, image.high as i32,
            0,
            gl::RGBA, gl::UNSIGNED_BYTE,
            image.levels[0].as_ptr() as _
        );
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR  as i32);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
        tex
    }
}

