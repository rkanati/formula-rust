use {
    crate::{
        bundler::atlas::Atlas,
        be::Be,
        un16,
    },
    anyhow::{Result as Anyhow, Context as _, anyhow},
    bytemuck::{self as bm, Pod, Zeroable, AnyBitPattern},
    ultraviolet as uv,
};

pub struct Prm {
    pub objects: Vec<crate::SceneObject>,
    pub sprites: Vec<crate::Sprite>,
    pub mesh: crate::Mesh,
}

impl Prm {
    pub fn load(bytes: &[u8], debug_name: &str, atlas: &Atlas) -> Anyhow<Prm> {
        log::debug!(target: "prm", "loading prm '{debug_name}'");

        let cursor = &mut &bytes[..];
        let mut objects = Vec::new();
        let mut sprites = Vec::new();
        let mut mesh = crate::Mesh::default();
        while !cursor.is_empty() {
            let offset = bytes.len() - cursor.len();
            let object = load_object(bytes.len(), cursor, &mut mesh, &mut sprites, atlas)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("prm offset: {offset:#0x}"))?;
            objects.push(object);
        }

        Ok(Prm{objects, sprites, mesh})
    }
}

fn load_object(
    prm_len: usize, bytes: &mut &[u8],
    mesh: &mut crate::Mesh,
    sprites: &mut Vec<crate::Sprite>,
    atlas: &Atlas,
) -> Anyhow<crate::SceneObject>
{
    let cursor = &mut *bytes;

    log::trace!(target: "prm", "grabbing object at +{off:x}", off = prm_len - cursor.len());
    let raw_object: RawObject = grab(cursor)?;
    log::trace!(target: "prm", "object name '{obj_name}'", obj_name = raw_object.name());

    let n_verts = raw_object.n_verts.get() as usize;
    log::trace!(target: "prm", "{n_verts} vertices at +{off:x}", off = prm_len - cursor.len());
    let verts: &[RawSVector] = grab_n(n_verts, cursor)?;

    let obj_xyz = raw_object.pos.map(Be::get);

    let start = mesh.idxs.len() as u32;

    for _ in 0..raw_object.n_prims.get() {
        log::trace!(target: "prm", "grabbing primitive at +{off:x}", off = prm_len - cursor.len());
        let raw_ty: Be<u16> = grab(cursor)?;
        let ty = RawPrimType::parse(raw_ty.get())?;
        let flags: Be<u16> = grab(cursor)?;
        let flags = flags.get();
        let _one_sided   = flags & 1 != 0;
        let ship_engine = flags & 2 != 0;
        let _translucent = flags & 4 != 0;
        log::trace!(target: "prm", "primitive type {raw_ty}: {ty:?}, flags {flags:03b}",
            raw_ty = raw_ty.get());

        //eprintln!("+{off:#x}: {ty:?}");
        match ty {
            RawPrimType::Poly{quad, textured, smooth} => {
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
                    [atlas.no_texture().xy(); 4]
                };

                let mut colors = [super::RawRgbx::MAGENTA; 4];
                colors[..n_smooth].copy_from_slice(
                    if let Ok(src) = grab_n(n_smooth, cursor) { src }
                    else { let _ = cursor.take(..2); grab_n(n_smooth, cursor)? }
                );

                for color in &mut colors {
                    if ship_engine {*color = super::RawRgbx([128, 64, 0, 255]);}
                }

                let elems = (0..n_verts)
                    .map(|i_vert| {
                        let xyz = {
                            let vert = verts.get(vert_is[i_vert].get() as usize)
                                .copied()
                                .unwrap_or(verts[0]);
                            let RawSVector([x,y,z,_]) = vert;
                            [x,y,z].map(|x| x.get() as f32)
                        };

                        let uv: [f32; 2] = uvs[i_vert].into();
                        let uv = uv.map(un16::new);
                        let rgb = colors[i_vert as usize % n_smooth].into();
                        crate::MeshVert{xyz, uv, rgb}
                    });

                let idx_base = mesh.verts.len() as u32;
                mesh.verts.extend(elems);

                if quad {
                    mesh.idxs.extend([2, 1, 0, 2, 3, 1].map(|i| idx_base + i));
                }
                else {
                    mesh.idxs.extend([2, 1, 0].map(|i| idx_base + i));
                }
            }

            RawPrimType::Tspr | RawPrimType::Bspr => {
                let sprite: RawSprite = grab(cursor)?;
                let wh = [sprite.width, sprite.height].map(|x| x.get() as f32);
                let RawSVector([x,y,z,_]) = verts.get(sprite.vertex.get() as usize).copied()
                    .unwrap_or(verts[0]);
                let dy = wh[1] * if let RawPrimType::Tspr = ty {0.5} else {-0.5};
                let xyz = [
                    x.get() as f32 + obj_xyz[0] as f32,
                    y.get() as f32 + obj_xyz[1] as f32 + dy,
                    z.get() as f32 + obj_xyz[2] as f32,
                ];
                let rgb = sprite.color.into();
                let uvs: [f32; 4] = atlas.lookup_rect(sprite.texture.get() as usize).into();
                let uvs = uvs.map(un16::new);
                sprites.push(crate::Sprite{xyz, wh, rgb, uvs});
            }

            RawPrimType::Pad => {
                let _: [u16; 7] = grab(cursor)?;
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
    Ok(crate::SceneObject {
        xyz: obj_xyz,
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

#[repr(C)]
#[derive(Clone, Copy, AnyBitPattern)]
struct RawObject {
    name: [u8; 15],     //  16
    _pad_name: u8,
    n_verts: Be<u16>,   //  18
    _pad0: [u8; 14],    //  32
    n_prims: Be<u16>,   //  34
    _pad1: [u8; 82],
    pos: [Be<i32>; 3],  // 128
    _pad4: [u8; 16],    // 144
}

impl RawObject {
    fn name(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim()
            .trim_end_matches(|ch: char| !ch.is_ascii_graphic())
            .into()
    }
}

impl std::fmt::Debug for RawObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawObject")
            .field("name", &self.name())
            .field("n_verts", &self.n_verts)
            .field("n_prims", &self.n_prims)
            .field("pos", &self.pos)
            .finish()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, AnyBitPattern)]
struct RawSprite {
    vertex: Be<u16>,
    width: Be<u16>,
    height: Be<u16>,
    texture: Be<u16>,
    color: super::RawRgbx,
}

#[derive(Debug, Clone, Copy)]
enum RawPrimType {
    Pad,
    Poly {
        quad: bool,
        textured: bool,
        smooth: bool,
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
#[error("invalid primitive type {0:#x}")]
struct BadPrimType(u16);

impl RawPrimType {
    fn parse(raw: u16) -> Result<Self, BadPrimType> {
        raw.try_into()
    }
}

impl TryFrom<u16> for RawPrimType {
    type Error = BadPrimType;
    fn try_from(raw: u16) -> Result<Self, BadPrimType> {
        use RawPrimType::*;
        let pt = match raw {
            0 => Pad,

            1..=8 => {
                let raw = raw - 1;
                let textured = raw & 1 != 0;
                let quad     = raw & 2 != 0;
                let smooth   = raw & 4 != 0;
                Poly{quad, textured, smooth}
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

