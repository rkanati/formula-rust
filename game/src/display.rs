use {
    crate::gl::prelude::*,
    winit::{
        event_loop::EventLoop,
        window::{WindowBuilder, Window},
    },
    glutin::{
        prelude::*,
        config::{Api, ConfigTemplateBuilder},
        context::{PossiblyCurrentContext, ContextAttributesBuilder, ContextApi},
        display::GetGlDisplay as _,
        surface::{SurfaceAttributesBuilder, WindowSurface, SwapInterval, Surface},
    },
    glutin_winit::DisplayBuilder,
    raw_window_handle::HasRawWindowHandle as _,
    ultraviolet as uv,
};

pub struct Display {
    win: Window,
    surf: Surface<WindowSurface>,
    ctx: PossiblyCurrentContext,
    gl: Gl,
}

impl Display {
    pub fn init(eloop: &EventLoop<()>) -> Self {
        let (win, cfg) = {
            let win_build = WindowBuilder::new()
                .with_title("foRmula'rS\"");
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

        let gl = Gl::load_with(|sym|
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
            gl.Enable(gl::DEPTH_TEST);
            gl.BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            gl.PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        }

        Display{win, surf, ctx, gl}
    }

    pub fn resize(&self, [w, h]: [u32; 2]) {
        self.surf.resize(&self.ctx, w.try_into().unwrap(), h.try_into().unwrap());
    }

    pub fn set_grab(&self, grab: bool) {
        self.win.set_cursor_visible(!grab);
        use winit::window::CursorGrabMode as Grab;
        if grab {
            self.win.set_cursor_grab(Grab::Locked)
                .or_else(|_| self.win.set_cursor_grab(Grab::Confined))
                .unwrap();
        }
        else {
            self.win.set_cursor_grab(Grab::None).unwrap();
        }
    }

    pub fn dims(&self) -> [u32; 2] {
        self.win.inner_size().into()
    }

    pub fn finish_frame(&self) {
        self.win.request_redraw();
        self.surf.swap_buffers(&self.ctx).unwrap();
    }
}

impl std::ops::Deref for Display {
    type Target = Gl;
    fn deref(&self) -> &Gl { &self.gl }
}

extern "system" fn on_gl_debug(
    _source: GLenum,
    _ty:     GLenum,
    _id:     GLuint,
    level:   GLenum,
    length:  GLsizei,
    message: *const GLchar,
    _user:   *mut std::ffi::c_void)
{
    let bytes = unsafe { std::slice::from_raw_parts(message as *const u8, length as usize) };
    let msg = std::str::from_utf8(bytes)
        .unwrap_or("<error parsing debug message>");
    let level = match level {
        gl::DEBUG_SEVERITY_HIGH    => log::Level::Error,
        gl::DEBUG_SEVERITY_MEDIUM  => log::Level::Warn,
        gl::DEBUG_SEVERITY_LOW     => log::Level::Info,
        _                          => log::Level::Debug,
    };
    log::log!(target: "gl", level, "{msg}");
}
