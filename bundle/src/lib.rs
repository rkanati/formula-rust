#![feature(array_chunks)]
#![feature(array_zip)]
#![feature(int_roundings)]
#![feature(iter_array_chunks)]
#![feature(iter_collect_into)]
#![feature(slice_take)]
#![feature(try_blocks)]

mod bundle;
pub use bundle::*;

mod lzss;

pub mod bundler;

pub use lz4_flex;

