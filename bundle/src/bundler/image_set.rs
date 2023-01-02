use {
    crate::be::*,
    anyhow::Result as Anyhow,
    bytemuck as bm,
    pixmap::{Pixmap, Rgba},
    rapid_qoi::Qoi,
};

pub fn build(label: &str, cmp: &[u8], ttf: Option<&[u8]>) -> Anyhow<crate::ImageSet> {
    let images = formats::load_cmp(&cmp)?;

    let images = if let Some(ttf) = ttf {
        let frag_dim = images[0].wide();
        bm::cast_slice(ttf)
            .iter()
            .map(|fm: &FragMap| {
                let mut image = Pixmap::new(frag_dim*4, frag_dim*4, Rgba::TRANSPARENT);
                for (fi, fj) in util::row_major(0..4, 0..4) {
                    let fx = fi * frag_dim;
                    let fy = fj * frag_dim;
                    let frag = fm.hi[fj as usize][fi as usize].get() as usize;
                    let src = &images[frag];
                    image.copy_from([fx as i32, fy as i32], &src);
                }
                Ok(image)
            })
            .collect::<Anyhow<Vec<_>>>()?
    }
    else {
        images
    };

    for (i, image) in images.iter().enumerate() {
        image::save_buffer(
            format!("debug-out/iset-{label}-{i}.tga"),
            bytemuck::cast_slice(image.try_as_slice().unwrap()),
            image.wide() as u32,
            image.high() as u32,
            image::ColorType::Rgba8,
        ).unwrap();
    }

    let sizes = images.iter()
        .map(|img| (img.wide() as u16, img.high() as u16))
        .collect();

    let mut qoi_stream = Vec::with_capacity(0x10_0000);
    let (s0, s1, s2) = &mut ([[0u8; 4]; 64], [0u8, 0u8, 0u8, 0xff_u8], 0_usize);
    for image in images {
        let qoi = Qoi{
            width:  image.wide() as u32,
            height: image.high() as u32,
            colors: rapid_qoi::Colors::Rgba,
        };
        let needed = qoi.encoded_size_limit();
        let old_len = qoi_stream.len();
        qoi_stream.resize(old_len + needed, 0u8);
        let encode_buf = &mut qoi_stream[old_len..];
        let encoded_len = Qoi::encode_range(
            s0, s1, s2,
            bm::cast_slice(image.try_as_slice().unwrap()),
            encode_buf,
        ).unwrap();
        qoi_stream.resize(old_len + encoded_len, 0);
        qoi_stream.extend_from_slice(&[0,0,0,0,0,0,0,1]);
    }

    Ok(crate::ImageSet{sizes, qoi_stream})
}

#[derive(Clone, Copy, bm::AnyBitPattern)]
struct FragMap {
    hi:  [[Be<u16>; 4]; 4],
    _md: [[Be<u16>; 2]; 2],
    _lo: [[Be<u16>; 1]; 1],
}

