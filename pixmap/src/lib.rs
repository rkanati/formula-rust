#![feature(int_roundings)]

use bytemuck as bm;

#[derive(Clone, Copy, PartialEq, Eq, Hash, bm::Pod, bm::Zeroable)]
#[repr(C, align(4))]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba([0x00, 0x00, 0x00, 0x00]);
    pub const BLACK:       Rgba = Rgba([0x00, 0x00, 0x00, 0xff]);
    pub const WHITE:       Rgba = Rgba([0xff, 0xff, 0xff, 0xff]);

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        Rgba([r, g, b, a])
    }
}

impl From<[u8; 4]> for Rgba {
    fn from(rgba: [u8; 4]) -> Self { Rgba(rgba) }
}

mod meta {
    #[derive(Debug, Clone, Copy)]
    pub struct Meta {
        offset: usize,
        pitch:  usize,
        wide:   usize,
        high:   usize,
    }

    impl Meta {
        pub fn try_new(offset: i32, pitch: i32, wide: i32, high: i32) -> Option<Meta> {
            let offset = usize::try_from(offset).ok()?;
            let pitch  = usize::try_from(pitch).ok()?;
            let wide   = usize::try_from(wide).ok()?;
            let high   = usize::try_from(high).ok()?;
            let pitch  = wide.next_multiple_of(pitch);
            Some(Meta{offset, pitch, wide, high})
        }

        pub fn index(&self, [x, y]: [i32; 2]) -> Option<usize> {
            let x: usize = x.try_into().ok()?;
            let y: usize = y.try_into().ok()?;
            if x >= self.wide || y >= self.high {return None}
            Some(self.offset + self.pitch * y + x)
        }

        pub fn slice(&self, rect: [i32; 4]) -> Option<Self> {
            let [x0, y0, x1, y1] = rect;
            let _ = self.index([x1-1, y1-1]).expect("a");
            let offset = self.index([x0, y0]).expect("b");
            let pitch = self.pitch;
            let wide = usize::try_from(x1 - x0).ok().expect("c");
            let high = usize::try_from(y1 - y0).ok().expect("d");
            Some(Meta{offset, pitch, wide, high})
        }

        pub fn validate(&self, len: usize) -> Option<()> {
            let x = i32::try_from(self.wide-1).ok()?;
            let y = i32::try_from(self.high-1).ok()?;
            let i = self.index([x, y])?;
            (i < len).then_some(())
        }

        pub fn is_contiguous(&self) -> bool {
            self.wide == self.pitch
        }

        pub fn wide(&self) -> usize { self.wide }
        pub fn high(&self) -> usize { self.high }
    }
}

use meta::Meta;

pub struct Pixmap<Pixels> {
    pixels: Pixels,
    meta: Meta,
}

impl<Pixels> Pixmap<Pixels> {
    pub fn wide(&self) -> i32 { self.meta.wide().try_into().unwrap() }
    pub fn high(&self) -> i32 { self.meta.high().try_into().unwrap() }
}

impl Pixmap<Vec<Rgba>> {
    pub fn new(wide: i32, high: i32, fill: Rgba) -> Self {
        let meta = Meta::try_new(0, 1, wide, high).unwrap();

        let pixels = {
            let len = meta.wide() * meta.high();
            let mut pixels = Vec::new();
            pixels.resize(len, fill);
            pixels
        };

        Self{pixels: pixels.into(), meta}
    }

    pub fn new_from_fn(wide: i32, high: i32, mut f: impl FnMut([i32; 2]) -> Rgba) -> Self {
        let mut pm = Self::new(wide, high, Rgba::TRANSPARENT);
        for (x, y) in iter_2d(0..wide, 0..high) {
            let xy = [x, y];
            pm.put(xy, f(xy));
        }
        pm
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsRef<[Rgba]> {
    pub fn new_from_pixels(pixels: Pixels, offset: i32, pitch: i32, wide: i32, high: i32)
        -> Option<Self>
    {
        let meta = Meta::try_new(offset, pitch, wide, high)?;
        Some(Self{pixels, meta})
    }

    pub fn borrow(&self) -> Pixmap<&[Rgba]> {
        Pixmap {
            pixels: self.pixels.as_ref(),
            meta: self.meta,
        }
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsRef<[Rgba]> {
    pub fn slice(&self, rect: impl Into<[i32; 4]>) -> Option<Pixmap<&[Rgba]>> {
        let meta = self.meta.slice(rect.into())?;
        let pixels = self.pixels.as_ref();
        Pixmap{pixels, meta}.validate()
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsMut<[Rgba]> + AsRef<[Rgba]> {
    pub fn slice_mut(&mut self, rect: impl Into<[i32; 4]>) -> Option<Pixmap<&mut [Rgba]>> {
        let meta = self.meta.slice(rect.into())?;
        let pixels = self.pixels.as_mut();
        Pixmap{pixels, meta}.validate()
    }
}

impl<'a> Pixmap<&'a [Rgba]> {
    pub fn new_from_u32s(pixels: &'a [u32], offset: i32, pitch: i32, wide: i32, high: i32)
        -> Option<Self>
    {
        let rgbas = bytemuck::cast_slice(pixels);
        Self::new_from_pixels(rgbas, offset, pitch, wide, high)
    }
}

impl<Pixels> Pixmap<Pixels> {
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsRef<[Rgba]> {
    fn pixels(&self) -> &[Rgba] {
        self.pixels.as_ref()
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsMut<[Rgba]> + AsRef<[Rgba]> {
    fn pixels_mut(&mut self) -> &mut [Rgba] {
        self.pixels.as_mut()
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsRef<[Rgba]> {
    fn validate(self) -> Option<Self> {
        let len = self.pixels().len();
        self.meta.validate(len)?;
        Some(self)
    }

    pub fn get(&self, at: impl Into<[i32; 2]>) -> Option<Rgba> {
        let index = self.meta.index(at.into())?;
        Some(self.pixels()[index])
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsMut<[Rgba]> + AsRef<[Rgba]> {
    pub fn put(&mut self, at: impl Into<[i32; 2]>, p: impl Into<Rgba>) {
        let index = self.meta.index(at.into()).unwrap();
        self.pixels_mut()[index] = p.into();
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsRef<[Rgba]> {
    pub fn try_as_slice(&self) -> Option<&[Rgba]> {
        self.meta.is_contiguous().then(|| self.pixels())
    }
}

impl<Pixels> Pixmap<Pixels> where Pixels: AsMut<[Rgba]> + AsRef<[Rgba]> {
    pub fn copy_from<Others> (&mut self, at: impl Into<[i32; 2]>, src: &Pixmap<Others>) where
        Others: AsRef<[Rgba]>,
    {
        let [dx0, dy0] = at.into();
        for (sx, sy) in iter_2d(0..src.wide(), 0..src.high()) {
            let dx = dx0 + sx;
            let dy = dy0 + sy;
            self.put([dx, dy], src.get([sx, sy]).unwrap());
        }
    }
}

pub fn iter_2d<Xs, Ys> (xs: Xs, ys: Ys)
    -> impl Iterator<Item = (Xs::Item, Ys::Item)>
where
    Xs: Iterator + Clone,
    Xs::Item: 'static,
    Ys: Iterator,
    Ys::Item: Clone + 'static,
{
    ys.flat_map(move |y| xs.clone().map(move |x| (x, y.clone())))
}

