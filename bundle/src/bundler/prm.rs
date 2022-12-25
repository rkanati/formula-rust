use {
    crate::{bundle, bundler::atlas::Atlas},
    anyhow::{Result as Anyhow, Context as _, anyhow},
    bytemuck::{self as bm, Pod, Zeroable, AnyBitPattern},
    ultraviolet as uv,
};

pub struct Prm {
    pub objects: Vec<bundle::SceneObject>,
    pub mesh: bundle::Mesh,
}

impl Prm {
    pub fn load(bytes: &[u8], atlas: &Atlas) -> Anyhow<Prm> {
        let cursor = &mut &bytes[..];
        let mut objects = Vec::new();
        let mut mesh = bundle::Mesh::default();
        while !cursor.is_empty() {
            let object = load_object(cursor, &mut mesh, atlas)
                .map_err(|e| anyhow!(e))
                .context(format!("prm offset: {}", bytes.len() - cursor.len()))
                ?;
            objects.push(object);
        }

        Ok(Prm{objects, mesh})
    }
}

fn load_object(bytes: &mut &[u8], mesh: &mut bundle::Mesh, atlas: &Atlas)
    -> Anyhow<bundle::SceneObject>
{
    let cursor = &mut *bytes;
    let raw_object: RawObject = grab(cursor)?;
    dbg!(&raw_object);
    let verts: &[RawSVector] = grab_n(raw_object.n_verts.get() as usize, cursor)?;

    let start = mesh.idxs.len() as u32;

    for _ in 0..raw_object.n_prims.get() {
        let ty = RawPrimType::parse(grab(cursor)?)?;
        let flags: Be<u16> = grab(cursor)?;
        let flags = flags.get();

        match dbg!(ty) {
            RawPrimType::Poly{quad, textured, smooth, lit} => {
                assert!(!lit);
                let n_verts = if quad {4} else {3};
                let n_smooth = if smooth {n_verts} else {1};

                let vert_is: &[Be<u16>] = grab_n(n_verts, cursor)?;

                let uvs = if textured {
                    let tex: Be<u16> = grab(cursor)?;
                    let _: [u16; 2] = grab(cursor)?; // ignore "cba" and "tsb"
                    let mut rel_uivis = [[0x00_u8; 2]; 4];
                    rel_uivis[..n_verts].copy_from_slice(grab_n(n_verts, cursor)?);

                    let tex = tex.get() as usize;
                    rel_uivis.map(|off| atlas.lookup(tex, uv::Vec2::from(off.map(|u| u as f32))))
                }
                else {
                    [uv::Vec2::zero(); 4]
                };

                let mut colors = [RawRgbx::MAGENTA; 4];
                colors[..n_smooth].copy_from_slice(
                    if let Ok(src) = grab_n(n_smooth, cursor) { src }
                    else { let _ = cursor.take(..2); grab_n(n_smooth, cursor)? }
                );

                let elems = (0..n_verts)
                    .map(|i_vert| {
                        let xyz = {
                            let vert = verts.get(vert_is[i_vert].get() as usize)
                                .copied()
                                .unwrap_or(verts[0]);
                            let RawSVector([x,y,z,_]) = vert;
                            [x,y,z].map(|x| x.get() as i32)
                        };

                        let uv = uvs[i_vert].into();
                        let rgb = colors[i_vert as usize % n_smooth].0;
                        bundle::MeshVert{xyz, uv, rgb}
                    });

                let idx_base = mesh.verts.len() as u32;
                mesh.verts.extend(elems);

                if quad {
                    mesh.idxs.extend([0, 1, 3, 0, 2, 3].map(|i| idx_base + i));
                }
                else {
                    mesh.idxs.extend([0, 1, 2].map(|i| idx_base + i));
                }
            }

            RawPrimType::Tspr | RawPrimType::Bspr => {
                let _sprite: RawSprite = grab(cursor)?;
            }

            _ => todo!("other prim types"),
        }
    }

    let count = mesh.idxs.len() as u32 - start;

    let _name = String::from_utf8_lossy(&raw_object.name)
        .split_once(|ch: char| !ch.is_ascii_graphic())
        .map(|(name, _)| name)
        .unwrap_or("<no name>")
        .to_owned();


    *bytes = *cursor;
    Ok(bundle::SceneObject {
        xyz: raw_object.pos.map(Be::get),
        start,
        count,
    })
}

#[derive(Debug, thiserror::Error)]
enum GrabError {
    #[error("not enough bytes")]
    Underrun,

    #[error("cast error")]
    Cast(#[from] bm::PodCastError),
}

fn grab_ref<'a, T: AnyBitPattern> (bytes: &mut &'a [u8]) -> Result<&'a T, GrabError> {
    let mut bs = *bytes;
    let size = std::mem::size_of::<T>();
    let raw = bs.take(..size).ok_or(GrabError::Underrun)?;
    let value = bm::try_from_bytes(raw)?;
    *bytes = bs;
    Ok(value)
}

fn grab<T: AnyBitPattern> (bytes: &mut &[u8]) -> Result<T, GrabError> {
    let mut bs = *bytes;
    let size = std::mem::size_of::<T>();
    let raw = bs.take(..size).ok_or(GrabError::Underrun)?;
    let &value = bm::try_from_bytes(raw)?;
    *bytes = bs;
    Ok(value)
}

