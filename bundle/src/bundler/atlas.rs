use {
    crate::bundle,
    anyhow::Result as Anyhow,
    image::{GenericImage, RgbaImage, Rgba},
    pack_rects::prelude::*,
    ultraviolet as uv,
    util::row_major,
};

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

    pub fn make(images: &[RgbaImage]) -> Anyhow<(bundle::Image, Atlas)> {
        // TODO generate mipmaps; padding etc

        // allocate atlas regions for textures
        let mut packer = RectPacker::new([1024; 2]);
        let rects = images.iter()
            .map(|image| {
                let w = image.width() as i32;
                let h = image.height() as i32;
                let rect = packer.pack_now([w, h]).unwrap();
                rect
            })
            .collect::<Vec<_>>();

        // make atlas no larger than necessary
        let [wide, high] = rects.iter()
            .map(|rect| rect.maxs())
            .reduce(|a, b| a.zip(b).map(|(a, b)| a.max(b)))
            .unwrap()
            .map(|x| x as u32);

        // uv-coordinate scales based on atlas size
        //let uv_scale = uv::Vec4::from([wide, high, wide, high].map(|x| 1. / x as f32));
        let mut atlas = RgbaImage::from_pixel(wide, high, Rgba([255,0,255,255]));

        // blit texture images into atlas
        let uivis = rects.iter().enumerate()
            .map(|(image_i, &rect)| {
                let [x, y] = rect.mins().map(|x| x as u32);
                let src = &images[image_i as usize];
                atlas.copy_from(src, x, y);

                let rect = uv::IVec4::from(rect.corners());
                rect.into()//(uv::Vec4::from(rect) * uv_scale).into()
            })
            .collect::<Vec<_>>();

        let image = bundle::Image {
            wide: wide as u16,
            high: high as u16,
            levels: vec![atlas.into_raw()],
        };

        let scales = {
            let au = 1. / image.wide as f32;
            let av = 1. / image.high as f32;
            uv::Vec4::new(au, av, au, av)
        };

        Ok((image, Atlas{scales, uivis}))
    }

    pub fn make_for_road(fragments: &[RgbaImage], mip_mappings: &[MipMapping])
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

        Self::make(&images)
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

