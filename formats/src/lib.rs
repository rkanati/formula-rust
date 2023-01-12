#![feature(array_chunks)]
#![feature(array_zip)]
#![feature(int_roundings)]
#![feature(iter_array_chunks)]

pub mod lzss;

use {
    anyhow::{Result as Anyhow, anyhow, bail},
    bytemuck::pod_read_unaligned as read,
    pixmap::{Pixmap, Rgba},
};

pub type Image = Pixmap<Vec<Rgba>>;

pub fn load_cmp(cmp: &[u8]) -> Anyhow<Vec<Image>> {
    let n_tims: u32 = read(&cmp[0..4]);
    let (lens, data) = cmp[4..].split_at(n_tims as usize * 4);
    let mut tims = lzss::expand(data);
    lens.array_chunks()
        .enumerate()
        .map(|(i, &len)| {
            let len = u32::from_le_bytes(len) as usize;
            assert_ne!(len, 0);
            let rest = tims.split_off(len);
            let tim = std::mem::replace(&mut tims, rest);
            load_tim(&tim)
        })
        .collect()
}

pub fn load_tim(tim: &[u8]) -> Anyhow<Image> {
    if tim[0] != 0x10 || tim[1] != 0x00 {bail!("not a tim")}
    let pixel_type = tim[4] & 3;
    let got_clut = tim[4] & 8 != 0;
    if got_clut != (pixel_type < 2) {bail!("inconsistent pixel type and clut presence flag")}
    if got_clut {from_indexed(tim, pixel_type)}
    else        {from_direct(tim, pixel_type)}
}

fn from_indexed(tim: &[u8], pixel_type: u8) -> Anyhow<Image> {
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
    let data_len = read::<u32>(&header[0..4]) as usize - 12;
    let wide = read::<u16>(&header[ 8..10]);
    let high = read::<u16>(&header[10..12]);
    let data = bs.get(..data_len).ok_or(anyhow!("truncated tim"))?;

    //let wide = wide.next_multiple_of(2);

    let (wide, mut pixels) = if four_bit {
        let pixels = data.iter().copied()//chunks_exact((wide as usize * 2).next_multiple_of(4))
            //.flat_map(|row| &row[..wide as usize * 2])
            .flat_map(|b| [(b & 0xf) as usize, (b >> 4) as usize])
            .map(|i| clut[i])
            .collect::<Vec<_>>();
        (wide * 4, pixels)
    }
    else {
        let pixels = data.iter().copied()//chunks_exact((wide as usize * 2).next_multiple_of(4))
            //.flat_map(|row| &row[..wide as usize * 2])
            .map(|i| clut[i as usize])
            .collect::<Vec<_>>();
        (wide * 2, pixels)
    };

    //debug_assert_eq!(pixels.len(), (wide * high) as usize);
    pixels.resize(wide as usize * high as usize, Rgba::TRANSPARENT);
    let pm = Pixmap::new_from_pixels(pixels, 0, 1, wide.into(), high.into()).unwrap();
    Ok(pm)
}

fn from_direct(_bs: &[u8], pixel_type: u8) -> Anyhow<Image> {
    debug_assert!(pixel_type >= 2);
    todo!("direct-colour tims")
}

fn rgb15x1_to_rgba8(bits: u16) -> Rgba {
    if (bits >> 15) & 0x01 != 0 || (bits & 0x7fff) == 0 {
        return Rgba::TRANSPARENT;
    }

    let ramp = |y: f32| y;

    let chan = |i: u16| {
        let y0 = ((bits >> i) & 0x1f) as f32 / 31.;
        let y1 = ramp(y0);
        (y1 * 255.) as u8
    };

    let [r, g, b] = [0, 5, 10].map(chan);
    [r, g, b, 255].into()
}

