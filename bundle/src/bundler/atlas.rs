use {
    anyhow::Result as Anyhow,
    image::{GenericImage, RgbaImage, Rgba},
    pack_rects::prelude::*,
    rapid_qoi::Qoi,
    ultraviolet as uv,
    util::row_major,
};

const N_REDUCTIONS: usize = 4;

pub struct Atlas {
    scales: uv::Vec4,
    uivis: Vec<uv::Vec4>,
}

impl Atlas {
    pub fn into_uvs(mut self) -> Vec<uv::Vec4> {
        for uv in &mut self.uivis { *uv *= self.scales; }
        self.uivis
    }

    pub fn no_texture(&self) -> uv::Vec4 {
        *self.uivis.last().unwrap() * self.scales
    }

    pub fn lookup(&self, index: usize, offset: uv::Vec2) -> uv::Vec2 {
        (self.uivis[index].xy() + offset) * self.scales.xy()
    }

    pub fn lookup_rect(&self, index: usize) -> uv::Vec4 {
        self.uivis[index] * self.scales
    }

    pub fn make(label: &str, images: &[RgbaImage]) -> Anyhow<(crate::Image, Atlas)> {
        const ALIGN: i32 = 1 << N_REDUCTIONS;

        let mut rects = images.iter().enumerate()
            .map(|(i, image)| {
                let w = ALIGN * 2 + image.width() as i32;
                let h = ALIGN * 2 + image.height() as i32;
                ([w, h], i, image)
            })
            .collect::<Vec<_>>();

        // reserve a white area for non-texturing
        let white_image = RgbaImage::from_pixel(1, 1, Rgba([0xff; 4]));
        let white_i = images.len();
        rects.push(([ALIGN * 2; 2], white_i, &white_image));

        rects.sort_unstable_by_key(|&([w, h], _, _)| std::cmp::Reverse(h.max(w)));

        // allocate atlas regions for textures
        let mut packer = RectPacker::new([1024; 2]);
        let allocs = rects.into_iter()
            .map(|([w, h], i, img)| {
                let rect = packer.pack_now([w, h])?;
                Some((rect, i, img))
            })
            .collect::<Option<Vec<_>>>().ok_or(anyhow::anyhow!("ran out of atlas space"))?;

        // make atlas no larger than necessary
        let [wide, high] = allocs.iter()
            .map(|(rect, _, _)| rect.maxs())
            .reduce(|a, b| a.zip(b).map(|(a, b)| a.max(b)))
            .unwrap()
            .map(|x| x.next_multiple_of(ALIGN) as u32);

        let mut atlas = checkerboard(
            wide, high, N_REDUCTIONS as u32, Rgba([255,0,255,255]), Rgba([0,255,255,255])
        );

        // blit texture images into atlas
        let mut uivis = allocs.into_iter()
            .map(|(dst_rect, i, image)| {
                let src_rect = blit_with_apron(&mut atlas, dst_rect, image);
                let rect = uv::IVec4::from(src_rect.corners());
                (i, rect.into())
            })
            .collect::<Vec<_>>();
        uivis.sort_unstable_by_key(|&(i, _)| i);
        let uivis = uivis.into_iter().map(|(_, uivi)| uivi).collect();

        let data = generate(label, atlas);

        let image = crate::Image {
            wide: wide as u16,
            high: high as u16,
            coding: crate::ImageCoding::Qoi,
            data,
        };

        let scales = {
            let au = 1. / image.wide as f32;
            let av = 1. / image.high as f32;
            uv::Vec4::new(au, av, au, av)
        };

        Ok((image, Atlas{scales, uivis}))
    }

    pub fn make_for_road(label: &str, fragments: &[RgbaImage], mip_mappings: &[MipMapping])
        -> Anyhow<(crate::Image, Atlas)>
    {
        let frag_dims = {
            let (frag_w, frag_h) = fragments[0].dimensions();
            uv::IVec2::new(frag_w as i32, frag_h as i32)
        };

        let image_dims = frag_dims * 4;

        let images = mip_mappings.iter()
            .map(|mm| {
                let mut image = RgbaImage::from_pixel(
                    image_dims.x as u32, image_dims.y as u32, Rgba([255,0,255,255])
                );
                row_major(0..4, 0..4)
                    .map(|(i, j)| uv::IVec2::new(i, j) * frag_dims)
                    .zip(mm.hi)
                    .map(|(pos, image_i)| {
                        let src = &fragments[image_i as usize];
                        image.copy_from(src, pos.x as u32, pos.y as u32)?;
                        Ok(())
                    })
                    .collect::<Anyhow<()>>()?;
                Ok(image)
            })
            .collect::<Anyhow<Vec<_>>>()?;

        Self::make(label, &images)
    }
}

pub struct MipMapping {
    hi: [u16; 16],
  /*md: [u16;  4],
    lo: [u16;  1],*/
}

pub fn parse_ttf(ttf: &[u8]) -> Vec<MipMapping> {
    ttf.array_chunks().copied()
        .map(u16::from_be_bytes)
        .array_chunks()
        .map(|is: [u16; 21]| MipMapping {
            hi: is[0..16].try_into().unwrap(),
          /*md: is[16..20].try_into().unwrap(),
            lo: [is[20]],*/
        })
        .collect()
}

fn blit_with_apron(dst: &mut RgbaImage, dst_rect: Rect, src: &RgbaImage) -> Rect {
    let dst_mins = uv::IVec2::from(dst_rect.mins());
    let dst_dims = uv::IVec2::from(dst_rect.dims());
    let src_dims = uv::IVec2::new(src.width() as i32, src.height() as i32);
    let src_offset = (dst_dims - src_dims) / 2;

    for dst_rel in row_major(0..dst_dims.x, 0..dst_dims.y).map(uv::IVec2::from) {
        let src_abs = (dst_rel - src_offset)
            .clamped(uv::IVec2::one()*0, src_dims - 1*uv::IVec2::one());
        let dst_abs = dst_mins + dst_rel;
        dst.put_pixel(
            dst_abs.x as u32, dst_abs.y as u32,
            *src.get_pixel(src_abs.x as u32, src_abs.y as u32)
        );
    }

    let src_mins = dst_mins + src_offset;
    Rect::with_dims(src_mins.into(), src_dims.into())
}

fn generate(label: &str, image: RgbaImage) -> Vec<u8> {
    let base_width = image.width();
    let base_height = image.height();

    let _ = image.save(&format!("debug-out/atlas-{label}.tga"));//-{level_i:02}.tga"));

    let mut qoi_buffer = {
        let qoi = Qoi {
            width: base_width,
            height: base_height,
            colors: rapid_qoi::Colors::Rgba
        };
        let mut buf = Vec::new();
        buf.resize(qoi.encoded_size_limit(), 0u8);
        buf
    };

    let (s0, s1, s2) = &mut ([[0u8; 4]; 64], [0u8, 0u8, 0u8, 0xff_u8], 0_usize);
    let len = Qoi::encode_range(s0, s1, s2, &image, &mut qoi_buffer[..]).unwrap();
    qoi_buffer[len..][..8].copy_from_slice(&[0,0,0,0,0,0,0,1]);
    qoi_buffer[..len+8].into()
}

fn checkerboard(w: u32, h: u32, log2_pitch: u32, a: Rgba<u8>, b: Rgba<u8>) -> RgbaImage {
    RgbaImage::from_fn(
        w, h, 
        |x, y| {
            let q = ((x >> log2_pitch) + (y >> log2_pitch)) & 1 == 0;
            if q {a} else {b}
        }
    )
}

