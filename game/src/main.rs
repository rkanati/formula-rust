#![feature(array_windows)]
#![feature(array_zip)]
#![feature(get_many_mut)]
#![feature(int_roundings)]
#![feature(iter_array_chunks)]
#![feature(iter_collect_into)]
#![feature(slice_take)]

mod atlas;
mod camera;
mod display;
mod font;
mod gl;
mod input;
mod render;
mod road;

use {
    input::Button,
    display::Display,
    font::Font,
    camera::*,
    winit::event_loop::EventLoop,
    ultraviolet as uv,
};

const VFOV_DEG: f32 = 57.;

fn main() {
    log_init(log::LevelFilter::Debug);

    // process arguments
    let track_name = std::env::args().nth(1);
    log::info!("selected track: {track_name:?}");

    // bring up graphics
    let eloop = EventLoop::new();
    let display = Display::init(&eloop);
    let shader = render::BasicShader::create(&display);

    // start loading assets
    let bundle = {
        let decomp_start = std::time::Instant::now();

        let compressed = include_bytes!(env!("BUNDLE_PATH"));
        let bytes = bundle::lz4_flex::decompress_size_prepended(compressed).unwrap();
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let bundle = bundle::Root::from_bytes(&bytes);

        log::info!("bundle decompressed: {} -> {} MiB; took {}s",
            compressed.len() >> 20,
            bytes.len() >> 20,
            decomp_start.elapsed().as_secs());

        /*let fonts_size = bundle.fonts.values()
            .map(|f| std::mem::size_of_val(&f.verts[..]) + std::mem::size_of_val(&f.idxs[..]))
            .sum::<usize>();
        log::debug!("font data size: {fonts_size}");*/

        let aux = include_bytes!(concat!(env!("BUNDLE_PATH"), "-aux"));

        bundle
    };

    let blank_tex = render::make_blank_texture(&display);

    let (track_name, track) = if let Some(track_name) = &track_name {
        (&track_name[..], &bundle.tracks[track_name.as_str()])
    }
    else {
        let bytes = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap()
            .as_nanos().to_ne_bytes();
        let i = util::fnv1a_64(&bytes) as usize % bundle.tracks.len();
        bundle.tracks.iter().nth(i).map(|(n,t)| (n.as_str(), t)).unwrap()
    };
    log::info!("loading {track_name}");

    let road_mesh = {
        log::debug!("loading {track_name}: road");
        let atlas = atlas::Atlas::build(&display, &track.road_iset, "road").unwrap();
        road::RoadMesh::build(&display, &track.road_model, atlas)
    };

    let scenery = {
        log::debug!("loading {track_name}: scenery");
        let atlas = atlas::Atlas::build(&display, &track.scenery_iset, "scenery").unwrap();
        render::ModelSet::load(&display, &track.scenery_scene.mset, atlas).unwrap()
    };

    let sky = {
        log::debug!("loading {track_name}: sky");
        let atlas = atlas::Atlas::build(&display, &track.sky_iset, "sky").unwrap();
        render::ModelSet::load(&display, &track.sky_mset, atlas).unwrap()
    };

    let ships = {
        log::debug!("loading {track_name}: ships");
        let atlas = atlas::Atlas::build(&display, &bundle.ship_iset, "ships").unwrap();
        render::ModelSet::load(&display, &bundle.ship_mset, atlas).unwrap()
    };

    for k in bundle.fonts.keys() {
        log::debug!("font: {k}");
    }

    let fonts = ["Amalgama", "2097", "Fusion", "supErphoniX2", "WO3", "X2"]
        .into_iter()
        .map(|name| Font::load(&display, &bundle.fonts[name], blank_tex))
        .collect::<Result<Vec<_>, _>>().unwrap();

    let _title = fonts[0].bake_run(font::Anchor::Center, "formula'rs\"");
    let silly = fonts[0].bake_run(font::Anchor::Center, "Hold © tight!");
    let man = fonts[0].bake_run(font::Anchor::Center, "©");

    //let mut cam = Camera::from(FlythruCam::new(&track.graph[..]));
    let mut cam = Camera::from(DebugCam::new());
    let mut ctrl_turning = false;

    eloop.run(move |ev, _, flow| {
        flow.set_poll();

        use winit::event::{Event as Ev, WindowEvent as WinEv, DeviceEvent as DevEv};
        match ev {
            Ev::WindowEvent{event: win_ev, ..} => match win_ev {
                WinEv::CloseRequested => flow.set_exit(),

                WinEv::Resized(size) => display.resize(size.into()),

                WinEv::KeyboardInput{input, ..} => {
                    use winit::event::VirtualKeyCode as Vk;
                    let pressed = input.state == winit::event::ElementState::Pressed;
                    match input.virtual_keycode {
                        Some(Vk::W)        => cam.button(Button::Forward, pressed),
                        Some(Vk::S)        => cam.button(Button::Back, pressed),
                        Some(Vk::D)        => cam.button(Button::Right, pressed),
                        Some(Vk::A)        => cam.button(Button::Left, pressed),
                        Some(Vk::LControl) => cam.button(Button::Descend, pressed),
                        Some(Vk::Space)    => cam.button(Button::Ascend, pressed),
                        Some(Vk::LShift)   => cam.button(Button::Fast, pressed),
                        _ => { }
                    }
                }

                WinEv::MouseInput{button, state, ..} => {
                    use winit::event::MouseButton as Mb;
                    let pressed = state == winit::event::ElementState::Pressed;
                    if let Mb::Left | Mb::Right = button {
                        ctrl_turning = pressed;
                        display.set_grab(pressed);
                    }
                }

                _ => { }
            }

            Ev::DeviceEvent{event, ..} => match event {
                DevEv::MouseMotion{delta: (dx, dy)} if ctrl_turning => {
                    cam.mouse([dx, dy].map(|x| x as f32));
                }

                _ => { }
            }

            Ev::RedrawEventsCleared => {
                let cam_xform = cam.update();

                let [w, h] = display.dims().map(|x| x as i32);
                let aspect = w as f32 / h as f32;

                let eye_to_clip = {
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
                    * cam_xform.rotation.reversed().into_matrix().into_homogeneous();

                let world_to_clip
                    = eye_to_clip
                    * cam_xform.rotation.reversed().into_matrix().into_homogeneous()
                    * uv::Mat4::from_translation(-cam_xform.translation);

                let ui_to_clip = {
                    let vfov: f32 = 90f32.to_radians();
                    let proj = uv::projection::perspective_gl(
                        vfov,
                        aspect,
                        0.1,
                        1000.0,
                    );

                    let hh = (vfov * 0.5).tan();
                    let rescale = uv::Mat4::from_nonuniform_scale(uv::Vec3::new(1., 1., -1.));
                    let shift = uv::Mat4::from_translation(uv::Vec3::unit_z() * (1./hh));

                    proj * rescale * shift
                };

                unsafe {
                    use gl::api as gl;
                    let gl = &display;

                    gl.ClearColor(0.04, 0.0, 0.08, 1.);
                    gl.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

                    gl.Viewport(0, 0, w, h);

                    shader.select(gl, sky_to_clip);
                    gl.Enable(gl::CULL_FACE);
                    gl.Disable(gl::DEPTH_TEST);
                    gl.DepthMask(gl::FALSE);
                    sky.draw(gl, &shader);//, cam_xform.translation);

                    shader.select(gl, world_to_clip);
                    gl.Enable(gl::DEPTH_TEST);
                    gl.DepthMask(gl::TRUE);
                    scenery.draw(gl, &shader);//, cam_xform.translation);

                    let ship_params = (0..ships.object_count())
                        .map(|i| (i, uv::Vec3::unit_x() * 800. * i as f32));
                    ships.draw_objects(gl, &shader, ship_params);

                    gl.Disable(gl::CULL_FACE);
                    gl.VertexAttrib4f(1, 1., 1., 0., 1.);
                    for (i, font) in fonts.iter().enumerate() {
                        font.draw_glyphs_test(
                            gl,
                            uv::Vec3::new(-5_000., 1500. + 100. * i as f32, 0.)
                        );
                    }

                    road_mesh.draw(gl, &shader);

                    gl.Disable(gl::DEPTH_TEST);
                    shader.select(gl, ui_to_clip);
                    gl.VertexAttrib4f(1, 1., 1., 0., 1.);
                    //title.draw(gl, 0.3, 0.1, uv::Vec3::new(0., 0.7, 0.));
                    //silly.draw(gl, &shader, 0.2, 0.1, uv::Vec3::new(0., -2.8, 2.));
                }

                display.finish_frame();
            }

            _ => { }
        }
    });
}

fn log_init(filter: log::LevelFilter) {
    use simplelog::*;
    let simple = TermLogger::new(
        filter,
        Config::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    );
    /*let file = WriteLogger::new(
        log::LevelFilter::Debug,
        Config::default(),
        std::fs::File::create(concat!(env!("CARGO_MANIFEST_DIR"), "/build-script.log")).unwrap()
    );*/
    CombinedLogger::init(vec![simple, /*file*/]).unwrap();
}

