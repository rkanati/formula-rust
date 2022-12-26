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

    pub fn lookup(&self, index: usize, offset: uv::Vec2) -> uv::Vec2 {
        (self.uivis[index].xy() + offset) * self.scales.xy()
    }

    pub fn lookup_rect(&self, index: usize) -> uv::Vec4 {
        self.uivis[index] * self.scales
    }

    pub fn make(label: &str, images: &[RgbaImage]) -> Anyhow<(bundle::Image, Atlas)> {
        // allocate atlas regions for textures
        let mut packer = RectPacker::new([1024; 2]);
        let rects = images.iter()
            .map(|image| {
                const APRON: i32 = 1 << N_REDUCTIONS;
                let w = APRON * 2 + image.width() as i32;
                let h = APRON * 2 + image.height() as i32;
                packer.pack_now([w, h]).unwrap()
            })
            .collect::<Vec<_>>();

        // make atlas no larger than necessary
        let [wide, high] = rects.iter()
            .map(|rect| rect.maxs())
            .reduce(|a, b| a.zip(b).map(|(a, b)| a.max(b)))
            .unwrap()
            .map(|x| x as u32);
        let mut atlas = RgbaImage::from_fn(
            wide, high,
            |x, y| if ((x >> N_REDUCTIONS) + (y >> N_REDUCTIONS)) & 1 == 0 {
                Rgba([255,0,255,255])
            }
            else {
                Rgba([0,255,255,255])
            }
        );

        // blit texture images into atlas
        let uivis = rects.iter().enumerate()
            .map(|(image_i, &dst_rect)| {
                let src = &images[image_i as usize];
                let src_rect = blit_with_apron(&mut atlas, dst_rect, &src);
                let rect = uv::IVec4::from(src_rect.corners());
                rect.into()
            })
            .collect::<Vec<_>>();

        atlas.save(&format!("debug-out/atlas-{label}.png"))?;

        let levels = generate_levels(atlas);

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
    let src_offset = (dbg!(dst_dims) - dbg!(src_dims)) / 2;

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

fn generate_levels(mut image: RgbaImage) -> Vec<Vec<u8>> {
    let mut levels = vec![image.to_vec()];
    for _ in 0..N_REDUCTIONS {
        image = image::imageops::resize(
            &image,
            image.width() / 2, image.height() / 2,
            image::imageops::FilterType::Triangle
        );
        levels.push(image.to_vec());
    }
    levels
}

