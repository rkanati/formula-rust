#![feature(array_zip)]

mod gl;

use {
    gl::{Gl, types::{GLenum, GLuint, GLsizei, GLchar}},
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

const TURN_SPEED: f32 = 0.00015;
const MOVE_SPEED: f32 = 50.;
const VFOV_DEG: f32 = 57.;

fn main() {
    // process arguments
    let track_name = std::env::args().nth(1)
        .expect("usage: formula-rust \x1b[3m<track-name>\x1b[0m");

    // bring up graphics
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
            .with_srgb(Some(true))
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
        gl.DebugMessageCallback(Some(on_gl_debug), std::ptr::null());
        gl.DebugMessageControl(
            gl::DONT_CARE,
            gl::DONT_CARE,
            gl::DONT_CARE,
            0, std::ptr::null(),
            gl::TRUE
        );
        gl.Enable(gl::DEBUG_OUTPUT);
    }

    unsafe {
        //gl.Enable(gl::BLEND);
        //gl.Enable(gl::FRAMEBUFFER_SRGB);
        gl.Enable(gl::DEPTH_TEST);
        gl.BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
        gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);
    }

    let shader_prog = make_shader_prog(&gl);

    // start loading assets
    let bundle = {
        let decomp_start = std::time::Instant::now();

        let compressed = include_bytes!(env!("BUNDLE_PATH"));
        let bytes = bundle::lz4_flex::decompress_size_prepended(compressed).unwrap();
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let bundle = bundle::Root::from_bytes(&bytes);

        eprintln!("bundle loaded: {} -> {} MiB; took {}s",
            compressed.len() >> 20,
            bytes.len() >> 20,
            decomp_start.elapsed().as_secs());

        bundle
    };

    let track = &bundle.tracks[bundle::asset_id(&track_name)];

    let (road_mesh_len, road_vao, road_tex) = {
        let vao = make_mesh(&gl, &*track.road_mesh);
        let tex = make_texture(&gl, &*track.road_image);
        (track.road_mesh.idxs.len(), vao, tex)
    };

    let scenery = Scene::load(&gl, &*track.scenery_scene, &*track.scenery_image);
    let sky = Scene::load(&gl, &*track.sky_scene, &*track.sky_image);
    let ships = Scene::load(&gl, &*bundle.ship_scene, &*bundle.ship_image);

    let mut ctrl_move = ControlMove::default();
    let mut ctrl_pan = 0.;
    let mut ctrl_tilt = 0.;
    let mut ctrl_turning = false;
    let mut ctrl_fast = false;

    let mut cam_pos = uv::Vec3::new(0., 0., 0.);
    let mut cam_tilt = 0.;
    let mut cam_pan = 0.;

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
                        Some(Vk::W) => ctrl_move.fore_back.pos(pressed),
                        Some(Vk::S) => ctrl_move.fore_back.neg(pressed),

                        Some(Vk::D) => ctrl_move.rigt_left.pos(pressed),
                        Some(Vk::A) => ctrl_move.rigt_left.neg(pressed),

                        Some(Vk::LControl) => ctrl_move.fall_rise.pos(pressed),
                        Some(Vk::Space)    => ctrl_move.fall_rise.neg(pressed),

                        Some(Vk::LShift) => ctrl_fast = pressed,

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
                let pan_intent  = -ctrl_pan  as f32 * TURN_SPEED;
                let tilt_intent = -ctrl_tilt as f32 * TURN_SPEED;
                ctrl_pan  = 0.;
                ctrl_tilt = 0.;
                cam_pan = (cam_pan + pan_intent).fract();
                cam_tilt = (cam_tilt + tilt_intent).clamp(-0.245, 0.245);
                let pan  = uv::Rotor3::from_rotation_xz(cam_pan  * std::f32::consts::TAU);
                let tilt = uv::Rotor3::from_rotation_yz(cam_tilt * std::f32::consts::TAU);
                let cam_turn = pan * tilt;

                let dx = ctrl_move.rigt_left.eval() as f32;
                let dy = ctrl_move.fall_rise.eval() as f32;
                let dz = ctrl_move.fore_back.eval() as f32;
                let move_intent = cam_turn * uv::Vec3::new(dx, 0., dz) + uv::Vec3::new(0., dy, 0.);
                cam_pos += move_intent * MOVE_SPEED * if ctrl_fast {4.} else {1.};

                let size = win.inner_size();
                let w = size.width as i32;//surf.width().unwrap() as i32;
                let h = size.height as i32;//surf.height().unwrap() as i32;

                let eye_to_clip = {
                    let aspect = w as f32 / h as f32;
                    let gl_eye_to_clip = uv::projection::perspective_gl(
                        VFOV_DEG.to_radians(),
                        aspect,
                        10.0,
                        1_000_000.0
                    );

                    let wo_eye_to_gl_eye = uv::Mat4::from_nonuniform_scale(
                        uv::Vec3::new(1., -1., -1.)
                    );

                    gl_eye_to_clip * wo_eye_to_gl_eye
                };

                let sky_to_clip
                    = eye_to_clip
                    * cam_turn.reversed().into_matrix().into_homogeneous();

                let world_to_clip
                    = eye_to_clip
                    * cam_turn.reversed().into_matrix().into_homogeneous()
                    * uv::Mat4::from_translation(-cam_pos);

                unsafe {
                    gl.ClearColor(0.04, 0.0, 0.08, 1.);
                    gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

                    gl.Viewport(0, 0, w, h);

                    gl.UseProgram(shader_prog);
                    gl.Enable(gl::CULL_FACE);

                    gl.DepthMask(gl::FALSE);
                    gl.UniformMatrix4fv(0, 1, gl::FALSE, sky_to_clip.as_ptr() as _);
                    sky.draw(&gl);

                    gl.DepthMask(gl::TRUE);
                    gl.UniformMatrix4fv(0, 1, gl::FALSE, world_to_clip.as_ptr() as _);
                    scenery.draw(&gl);

                    let ship_params = (0..ships.objs.len())
                        .map(|i| (i, uv::Vec3::unit_x() * 800. * i as f32));
                    ships.draw_objects(&gl, ship_params);

                    gl.Disable(gl::CULL_FACE);
                    gl.BindVertexArray(road_vao);
                    gl.BindTexture(gl::TEXTURE_2D, road_tex);
                    gl.Uniform3f(4, 0., 0., 0.);
                    gl.DrawElements(
                        gl::TRIANGLES,
                        road_mesh_len as _,
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
struct ControlMove {
    fore_back: Bipole,
    rigt_left: Bipole,
    fall_rise: Bipole,
}

fn make_shader_prog(gl: &Gl) -> GLuint {
    let vs = make_shader_unit(&gl, gl::VERTEX_SHADER,   "v", include_bytes!("basic-v.glsl"));
    let fs = make_shader_unit(&gl, gl::FRAGMENT_SHADER, "f", include_bytes!("basic-f.glsl"));

    unsafe {
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
            eprintln!("program link log:");
            buf.split(|&ch| ch == b'\n')
                .for_each(|line| eprintln!("> {}", std::str::from_utf8(line).unwrap_or("???")));
            eprintln!();
        }

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
            (3, gl::INT,   false,  0),
            (3, gl::FLOAT, false, 12),
            (2, gl::FLOAT, false, 24),
        ];

        let stride = std::mem::size_of_val(&mesh.verts[0]);
        for (i, (dim, ty, norm, off)) in attrs.into_iter().enumerate() {
            gl.VertexAttribPointer(i as u32, dim, ty, norm as _, stride as _, off as _);
            gl.EnableVertexAttribArray(i as u32);
        }

        vao
    }
}

