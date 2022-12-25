
#![feature(array_chunks)]
#![feature(array_zip)]
#![feature(iter_array_chunks)]

pub mod lzss;

use {
    anyhow::{Result as Anyhow, anyhow, bail},
    bytemuck::pod_read_unaligned as read,
    image::RgbaImage,
};

// TODO positioning

pub fn load_tim(tim: &[u8]) -> Anyhow<RgbaImage> {
    if tim[0] != 0x10 || tim[1] != 0x00 {bail!("not a tim")}
    let pixel_type = tim[4] & 3;
    let got_clut = tim[4] & 8 != 0;
    if got_clut != (pixel_type < 2) {bail!("inconsistent pixel type and clut presence flag")}
    if got_clut {from_indexed(tim, pixel_type)}
    else        {from_direct(tim, pixel_type)}
}

fn from_indexed(tim: &[u8], pixel_type: u8) -> Anyhow<RgbaImage> {
    let bs = &tim[8..];

    debug_assert!(pixel_type < 2);
    let four_bit = pixel_type == 0;

    let (clut, bs) = {
        let clut_len_bs = bs.get(0..4).ok_or(anyhow!("truncated tim"))?;
        let clut_len = read::<u32>(clut_len_bs) as usize;
        if bs.len() < clut_len {bail!("truncated tim")}
        bs.split_at(clut_len)
    };

    let clut = {
        let clut = clut.get(12..).ok_or(anyhow!("truncated tim"))?;
        let pal_n = if four_bit {16} else {256};
        if clut.len() != pal_n * 2 {bail!("wrong sized clut")}
        clut.array_chunks().copied()
            .map(u16::from_le_bytes)
            .map(rgb15x1_to_rgba8)
            .collect::<Vec<_>>()
    };

    if bs.len() < 12 {bail!("truncated tim")}
    let (header, bs) = bs.split_at(12);
    let pixels_len = read::<u32>(&header[0..4]) as usize - 12;
    let wide = read::<u16>(&header[ 8..10]) as u32;
    if wide == 0 {std::fs::write("dump", tim);}
    let high = read::<u16>(&header[10..12]) as u32;
    let pixels = bs.get(..pixels_len).ok_or(anyhow!("truncated tim"))?;

    let (wide, pixels) = if four_bit {
        let pixels = pixels.iter().copied()
            .flat_map(|b| [(b & 0xf) as usize, (b >> 4) as usize])
            .flat_map(|i| clut[i])
            .collect();
        (wide * 4, pixels)
    }
    else {
        let pixels = pixels.iter().copied()
            .flat_map(|i| clut[i as usize])
            .collect();
        (wide * 2, pixels)
    };

    let image = RgbaImage::from_vec(wide, high, pixels).unwrap();
    Ok(image)
}

fn from_direct(_bs: &[u8], pixel_type: u8) -> Anyhow<RgbaImage> {
    debug_assert!(pixel_type >= 2);
    todo!("direct-colour tims")
}

fn rgb15x1_to_rgba8(bits: u16) -> [u8; 4] {
    if (bits >> 15) & 0x01 != 0 {
        return [0; 4];
    }

    let ramp = |y: f32| y;

    let chan = |i: u16| {
        let y0 = ((bits >> i) & 0x1f) as f32 / 31.;
        let y1 = ramp(y0);
        (y1 * 255.) as u8
    };

    let [r, g, b] = [0, 5, 10].map(chan);
    [r, g, b, 255]
}

pub fn load_cmp(cmp: &[u8]) -> Anyhow<Vec<RgbaImage>> {
    let n_tims: u32 = read(&cmp[0..4]);
    let (lens, data) = cmp[4..].split_at(n_tims as usize * 4);
    let mut tims = lzss::expand(data);
    lens.array_chunks()
        .map(|&len| {
            let len = u32::from_le_bytes(len) as usize;
            if len == 0 {return Ok(RgbaImage::from_pixel(1, 1, [0; 4].into()))}
            let rest = tims.split_off(len);
            let tim = std::mem::replace(&mut tims, rest);
            load_tim(&tim)
        })
        .collect::<Result<Vec<_>, _>>()
}

