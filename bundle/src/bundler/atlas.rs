use {
    anyhow::Result as Anyhow,
    pack_rects::prelude::*,
    rapid_qoi::Qoi,
    ultraviolet as uv,
    util::row_major,
    pixmap::{Pixmap, Rgba},
    bytemuck as bm,
};

const N_REDUCTIONS: usize = 4;
const ALIGN: i32 = 1 << N_REDUCTIONS;

pub struct Atlas {
    scales: uv::Vec4,
    uivis: Vec<uv::IVec4>,
}

impl Atlas {
    pub fn no_texture(&self) -> uv::Vec4 {
        uv::Vec4::from(*self.uivis.last().unwrap()) * self.scales
    }

    pub fn lookup(&self, index: usize, offset: uv::Vec2) -> uv::Vec2 {
        (uv::Vec2::from(self.uivis[index].xy()) + offset) * self.scales.xy()
    }

    pub fn lookup_rect(&self, index: usize) -> uv::Vec4 {
        uv::Vec4::from(self.uivis[index]) * self.scales
    }

    pub fn build(label: &str, pixmaps: &[Pixmap<Vec<Rgba>>]) -> Anyhow<(crate::Image, Atlas)> {
        let mut rects = pixmaps.iter()
            .map(|pm| {
                let w = pm.wide() + ALIGN * 2;
                let h = pm.high() + ALIGN * 2;
                [w, h]
            })
            .enumerate()
            .collect::<Vec<_>>();

        let white_image = Pixmap::new(1, 1, Rgba::WHITE);
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
                let src = if i == white_i {&white_image} else {&pixmaps[i]};
                let [rx, ry] = blit_with_apron(
                    &mut image.slice_mut(alloc).expect("bad slice"),
                    &src.borrow(),
                );
                let [x0, y0] = [rx + alloc.x, ry + alloc.y];
                let [x1, y1] = [x0 + src.wide(), y0 + src.high()];
                [x0, y0, x1, y1].into()
            })
            .collect::<Vec<_>>();

        let image = generate(label, image);

        Ok((image, Atlas{scales, uivis}))
    }
}

fn generate(label: &str, image: Pixmap<Vec<Rgba>>) -> crate::Image {
    //let base_width  = image.wide();
    //let base_height = image.high();

    //let _ = image.save(&format!("debug-out/atlas-{label}.tga"));//-{level_i:02}.tga"));

    let mut qoi_buffer = {
        let qoi = Qoi {
            width:  image.wide() as u32,
            height: image.high() as u32,
            colors: rapid_qoi::Colors::Rgba
        };
        let mut buf = Vec::new();
        buf.resize(qoi.encoded_size_limit(), 0u8);
        buf
    };

    let len = Qoi::encode_range(
        &mut [[0; 4]; 64], &mut [0, 0, 0, 255], &mut 0,
        bm::cast_slice(&image.try_as_slice().unwrap()),
        &mut qoi_buffer,
    ).unwrap();
    qoi_buffer.shrink_to(len);
    qoi_buffer.extend_from_slice(&[0,0,0,0,0,0,0,1]);
    crate::Image {
        wide: image.wide() as u16,
        high: image.high() as u16,
        coding: crate::ImageCoding::Qoi,
        data: qoi_buffer,
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

