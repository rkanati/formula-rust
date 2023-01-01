mod debug;
pub use debug::DebugCam;

mod flythru;
pub use flythru::FlythruCam;

use ultraviolet as uv;

pub enum Camera {
    Debug(DebugCam),
    Flythru(FlythruCam),
}

impl Camera {
    pub fn update(&mut self) -> uv::Isometry3 {
        match self {
            Camera::Debug(c) => c.update(),
            Camera::Flythru(c) => c.update(),
        }
    }

    pub fn button(&mut self, button: crate::input::Button, down: bool) {
        match self {
            Camera::Debug(c) => c.button(button, down),
            Camera::Flythru(_) => { }
        }
    }

    pub fn mouse(&mut self, delta: [f32; 2]) {
        match self {
            Camera::Debug(c) => c.mouse(delta),
            Camera::Flythru(_) => { }
        }
    }
}

impl From<DebugCam> for Camera {
    fn from(c: DebugCam) -> Self {
        Self::Debug(c)
    }
}

impl From<FlythruCam> for Camera {
    fn from(c: FlythruCam) -> Self {
        Self::Flythru(c)
    }
}

