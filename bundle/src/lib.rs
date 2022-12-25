#![feature(array_chunks)]
#![feature(array_zip)]
#![feature(iter_array_chunks)]
#![feature(iter_collect_into)]
#![feature(slice_take)]

mod bundle;
pub use bundle::*;

mod lzss;

pub mod bundler;

pub use lz4_flex;

