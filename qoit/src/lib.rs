#![feature(slice_take)]
#![feature(extend_one)]

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Input underrun")]
    Underrun,
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("Output buffer overrun")]
    Overrun,
}

pub type Pixel = [u8; 4];

fn hash_pixel([r, g, b, a]: Pixel) -> u8 { 
    let r = r.wrapping_mul(3);
    let g = g.wrapping_mul(5);
    let b = b.wrapping_mul(7);
    let a = a.wrapping_mul(11);
    0x3f & r.wrapping_add(g).wrapping_add(b).wrapping_add(a)
}

pub struct State {
    prev: Pixel,
    array: [Pixel; 64],
    run: usize,
}

impl State {
    pub fn new() -> Self {
        State {
            prev: [0, 0, 0, 255],
            array: [[0; 4]; 64],
            run: 0,
        }
    }
}

impl State {
    pub fn decode_some<'s, 'p, 'b>(&'s mut self, output: &'p mut [Pixel], bytes: &'b [u8])
        -> Result<&'b [u8], DecodeError>
    where
        'p: 's,
        'b: 's,
    {
        let cursor_in = &mut &bytes[..];
        let cursor_out = &mut &mut output[..];

        while !cursor_out.is_empty() {
            if self.run != 0 {
                let n = self.run.min(cursor_out.len());
                let pixels_out;
                (pixels_out, *cursor_out) = cursor_out.split_at_mut(n);
                pixels_out.fill(self.prev);
                self.run -= n;
                continue;
            }

            let pixel;
            (pixel, *cursor_in) = match *cursor_in {
                &[0xff, r, g, b, a, ref rest@..] => ([r, g, b, a], rest),
                &[0xfe, r, g, b, ref rest@..] => ([r, g, b, self.prev[3]], rest),
                &[b0@0x00..=0x3f, ref rest@..] => (self.array[(b0 & 0x3f) as usize], rest),

                // diff
                &[b0@0x40..=0x7f, ref rest@..] => {
                    let c = |i: usize| (((b0 >> 4-i*2 & 0x3) as i8 - 2) as u8)
                        .wrapping_add(self.prev[i]);
                    ([c(0), c(1), c(2), self.prev[3]], rest)
                }

                // luma
                &[b0@0x80..=0xbf, b1, ref rest@..] => {
                    let dg = (b0 & 0x3f) as i8 - 32;
                    let dr = dg + (b1 >> 4 & 0xf) as i8 - 8;
                    let db = dg + (b1 >> 0 & 0xf) as i8 - 8;
                    let pixel = [
                        self.prev[0].wrapping_add(dr as u8),
                        self.prev[1].wrapping_add(dg as u8),
                        self.prev[2].wrapping_add(db as u8),
                        self.prev[3],
                    ];
                    (pixel, rest)
                }

                // run
                &[b0@0xc0..=0xfd, ref rest@..] => {
                    debug_assert_eq!(self.run, 0);
                    self.run = (b0 & 0x3f) as usize + 1;
                    *cursor_in = rest;
                    continue;
                }

                _ => return Err(DecodeError::Underrun),
            };

            let pixel_out;
            (pixel_out, *cursor_out) = cursor_out.split_first_mut().unwrap();
            *pixel_out = pixel;
            self.prev = pixel;
            let hash = hash_pixel(pixel);
            self.array[hash as usize] = pixel;
        }

        Ok(*cursor_in)
    }

    pub fn encode_flush<'s, 'b, Output>(&'s mut self, output: &mut Output)
        -> Result<(), EncodeError>
    where
        'b: 's,
        Output: Extend<u8> + 'b,
    {
        if self.run != 0 {
            debug_assert!(self.run <= 62);
            output.extend_one(0xc0 | self.run as u8 - 1);
        }
        self.run = 0;

        Ok(())
    }

    pub fn encode_some<'s, 'b, 'p, Output>(&'s mut self, output: &mut Output, pixels: &'p [Pixel])
        -> Result<(), EncodeError>
    where
        'b: 's,
        'p: 's,
        Output: Extend<u8> + 'b,
    {
        for &pixel in pixels {
            let hash = hash_pixel(pixel);

            if pixel == self.prev {
                self.run += 1;
                if self.run == 62 {
                    self.encode_flush(output)?;
                }
            }
            else {
                self.encode_flush(output)?;

                let index_pixel = self.array[hash as usize];

                if pixel == index_pixel {
                    output.extend_one(hash);
                }
                else if pixel[3] != self.prev[3] {
                    output.extend([0xff, pixel[0], pixel[1], pixel[2], pixel[3]]);
                }
                else {
                    let dr = pixel[0].wrapping_sub(self.prev[0]);
                    let dg = pixel[1].wrapping_sub(self.prev[1]);
                    let db = pixel[2].wrapping_sub(self.prev[2]);

                    let dr2 = dr.wrapping_add(2);
                    let dg2 = dg.wrapping_add(2);
                    let db2 = db.wrapping_add(2);

                    let dg32 = dg.wrapping_add(32);
                    let drdg8 = dr.wrapping_sub(dg).wrapping_add(8);
                    let dbdg8 = db.wrapping_sub(dg).wrapping_add(8);

                    if dr2 < 4 && dg2 < 4 && db2 < 4 {
                        output.extend([0x40 | dr2 << 4 | dg2 << 2 | db2]);
                    }
                    else if dg32 < 64 && drdg8 < 16 && dbdg8 < 16 {
                        output.extend([0x80 | dg32, drdg8 << 4 | dbdg8]);
                    }
                    else {
                        output.extend([0xfe, pixel[0], pixel[1], pixel[2]]);
                    }
                }

                self.prev = pixel;
            }

            self.array[hash as usize] = pixel;
        }

        Ok(())
    }
}