fn grab_n<'a, T: AnyBitPattern> (n: usize, bytes: &mut &'a [u8])
    -> Result<&'a [T], GrabError>
{
    let mut bs = *bytes;
    let size = std::mem::size_of::<T>() * n;
    let raw = bs.take(..size).ok_or(GrabError::Underrun)?;
    let value = bm::try_cast_slice(raw)?;
    *bytes = bs;
    Ok(value)
}

#[repr(transparent)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Be<T>(T) where T: Pod;

impl<T> std::fmt::Debug for Be<T> where T: Pod + std::fmt::Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Be({:?})", self.get())
    }
}

impl<T> Be<T> where T: Pod {
    fn get(mut self) -> T {
        bm::bytes_of_mut(&mut self.0).reverse();
        self.0
    }
}

impl<T> From<T> for Be<T> where T: Pod {
    fn from(mut x: T) -> Self {
        bm::bytes_of_mut(&mut x).reverse();
        Be(x)
    }
}


#[repr(C)]
#[derive(Debug, Clone, Copy, AnyBitPattern)]
struct RawTransform {
    m: [[Be<i16>; 3]; 3],
    _pad: u16,
    t: [Be<i32>; 3],
}

impl From<RawTransform> for uv::Isometry3 {
    fn from(raw: RawTransform) -> Self {
        let m = raw.m.map(|col| col.map(|e| e.get()));
        assert_eq!(m, [[4096,0,0], [0,4096,0], [0,0,4096]]);
        // TODO check column major?
        // FIXME here and elsewhere: snorm -> fp range subtleties
        let t = raw.t.map(|x| x.get() as f32);
        let translation = uv::Vec3::from(t);
        let m = raw.m.map(|col| col.map(|e| e.get() as f32 / 4096.));
        let rotation = uv::Mat3::from(m).into_rotor3();
        uv::Isometry3::new(translation, rotation)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawSVector([Be<i16>; 4]);

#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct RawRgbx([u8; 4]);

type DummyPtr = u32;

impl RawRgbx {
    const MAGENTA: Self = Self([0xff, 0x00, 0xff, 0xff]);
}

#[repr(C)]
#[derive(Clone, Copy, AnyBitPattern)]
struct RawObject {
    name: [u8; 16],
    n_verts: Be<u16>,
    _pad0: [u8; 14],
    n_prims: Be<u16>,
    _pad1: [u8; 25],
    _pad2: [u8; 25],
    _rel: [i32; 3],
    _pad3: [u8; 20],
    pos: [Be<i32>; 3],
    _pad4: [u8; 16],
}

impl std::fmt::Debug for RawObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawObject")
            .field("name", &String::from_utf8_lossy(&self.name))
            .field("n_verts", &self.n_verts)
            .field("n_prims", &self.n_prims)
            .field("pos", &self.pos)
            .finish()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, AnyBitPattern)]
struct RawSprite {
    coord: Be<i16>,
    width: Be<i16>,
    height: Be<i16>,
    texture: Be<u16>,
    color: RawRgbx,
}

#[derive(Debug)]
enum RawPrimType {
    Poly {
        quad: bool,
        textured: bool,
        smooth: bool,
        lit: bool,
    },
    Lines,
    Tspr,
    Bspr,
    Spline,
    DirLight,
    PointLight,
    SpotLight,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid primitive type {0}")]
struct BadPrimType(u16);

impl RawPrimType {
    fn parse(raw: Be<u16>) -> Result<Self, BadPrimType> {
        raw.try_into()
    }
}

impl TryFrom<Be<u16>> for RawPrimType {
    type Error = BadPrimType;
    fn try_from(raw: Be<u16>) -> Result<Self, BadPrimType> {
        let raw = raw.get();
        use RawPrimType::*;
        let pt = match raw {
            1..=8 | 12..=19 => {
                let lit = raw > 8;
                let raw = if lit {raw - 12} else {raw - 1};
                let textured = raw & 1 != 0;
                let quad     = raw & 2 != 0;
                let smooth   = raw & 4 != 0;
                Poly{quad, textured, smooth, lit}
            }

            9  => Lines,
            10 => Tspr,
            11 => Bspr,
            20 => Spline,
            21 => DirLight,
            22 => PointLight,
            23 => SpotLight,

            _ => return Err(BadPrimType(raw))
        };
        Ok(pt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OctI16(pub i16, pub i16);

impl From<[i16; 3]> for OctI16 {
    fn from(n: [i16; 3]) -> Self {
        let n = n.map(|x| x as f32 / 4096.);
        let scale = 1. / (n[0].abs() + n[1].abs() + n[2].abs());
        let p = [n[0] * scale, n[1] * scale];
        let ij = if n[2] <= 0. {
            [p[1], p[0]].zip(p)
                .map(|(q, p)| (1. - q.abs()) * p.signum())
        }
        else {
            p
        };
        let [i, j] = ij.map(|x| (x * i16::MAX as f32) as i16);
        OctI16(i, j)
    }
}

