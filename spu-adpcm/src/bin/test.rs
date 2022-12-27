
#![feature(array_chunks)]
#![feature(slice_flatten)]

fn main() {
    let in_path = std::env::args().nth(1).unwrap();
    let out_name = std::env::args().nth(2).unwrap();

    let adpcm = std::fs::read(in_path).unwrap();
    let frames: &[[u8; 16]] = bytemuck::cast_slice(&adpcm[..]);

    frames
        .split_inclusive(|frame| frame[1] & 1 != 0)
        .enumerate()
        .scan(0, |off, (i, chunk)| {
            let chunk_len_bytes = chunk.len() * 16;
            println!("chunk {i:02} at +{off:05x}; {chunk_len_bytes}B", off=*off);
            *off += chunk_len_bytes;
            Some(chunk)
        })
        .map(|adpcm| adpcm.into_iter().copied()
            .scan((0i32, 0i32), |(pp, p), [sf, _flags, pairs@..]: [u8; 16]| {
                let shift  = (sf >> 0) & 0xf;
                let filter = (sf >> 4) & 0x7;
                assert!(filter <= 4);
                const PT: [i32; 5] = [0, 60, 115,  98, 122];
                const NT: [i32; 5] = [0,  0, -52, -55, -60];

                let mut bytes = [0; 28];
                bytes.copy_from_slice(
                    pairs
                        .map(|pair| [pair & 0xf, pair >> 4])
                        .flatten()
                );

                let f0 = PT[filter as usize];
                let f1 = NT[filter as usize];

                let samples = bytes.map(|byte| {
                    let shifted = (((byte as i16) << 12) >> shift) as i32; 
                    let sample = shifted + ((*p * f0 + *pp * f1 + 32) >> 6);
                    let sample = sample.clamp(-0x8000, 0x7fff);
                    *pp = *p;
                    *p = sample;
                    sample as i16
                });

                Some(samples)
            })
            .flatten()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<u8>>()
        )
        .enumerate()
        .for_each(|(i, pcm)| {
            let out_path = format!("{out_name}-{i:02}.wav");

            const CD: u32 = 44_100;
            const RATE: u32 = CD / 2;
            let fmt_chunk = [
                u32::from_le_bytes(*b"fmt "),
                16,
                0x0001_0001,
                RATE,
                RATE * 2,
                0x0010_0002,
            ];

            let data_head = [
                u32::from_le_bytes(*b"data"),
                pcm.len() as u32
            ];

            let contents_len
                = 4
                + std::mem::size_of_val(&fmt_chunk) as u32
                + std::mem::size_of_val(&data_head) as u32
                + pcm.len() as u32;

            let riff_head = [
                u32::from_le_bytes(*b"RIFF"),
                contents_len,
                u32::from_le_bytes(*b"WAVE"),
            ];

            let mut out = std::fs::File::create(out_path).unwrap();
            use std::io::Write as _;
            out.write_all(bytemuck::bytes_of(&riff_head)).unwrap();
            out.write_all(bytemuck::bytes_of(&fmt_chunk)).unwrap();
            out.write_all(bytemuck::bytes_of(&data_head)).unwrap();
            out.write_all(&pcm[..]).unwrap();
        });
}