/*trait Put {
    fn put(&mut self, byte: u8) -> Result<(), EncodeError>;
    fn put_many<const N: usize>(&mut self, bytes: [u8; N]) -> Result<(), EncodeError>;
}

impl Put for &mut [u8] {
    fn put(&mut self, byte: u8) -> Result<(), EncodeError> {
        *self.take_first_mut().ok_or(EncodeError::Overrun)? = byte;
        Ok(())
    }

    fn put_many<const N: usize>(&mut self, bytes: [u8; N]) -> Result<(), EncodeError> {
        let out = self.take_mut(..bytes.len()).ok_or(EncodeError::Overrun)?;
        out.copy_from_slice(&bytes);
        Ok(())
    }
}*/

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ColorSpec {
    Srgb8   = 0x0003,
    Srgb8A8 = 0x0004,
    Rgb8    = 0x0103,
    Rgba8   = 0x0104,
}

#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub wide: u32,
    pub high: u32,
    pub spec: ColorSpec,
}

#[derive(Debug, thiserror::Error)]
pub enum HeaderError {
    #[error("file header has wrong magic (not 'qoif')")]
    WrongMagic,
    #[error("either image dimension is zero")]
    ZeroDimension,
    #[error("invalid channel count or color space")]
    BadColorSpec,
}

impl Header {
    pub fn to_bytes(&self) -> [u8; 14] {
        let mut buf = [0u8; 14];
        buf[ 0.. 4].copy_from_slice(b"qoif");
        buf[ 4.. 8].copy_from_slice(&self.wide.to_be_bytes());
        buf[ 8..12].copy_from_slice(&self.high.to_be_bytes());
        buf[12..14].copy_from_slice(&(self.spec as u16).to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: [u8; 14]) -> Result<Self, HeaderError> {
        if &bytes[0..4] != b"qoif" {return Err(HeaderError::WrongMagic)}
        let wide = u32::from_be_bytes(bytes[4.. 8].try_into().unwrap());
        let high = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
        if wide == 0 || high == 0 {return Err(HeaderError::ZeroDimension)}
        let spec = match u16::from_le_bytes(bytes[12..14].try_into().unwrap()) {
            0x0003 => ColorSpec::Srgb8,
            0x0004 => ColorSpec::Srgb8A8,
            0x0103 => ColorSpec::Rgb8,
            0x0104 => ColorSpec::Rgba8,
            _ => return Err(HeaderError::BadColorSpec)
        };
        Ok(Header{wide, high, spec})
    }
}

#[derive(Debug)]
pub enum FileError {
    Decode(DecodeError),
    Encode(EncodeError),
    Header(HeaderError),
    InputTooShort,
    BadPadding,
}

pub fn decode_qoi_file(bytes: &[u8]) -> Result<(Header, Vec<Pixel>), FileError> {
    if bytes.len() < (14 + 1 + 8) {return Err(FileError::InputTooShort)}
    let header = Header::from_bytes(bytes[0..14].try_into().unwrap())
        .map_err(FileError::Header)?;
    let mut pixels = Vec::new();
    pixels.resize(header.wide as usize * header.high as usize, [0; 4]);
    let mut state = State::new();
    let rest = state.decode_some(&mut pixels, &bytes[14..])
        .map_err(FileError::Decode)?;
    if rest != [0, 0, 0, 0, 0, 0, 0, 1] {return Err(FileError::BadPadding)}
    Ok((header, pixels))
}

// TODO validate header
pub fn encode_qoi_file(header: Header, pixels: &[Pixel]) -> Result<Vec<u8>, FileError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&header.to_bytes());
    let mut state = State::new();
    state.encode_some(&mut bytes, pixels).map_err(FileError::Encode)?;
    state.encode_flush(&mut bytes).map_err(FileError::Encode)?;
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    Ok(bytes)
}

#[cfg(test)]
#[test]
fn decode_qoifs() {
    let qoif = [
        b'q', b'o', b'i', b'f',
        0, 0, 0, 1,
        0, 0, 0, 1,
        4, 0,

        0,

        0, 0, 0, 0, 0, 0, 0, 1,
    ];

    let (header, pixels) = decode_qoi_file(&qoif).unwrap();
    assert_eq!(header.wide, 1);
    assert_eq!(header.high, 1);
    assert_eq!(pixels.len(), 1);
    assert_eq!(pixels[0], [0, 0, 0, 0]);



    let qoif = [
        b'q', b'o', b'i', b'f',
        0, 0, 0, 2,
        0, 0, 0, 2,
        4, 0,

        0xff, 27, 146, 55, 203,
        0xc2,

        0, 0, 0, 0, 0, 0, 0, 1,
    ];

    let (header, pixels) = decode_qoi_file(&qoif).unwrap();
    assert_eq!(header.wide, 2);
    assert_eq!(header.high, 2);
    assert_eq!(pixels.len(), 4);
    assert_eq!(pixels[0], [27, 146, 55, 203]);
    assert_eq!(pixels[1], [27, 146, 55, 203]);
    assert_eq!(pixels[2], [27, 146, 55, 203]);
    assert_eq!(pixels[3], [27, 146, 55, 203]);
}

