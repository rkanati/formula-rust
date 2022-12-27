use {
    crate::bundle,
    anyhow::Result as Anyhow,
    image::{GenericImage, RgbaImage, Rgba},
    pack_rects::prelude::*,
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

    pub fn make(label: &str, images: &[RgbaImage]) -> Anyhow<(bundle::Image, Atlas)> {
        const ALIGN: i32 = 1 << N_REDUCTIONS;

        // allocate atlas regions for textures
        let mut packer = RectPacker::new([1024; 2]);

        let rects = images.iter()
            .map(|image| {
                let w = ALIGN * 2 + image.width() as i32;
                let h = ALIGN * 2 + image.height() as i32;
                packer.pack_now([w, h]).unwrap()
            })
            .collect::<Vec<_>>();

        // reserve a white area for non-texturing
        let white_area = packer.pack_now([ALIGN*2; 2]).unwrap();

        // make atlas no larger than necessary
        let [wide, high] = rects.iter()
            .map(|rect| rect.maxs())
            .reduce(|a, b| a.zip(b).map(|(a, b)| a.max(b)))
            .unwrap()
            .map(|x| x.next_multiple_of(ALIGN) as u32);
        let wide = wide.max(white_area.max_x() as u32);
        let high = high.max(white_area.max_y() as u32);

        let mut atlas = checkerboard(
            wide, high, N_REDUCTIONS as u32, Rgba([255,0,255,255]), Rgba([0,255,255,255])
        );

        // fill white area
        row_major(
                white_area.min_x() .. white_area.max_x(),
                white_area.min_y() .. white_area.max_y()
            )
            .for_each(|(x, y)| {
                atlas.put_pixel(x as u32, y as u32, [255;4].into());
            });

        // blit texture images into atlas
        let mut uivis = rects.iter().enumerate()
            .map(|(image_i, &dst_rect)| {
                let src = &images[image_i as usize];
                let src_rect = blit_with_apron(&mut atlas, dst_rect, &src);
                let rect = uv::IVec4::from(src_rect.corners());
                rect.into()
            })
            .collect::<Vec<_>>();

        uivis.push({
            let uv::IVec2{x, y}
                = uv::IVec2::from(white_area.mins())
                + uv::IVec2::from(white_area.dims()) / 2;
            uv::IVec4::from([x-1, y-1, x, y]).into()
        });

        let levels = generate_levels(label, atlas);

        let image = bundle::Image {
            wide: wide as u16,
            high: high as u16,
            levels,
        };

        let scales = {
            let au = 1. / image.wide as f32;
            let av = 1. / image.high as f32;
            uv::Vec4::new(au, av, au, av)
        };

        Ok((image, Atlas{scales, uivis}))
    }

    pub fn make_for_road(label: &str, fragments: &[RgbaImage], mip_mappings: &[MipMapping])
        -> Anyhow<(bundle::Image, Atlas)>
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

fn reduce(image: &RgbaImage) -> RgbaImage {
    assert!(image.width()  & 1 == 0, "image dims: {:?}", image.dimensions());
    assert!(image.height() & 1 == 0, "image dims: {:?}", image.dimensions());

    let rw = image.width()  / 2;
    let rh = image.height() / 2;

    let pixels: &[[[u8; 4]; 2]] = bytemuck::cast_slice(image.as_ref());
    let pixels = pixels.chunks(rw as usize)
        .array_chunks()
        .flat_map(|[even_row, odd_row]| {
            even_row.into_iter().zip(odd_row.into_iter())
                .map(|([a, b], [c, d])| {
                    [a, b, c, d].into_iter()
                        .fold(
                            [0u32; 4],
                            |a, &p| a.zip(p).map(|(a, p)| a + p as u32)
                        )
                        .map(|x| (x / 4) as u8)
                })
        })
        .flatten()
        .collect::<Vec<_>>();

    RgbaImage::from_vec(rw, rh, pixels).unwrap()
}

fn generate_levels(label: &str, image: RgbaImage) -> Vec<Vec<u8>> {
    let mut levels = vec![image];
    while levels.len() <= N_REDUCTIONS {
        let image = reduce(levels.last().unwrap());
        levels.push(image);
    }

    for (level_i, level) in levels.iter().enumerate() {
        let _ = level.save(&format!("debug-out/atlas-{label}-{level_i:02}.tga"));
    }

    levels.into_iter()
        .map(|lv| lv.into_vec())
        .collect()
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

