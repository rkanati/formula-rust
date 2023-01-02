pub mod api {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/gl_binds.rs"));
}

pub mod prelude {
    pub use super::api::{Gles2 as Gl, types::*, self as gl};
}