fn make_texture(gl: &Gl, image: &bundle::ArchivedImage) -> GLuint {
    unsafe {
        let mut tex = 0u32;
        gl.GenTextures(1, &mut tex as _);
        gl.BindTexture(gl::TEXTURE_2D, tex);
        let params = [
            (gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR),
            (gl::TEXTURE_MAG_FILTER, gl::NEAREST),
            (gl::TEXTURE_MAX_LEVEL,  image.levels.len() as u32 - 1),
            (gl::TEXTURE_WRAP_S,     gl::CLAMP_TO_EDGE),
            (gl::TEXTURE_WRAP_T,     gl::CLAMP_TO_EDGE),
            (gl::TEXTURE_MAX_ANISOTROPY_EXT, 8),
            //(gl::TEXTURE_M,     gl::CLAMP_TO_EDGE),
        ];
        for (pn, pv) in params { gl.TexParameteri(gl::TEXTURE_2D, pn, pv as i32); }

        for (level_i, level) in image.levels.iter().enumerate() {
            gl.TexImage2D(
                gl::TEXTURE_2D,
                level_i as _,
                //gl::SRGB8_ALPHA8 as i32,
                gl::RGBA8 as i32,
                image.wide as i32 >> level_i,
                image.high as i32 >> level_i,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                level.as_ptr() as _
            );
        }

        tex
    }
}

extern "system" fn on_gl_debug(
    _source: GLenum,
    _ty:     GLenum,
    _id:     GLuint,
    _level:  GLenum,
    length:  GLsizei,
    message: *const GLchar,
    _user:   *mut std::ffi::c_void)
{
    let bytes = unsafe { std::slice::from_raw_parts(message as *const u8, length as usize) };
    let msg = std::str::from_utf8(bytes)
        .unwrap_or("<error parsing debug message>");
    eprintln!("gl: {msg}");
}

struct Scene<'a> {
    objs: &'a [bundle::ArchivedSceneObject],
    vao: GLuint,
    tex: GLuint,
}

impl<'a> Scene<'a> {
    fn load(
        gl: &Gl,
    //  bundle: &'a bundle::ArchivedRoot,
        scene: &'a bundle::ArchivedScene,
        image: &'a bundle::ArchivedImage,
    ) -> Self
    {
        let objs = &scene.objects[..];
        let vao = make_mesh(&gl, &*scene.mesh);
        let tex = make_texture(&gl, &*image);
        Scene{objs, vao, tex}
    }

    fn draw_objects(&self, gl: &Gl, params: impl IntoIterator<Item = (usize, uv::Vec3)>) {
        let params = params.into_iter();
        unsafe {
            gl.BindVertexArray(self.vao);
            gl.BindTexture(gl::TEXTURE_2D, self.tex);
            for (obj_i, translate) in params {
                let bundle::ArchivedSceneObject{xyz, start, count} = self.objs[obj_i];
                let uv::Vec3{x, y, z} = translate + uv::Vec3::from(uv::IVec3::from(xyz));
                gl.Uniform3f(4, x, y, z);
                gl.DrawElements(
                    gl::TRIANGLES,
                    count as _,
                    gl::UNSIGNED_INT,
                    (start * 4) as _
                );
            }
        }
    }

    fn draw_object(&self, gl: &Gl, index: usize, translate: uv::Vec3) {
        self.draw_objects(gl, [(1, translate)])
    }

    fn draw(&self, gl: &Gl) {
        let all = (0..self.objs.len())
            .map(|i| (i, uv::Vec3::zero()));
        self.draw_objects(gl, all);
    }
}

