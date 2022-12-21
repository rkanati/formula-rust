
use std::collections::HashMap;

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Atlas {
    pub image: Id,//<Image>,
    pub uvs: Vec<[f32; 4]>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Image {
    pub wide:   u16,
    pub high:   u16,
    pub levels: Vec<Vec<u8>>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct MeshVert {
    pub xyz: [i32; 3], // 12
    pub rgb: [u8;  4], // 16
    pub uv:  [f32; 2], // 24
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Mesh {
    pub verts: Vec<MeshVert>,
    pub idxs:  Vec<u32>,
}

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Track {
    pub mesh: Id,//<Mesh>,
    pub atlas: Id,//<Atlas>,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, rkyv::Archive, rkyv::Serialize)]
#[archive(as = "Self")]
pub struct Id/*<A>*/(u64/*, std::marker::PhantomData<A>*/);

#[derive(rkyv::Archive, rkyv::Serialize)]
pub struct Assets<T> {
    assets: HashMap<u64, T>,
}

impl<A> Default for Assets<A> {
    fn default() -> Self {
        Self{assets: HashMap::new()}
    }
}

impl<A> Assets<A> {
    pub fn add(&mut self, name: &str, asset: A) -> Id/*<A>*/ {
        let id = asset_id(name);
        let None = self.assets.insert(id.0, asset) else {panic!("asset id collision!")};
        //Id(id, std::marker::PhantomData)
        id
    }
}

impl<A: rkyv::Archive> std::ops::Index<Id/*<A>*/> for ArchivedAssets<A> {
    type Output = A::Archived;

    fn index(&self, i: Id/*<A>*/) -> &A::Archived {
        &self.assets[&i.0]
    }
}

#[derive(Default, rkyv::Archive, rkyv::Serialize)]
pub struct Root {
    pub tracks: Assets<Track>,
    pub images: Assets<Image>,
    pub meshes: Assets<Mesh>,
    pub atlases: Assets<Atlas>,
}

/*#[derive(Debug)]
pub struct BakeError(Box<dyn std::error::Error + Send + Sync + 'static>);

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for BakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}*/

impl Root {
    pub fn new() -> Self {
        Self::default()
    }

    //pub fn bake<W> (self, to: W) -> Result<W, BakeError> where
    pub fn bake<W> (self, to: W) -> anyhow::Result<W> where
        W: std::io::Write,
    {
        use rkyv::ser::{serializers as sers, Serializer as _};
        let ser = sers::WriteSerializer::new(to);
        let scratch = sers::AllocScratch::new();
        let mut ser = sers::CompositeSerializer::new(ser, scratch, rkyv::Infallible);
        ser.serialize_value(&self)?;//.map_err(|e| BakeError(Box::new(e)))?;
        Ok(ser.into_serializer().into_inner())
    }
}

pub const fn asset_id/*<A>*/ (s: &str) -> Id/*<A>*/ {
    Id(util::fnv1a_64(s.as_bytes()))
}

