use {
    crate::be::*,
    super::RawRgbx,
    anyhow::{Result as Anyhow, bail},
    bytemuck::{self as bm, AnyBitPattern},
};

pub fn build(prm: &[u8]) -> Anyhow<crate::ModelSet> {
    let scene = build_scene(prm)?;
    Ok(scene.mset)
}

pub fn build_scene(prm: &[u8]) -> Anyhow<crate::Scene> {
    let cursor = &mut &prm[..];
    let mut mset = crate::ModelSet::default();
    let mut sprites = crate::Sprites::default();

    while !cursor.is_empty() {
        let raw_obj: &RawObject = grab(cursor)?;
        let n_verts = raw_obj.n_verts.get() as usize;
        let n_prims = raw_obj.n_prims.get() as usize;
        let raw_verts: &[[Be<i16>; 4]] = grab_n(cursor, n_verts)?;

        let verts = raw_verts.iter().copied()
            .map(|[x,y,z,_]| [x,y,z].map(|x| x.get() as f32))
            .collect::<Vec<_>>();

        let mut faces = Vec::new();

        for _ in 0..n_prims {
            let raw_ty: &Be<u16> = grab(cursor)?;
            let _flags: &Be<u16> = grab(cursor)?;
            let ty = RawPrimType::parse(raw_ty.get())?;
            match ty {
                RawPrimType::Poly{quad, textured, smooth} => {
                    let n_verts = if quad {4} else {3};
                    let n_smooth = if smooth {n_verts} else {1};

                    let vis: &[Be<u16>] = grab_n(cursor, n_verts)?;

                    let (tex, ruv) = if textured {
                        let tex: &Be<u16> = grab(cursor)?;
                        let _: &[u16; 2] = grab(cursor)?;
                        let ruv: &[[u8; 2]] = grab_n(cursor, n_verts)?;
                        (tex.get(), ruv)
                    }
                    else {
                        (0xffff, &[[0u8; 2]; 4][..])
                    };

                    // align
                    if !(quad && !textured) { let _: &[u8; 2] = grab(cursor)?; }
                    let rgb: &[RawRgbx] = grab_n(cursor, n_smooth)?;

                    faces.push(crate::ModelFace {
                        vis: [2,1,0].map(|i| vis[i].get()),
                        rgb: [2,1,0].map(|i| rgb[i%n_smooth].into()),
                        ruv: [2,1,0].map(|i| ruv[i]),
                        tex,
                    });

                    if quad {
                        faces.push(crate::ModelFace {
                            vis: [2,3,1].map(|i| vis[i].get()),
                            rgb: [2,3,1].map(|i| rgb[i%n_smooth].into()),
                            ruv: [2,3,1].map(|i| ruv[i]),
                            tex,
                        });
                    }
                }

                RawPrimType::Tspr | RawPrimType::Bspr => {
                    let _raw_sprite: &RawSprite = grab(cursor)?;
                    // TODO
                }

                RawPrimType::Pad => {
                    let _: &[u16; 7] = grab(cursor)?;
                }

                _ => unimplemented!("other prim types")
            }
        }

        let pos = raw_obj.pos.map(|x| x.get() as f32);
        mset.push_object(pos, verts, faces);
    }

    Ok(crate::Scene{mset, sprites})
}

/*#[repr(C)]
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
}*/

/*#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RawSVector([Be<i16>; 4]);*/

#[repr(C)]
#[derive(Clone, Copy, AnyBitPattern)]
struct RawObject {
    name: [u8; 15],     //  16
    _pad_name: u8,
    n_verts: Be<u16>,   //  18
    _pad0: [u8; 14],    //  32
    n_prims: Be<u16>,   //  34
    _pad1a: [u8; 32],
    _pad1b: [u8; 32],
    _pad1c: [u8; 18],
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
    color: RawRgbx,
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

            9  => unimplemented!(),//Lines,
            10 => Tspr,
            11 => Bspr,
            20 => unimplemented!(),//Spline,
            21 => unimplemented!(),//DirLight,
            22 => unimplemented!(),//PointLight,
            23 => unimplemented!(),//SpotLight,

            _ => return Err(BadPrimType(raw))
        };
        Ok(pt)
    }
}

fn grab<'a, T> (cursor: &mut &'a [u8]) -> Anyhow<&'a T> where T: bm::AnyBitPattern {
    let len = std::mem::size_of::<T>();
    if cursor.len() < len {bail!("underrun");}
    let (bytes, rest) = cursor.split_at(len);
    *cursor = rest;
    Ok(bm::from_bytes(bytes))
}

fn grab_n<'a, T> (cursor: &mut &'a [u8], n: usize) -> Anyhow<&'a [T]> where T: bm::AnyBitPattern {
    let len = std::mem::size_of::<T>() * n;
    if cursor.len() < len {bail!("underrun");}
    let (bytes, rest) = cursor.split_at(len);
    *cursor = rest;
    Ok(bm::cast_slice(bytes))
}

