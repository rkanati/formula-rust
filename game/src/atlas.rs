use {
    crate::gl::prelude::*,
    bundle::qoit,
    pack_rects::prelude::*,
    ultraviolet as uv,
    anyhow::Result as Anyhow,
    pixmap::{Pixmap, Rgba},
};

pub struct Atlas {
    tex: GLuint,
    scales: uv::Vec4,
    uivis: Vec<uv::IVec4>,
}

const N_REDUCTIONS: usize = 4;
const ALIGN: i32 = 1 << N_REDUCTIONS;

impl Atlas {
    pub fn build(gl: &Gl, iset: &bundle::ArchivedImageSet, label: &str) -> Anyhow<Atlas> {
        let mut pixmaps = {
            let mut qoi_state = qoit::State::new();
            let mut input = &iset.qoi_stream[..];
            log::debug!(target: "atlas", "decoding {} textures", iset.sizes.len());
            let pixmaps = iset.sizes.iter().copied().enumerate()
                .map(|(i, (w, h))| {
                    log::debug!(target: "atlas", "texture {i} size: {w} {h}");
                    let mut pixels = Vec::new();
                    pixels.resize(w as usize * h as usize, Rgba::TRANSPARENT);
                    input = qoi_state.decode_some(bytemuck::cast_slice_mut(&mut pixels), input)?;
                    //log::debug!(target: "atlas", "next 16 bytes: {:x?}", &input[..16.min(input.len())]);
                    let pm = Pixmap::new_from_pixels(pixels, 0, 1, w as i32, h as i32).unwrap();
                    Ok(pm)
                })
                .collect::<Anyhow<Vec<_>>>()?;
            //input.strip_prefix(&[0,0,0,0,0,0,0,1]).unwrap();
            pixmaps
        };

        for (i, pm) in pixmaps.iter().enumerate() {
            image::save_buffer(
                format!("debug-out/iset-{label}-{i}.tga"),
                bytemuck::cast_slice(pm.try_as_slice().unwrap()),
                pm.wide() as u32,
                pm.high() as u32,
                image::ColorType::Rgba8,
            ).unwrap();
        }

        let mut rects = iset.sizes.iter().copied()
            .map(|(w, h)| {
                let w = w as i32 + ALIGN * 2;
                let h = h as i32 + ALIGN * 2;
                [w, h]
            })
            .enumerate()
            .collect::<Vec<_>>();

        let white_image = Pixmap::new(1, 1, Rgba::WHITE);
        pixmaps.push(white_image);
        let white_i = rects.len();
        rects.push((white_i, [ALIGN * 2; 2]));

        rects.sort_unstable_by_key(|&(_, [w, h])|
            std::cmp::Reverse((w.max(h) << 16) | w.min(h))
        );

        let mut packer = RectPacker::new([1536; 2]);
        let mut allocs = rects.into_iter()
            .map(|(i, dims)| {
                let rect = packer
                    .pack_now(dims).unwrap()
                    .corners().into();
                (i, rect)
            })
            .collect::<Vec<(usize, uv::IVec4)>>();
        allocs.sort_unstable_by_key(|&(i, _)| i);

        let dims = allocs.iter().copied()
            .map(|(_, rect)| uv::IVec2::new(rect.z, rect.w))
            .reduce(uv::IVec2::max_by_component)
            .unwrap()
            .map(|x| x.next_multiple_of(ALIGN));

        let scales = {
            let au = 1. / dims.x as f32;
            let av = 1. / dims.y as f32;
            uv::Vec4::new(au, av, au, av)
        };

        let mut image = checkerboard(
            dims.x, dims.y,
            N_REDUCTIONS as i32,
            [255,0,255,255].into(),
            [0,255,255,255].into()
        );

        let uivis = allocs.into_iter()
            .map(|(i, alloc)| {
                let src = &pixmaps[i];
                let [xr, yr] = blit_with_apron(
                    &mut image.slice_mut(alloc).expect("bad slice"),
                    &src.borrow(),
                );
                let [x0, y0] = [alloc.x + xr, alloc.y + yr];
                let [x1, y1] = [x0 + src.wide(), y0 + src.high()];
                [x0, y0, x1, y1].into()
            })
            .collect::<Vec<_>>();

        /*image::save_buffer(
            "debug-out/road-atlas.tga",
            bytemuck::cast_slice(image.try_as_slice().unwrap()),
            image.wide() as u32,
            image.high() as u32,
            image::ColorType::Rgba8,
        ).unwrap();*/

        let tex = crate::render::make_texture(gl, &image.borrow());

        Ok(Atlas{tex, scales, uivis})
    }

    pub fn into_texture(self) -> GLuint {
        self.tex
    }

    pub fn lookup_rect(&self, index: usize) -> uv::Vec4 {
        let index = index.min(self.uivis.len() - 1);
        uv::Vec4::from(self.uivis[index]) * self.scales
    }

    pub fn lookup(&self, index: usize, offset: [u8; 2]) -> uv::Vec2 {
        let index = index.min(self.uivis.len() - 1);
        let base = self.uivis[index].xy();
        let offset = uv::IVec2::from(offset.map(|x| x as i32));
        uv::Vec2::from(base + offset) * self.scales.xy()
    }
}

fn checkerboard(w: i32, h: i32, log2_pitch: i32, a: Rgba, b: Rgba) -> Pixmap<Vec<Rgba>> {
    Pixmap::new_from_fn(
        w, h, 
        |[x, y]| {
            let q = ((x >> log2_pitch) + (y >> log2_pitch)) & 1 == 0;
            if q {a} else {b}
        }
    )
}

fn blit_with_apron(dst: &mut Pixmap<&mut [Rgba]>, src: &Pixmap<&[Rgba]>) -> [i32; 2] {
    let rx = (dst.wide() - src.wide()) / 2;
    let ry = (dst.high() - src.high()) / 2;

    for (dx, dy) in util::row_major(0..dst.wide(), 0..dst.high()) {
        let sx = (dx - rx).clamp(0, src.wide()-1);
        let sy = (dy - ry).clamp(0, src.high()-1);
        dst.put([dx, dy], src.get([sx, sy]).unwrap());
    }

    [rx, ry]
}

