#![feature(
    array_chunks,
    array_zip,
    int_roundings,
    iter_array_chunks,
    iter_collect_into,
    result_option_inspect,
    slice_take,
    try_blocks,
)]

mod lzss;
mod be;
pub mod bundler;

mod reexports {
    pub use lz4_flex;
    pub use rapid_qoi;
}
pub use reexports::*;

use std::{
    collections::HashMap,
    rc::Rc,
};

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Path {
    pub points: Vec<[f32; 3]>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub enum ImageCoding {
    Flat,
    Qoi,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Image {
    pub wide:   u16,
    pub high:   u16,
    pub coding: ImageCoding,
    pub data: Vec<u8>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct MeshVert {
    pub xyz: [f32; 3], // 12
    pub rgb: [un8; 4], // 16
    pub uv:  [un16; 2], // 20
}

#[derive(Default, rkyv::Archive, rkyv::Serialize)]
pub struct Mesh {
    pub verts: Vec<MeshVert>,
    pub idxs:  Vec<u32>,
}

// TODO gross. improve.
#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Sprite {
    pub xyz: [f32; 3],
    pub wh:  [f32; 2],
    pub rgb: [un8; 4],
    pub uvs: [un16; 4],
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Scene {
    pub mesh: Rc<Mesh>,
    pub objects: Vec<SceneObject>,
    pub sprites: Vec<Sprite>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct SceneObject {
    pub xyz: [i32; 3],
    pub start: u32,
    pub count: u32,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct TrackNode {
    pub prev: u32,
    pub next: [u32; 2],
    pub center: [f32; 3],
}

pub type TrackGraph = Vec<TrackNode>;

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Track {
    pub road_mesh: Rc<Mesh>,
    pub road_image: Rc<Image>,
    pub scenery_scene: Rc<Scene>,
    pub scenery_image: Rc<Image>,
    pub sky_scene: Rc<Scene>,
    pub sky_image: Rc<Image>,
    pub graph: TrackGraph,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Glyph {
    pub start: u32,
    pub count: u32,
    pub offset: [f32; 2],
    pub advance: f32,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Font {
    pub verts:  Vec<[f32; 3]>,
    pub idxs:   Vec<u32>,
    pub glyphs: HashMap<char, Glyph>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Root {
    pub tracks: Assets<Track>,
    pub ship_scene: Rc<Scene>,
    pub ship_image: Rc<Image>,
    pub fonts: HashMap<String, Font>,
}

impl Root {
    pub fn bake<W> (self, to: &mut W) -> anyhow::Result<()> where
        W: std::io::Write,
    {
        use rkyv::ser::{serializers as sers, Serializer as _};
        let ser = sers::WriteSerializer::new(to);
        let scratch = sers::AllocScratch::new();
        let shared = sers::SharedSerializeMap::new();
        let mut ser = sers::CompositeSerializer::new(ser, scratch, shared);
        ser.serialize_value(&self)?;
        Ok(())
    }

    pub fn from_bytes(bytes: &[u8]) -> &ArchivedRoot {
        unsafe { rkyv::archived_root::<Self>(bytes) }
    }
}

pub type Bundle = ArchivedRoot;

pub type Assets<T> = HashMap<String, T>;

#[derive(Clone, Copy, rkyv::Archive, rkyv::Serialize)]
#[archive_attr(repr(transparent), derive(Debug, Clone, Copy))]
pub struct UNorm16(pub u16);

#[allow(dead_code)]
static CHECK_UNORM16_COMPILE: [(); 2-std::mem::size_of::<ArchivedUNorm16>()] = [];

#[derive(Clone, Copy, rkyv::Archive, rkyv::Serialize)]
#[archive_attr(repr(transparent), derive(Debug, Clone, Copy))]
pub struct UNorm8(pub u8);

#[allow(dead_code)]
static CHECK_UNORM8_COMPILE: [(); 1-std::mem::size_of::<ArchivedUNorm8>()] = [];

impl UNorm16 { pub fn new(x: f32) -> Self { Self((x.fract() * 65535.).round() as u16) } }
impl UNorm8  { pub fn new(x: f32) -> Self { Self((x.fract() *   255.).round() as u8) } }

impl From<UNorm16> for f32 { fn from(UNorm16(y): UNorm16) -> Self { y as f32 / 65_535. } }
impl From<UNorm8>  for f32 { fn from(UNorm8(y):   UNorm8) -> Self { y as f32 /    255. } }
impl From<ArchivedUNorm16> for f32 { fn from(y: ArchivedUNorm16) -> Self { y.0 as f32 / 65_535. } }
impl From<ArchivedUNorm8>  for f32 { fn from(y:  ArchivedUNorm8) -> Self { y.0 as f32 /    255. } }

#[allow(non_camel_case_types)]
pub type un16 = UNorm16;

#[allow(non_camel_case_types)]
pub type un8 = UNorm8;

