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
    //pub use rapid_qoi;
    pub use qoit;
    pub use util::unorm::*;
}
pub use reexports::*;

use std::collections::HashMap;


#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Root {
    pub tracks: HashMap<String, Track>,
    pub ship_mset: ModelSet,
    pub ship_iset: ImageSet,
    pub fonts: HashMap<String, Font>,
    pub aux_table: HashMap<String, u64>,
}

pub type Bundle = ArchivedRoot;

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

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Path {
    pub points: Vec<[f32; 3]>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct ImageSet {
    pub sizes: Vec<(u16, u16)>,
    pub qoi_stream: Vec<u8>,
}

#[derive(Default, rkyv::Archive, rkyv::Serialize)]
pub struct ModelSet {
    pub verts: Vec<[f32; 3]>,

    pub face_vis: Vec<[u16;       3]>,
    pub face_rgb: Vec<[[un8;  3]; 3]>,
    pub face_ruv: Vec<[[u8; 2];   3]>,
    pub face_tex: Vec<u16>,

    pub obj_xyz: Vec<[f32; 3]>,
    pub obj_face_0: Vec<u16>,
}

impl ModelSet {
    fn push_object(&mut self, at: [f32; 3], verts: Vec<[f32; 3]>, faces: Vec<ModelFace>) {
        let v0 = self.verts.len() as u16;
        self.verts.extend_from_slice(&verts);

        let f0 = self.face_vis.len() as u16;
        for face in faces {
            self.face_vis.push(face.vis.map(|vi| vi + v0));
            self.face_rgb.push(face.rgb);
            self.face_ruv.push(face.ruv);
            self.face_tex.push(face.tex);
        }

        self.obj_xyz.push(at);
        self.obj_face_0.push(f0);
    }
}

#[derive(Clone, Copy)]
struct ModelFace {
    vis: [u16; 3],
    rgb: [[un8; 3]; 3],
    ruv: [[u8; 2]; 3],
    tex: u16,
}

#[derive(Default, rkyv::Archive, rkyv::Serialize)]
pub struct Sprites {
    pub xyz: Vec<[f32; 3]>,
    pub wh:  Vec<[f32; 2]>,
    pub rgb: Vec<[un8; 3]>,
    pub tex: Vec<u16>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Scene {
    pub mset: ModelSet,
    pub sprites: Sprites,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct TrackNode {
    pub prev: u32,
    pub next: [u32; 2],
    pub center: [f32; 3],
}

pub type TrackGraph = Vec<TrackNode>;

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct RoadModel {
    pub verts: Vec<[f32; 3]>,

    pub f_verts: Vec<[u16; 4]>,
    pub f_tex:   Vec<u8>,
    pub f_flags: Vec<u8>,
    pub f_rgb:   Vec<[un8; 3]>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Track {
    pub road_model: RoadModel,
    pub road_iset: ImageSet,
    pub scenery_scene: Scene,
    pub scenery_iset: ImageSet,
    pub sky_mset: ModelSet,
    pub sky_iset: ImageSet,
    pub graph: TrackGraph,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Font {
    pub points: Vec<[f32; 2]>,
    pub paths:  Vec<PathSeg>,
    pub glyphs: Vec<Glyph>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Glyph {
    pub start: u32,
    pub offset: [f32; 2],
    pub advance: f32,
}

pub const GLYPH_RANGES: &[std::ops::RangeInclusive<char>] = &[
    ' ' ..= '~',
    '©' ..= '©',
    '®' ..= '®',
    '™' ..= '™',
];

#[derive(rkyv::Archive, rkyv::Serialize)]
#[archive_attr(repr(u8))]
pub enum PathSeg {
    Start     = 0,
    Line      = 1,
    Quadratic = 2,
    Cubic     = 3,
}

impl std::fmt::Debug for PathSeg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start     => write!(f, "S"),
            Self::Line      => write!(f, "L"),
            Self::Quadratic => write!(f, "Q"),
            Self::Cubic     => write!(f, "C"),
        }
    }
}

