#![allow(dead_code)]

use anyhow::Result as Anyhow;
use bytemuck::{self as bm, Pod, Zeroable};

struct PolyType {
    n: usize,
    tex: bool,
    smooth: bool,
    lit: bool,
}

enum PrimType {
    Poly(PolyType),
    Lines,
    Tspr,
    Bspr,
    Spline,
    DirLight,
    PointLight,
    SpotLight,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid primitive type")]
struct BadPrimType;

impl TryFrom<u16> for PrimType {
    type Error = BadPrimType;
    fn try_from(raw: u16) -> Result<Self, BadPrimType> {
        let pt = match raw {
            1..=8 | 12..=19 => {
                let lit = raw > 8;
                let raw = if lit {raw - 12} else {raw - 1};
                let tex    = raw & 1 != 0;
                let quad   = raw & 2 != 0;
                let smooth = raw & 4 != 0;
                let n = if quad {4} else {3};
                PrimType::Poly(PolyType{n, tex, smooth, lit})
            }

            9  => PrimType::Lines,
            10 => PrimType::Tspr,
            11 => PrimType::Bspr,
            20 => PrimType::SpotLight,
            21 => PrimType::DirLight,
            22 => PrimType::PointLight,
            23 => PrimType::SpotLight,

            _ => return Err(BadPrimType)
        };
        Ok(pt)
    }
}

#[derive(Debug, thiserror::Error)]
enum GrabError {
    #[error("not enough bytes")]
    Underrun,

    #[error("cast error")]
    Cast(#[from] bm::PodCastError),
}

fn grab_ref<'a, T: Pod> (bytes: &mut &'a [u8]) -> Result<&'a T, GrabError> {
    let mut bs = *bytes;
    let size = std::mem::size_of::<T>();
    let raw = bs.take(..size).ok_or(GrabError::Underrun)?;
    let value = bm::try_from_bytes(raw)?;
    *bytes = bs;
    Ok(value)
}

fn grab<T: Pod> (bytes: &mut &[u8]) -> Result<T, GrabError> {
    let mut bs = *bytes;
    let size = std::mem::size_of::<T>();
    let raw = bs.take(..size).ok_or(GrabError::Underrun)?;
    let &value = bm::try_from_bytes(raw)?;
    *bytes = bs;
    Ok(value)
}

fn grab_n<'a, T: Pod> (n: usize, bytes: &mut &'a [u8]) -> Result<&'a [T], GrabError> {
    let mut bs = *bytes;
    let size = std::mem::size_of::<T>() * n;
    let raw = bs.take(..size).ok_or(GrabError::Underrun)?;
    let value = bm::try_cast_slice(raw)?;
    *bytes = bs;
    Ok(value)
}

#[repr(C, align(4))]
struct Rgbx([u8; 4]);

/*
    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C, packed)]
    struct TexInfo<const N: usize> {
        tex: i16,
        cba: i16,
        tsb: i16,
        uvs: [[u8; 2]; N],
    }

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C)]
    struct Poly<const N: usize, const C: usize> {
        verts: [i16; N],
        color: [Rgbx; C],
    }

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C)]
    struct PolyTex<const N: usize, const C: usize> {
        verts: [i16; N],
        tex: TexInfo<N>,
        color: [Rgbx; C],
    }

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C)]
    struct PolyLit<const N: usize, const C: usize> {
        verts: [i16; N],
        norms: [i16; C],
        color: [Rgbx; C],
    }

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C)]
    struct PolyLitTex<const N: usize, const C: usize> {
        verts: [i16; N],
        norms: [i16; C],
        tex: TexInfo<N>,
        color: [Rgbx; C],
    }


    type F3 = Poly<3, 1>;
    type F4 = Poly<4, 1>;
    type Ft3 = PolyTex<3, 1>;
    type Ft4 = PolyTex<4, 1>;
    type G3 = Poly<3, 3>;
    type G4 = Poly<4, 4>;
    type Gt3 = PolyTex<3, 3>;
    type Gt4 = PolyTex<4, 4>;

    type LsF3 = PolyLit<3, 1>;
    type LsF4 = PolyLit<4, 1>;
    type LsFt3 = PolyLitTex<3, 1>;
    type LsFt4 = PolyLitTex<4, 1>;
    type LsG3 = PolyLit<3, 3>;
    type LsG4 = PolyLit<4, 4>;
    type LsGt3 = PolyLitTex<3, 3>;
    type LsGt4 = PolyLitTex<4, 4>;
    */

struct Poly {
    ty: PolyType,
    flags: u16,
    verts: [u16; 4],
    norms: [u16; 4],
    tex:     u16,
    tex_cba: u16,
    tex_tsb: u16,
    tex_uvs: [[u8; 2]; 4],
    color: [[u8; 4]; 4],
}

enum Prim {
    Poly(Poly),
}

impl Prim {
    fn grab(cursor: &mut &[u8]) -> Anyhow<Prim> {
        let ty: u16 = grab(cursor)?;
        let flags: u16 = grab(cursor)?;
        let ty = PrimType::try_from(ty)?;

        let prim = match ty {
            PrimType::Poly(ty) => {
                let mut verts = [0xffff_u16; 4];
                let src = grab_n(ty.n, cursor)?;
                verts[..ty.n].copy_from_slice(src);

                let mut norms = [0xffff_u16; 4];
                if ty.lit {
                    let n = if ty.smooth {ty.n} else {1};
                    let src = grab_n(n, cursor)?;
                    norms[..n].copy_from_slice(src);
                }

                let mut tex = 0xffff;
                let mut tex_cba = 0xffff;
                let mut tex_tsb = 0xffff;
                let mut tex_uvs = [[0xff; 2]; 4];
                if ty.tex {
                    tex = grab(cursor)?;
                    tex_cba = grab(cursor)?;
                    tex_tsb = grab(cursor)?;
                    let uvs = grab_n(ty.n, cursor)?;
                    tex_uvs[..ty.n].copy_from_slice(uvs);
                }

                let mut color = [[0xff, 0x00, 0xff, 0xff]; 4];
                let color_n = if ty.smooth {ty.n} else {1};
                let src = if let Ok(src) = grab_n(ty.n, cursor) {
                    src
                }
                else {
                    let _ = cursor.take(..2);
                    grab_n(ty.n, cursor)?
                };
                color[..color_n].copy_from_slice(src);

                Prim::Poly(Poly{ty, flags, verts, norms, tex, tex_cba, tex_tsb, tex_uvs, color})
            }

            PrimType::Lines => todo!(),
            PrimType::Tspr => todo!(),
            PrimType::Bspr => todo!(),
            PrimType::Spline => todo!(),
            PrimType::DirLight => todo!(),
            PrimType::PointLight => todo!(),
            PrimType::SpotLight => todo!(),
        };

        Ok(prim)
    }
}

