use qoit::decode_qoi_file;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let qoif = std::fs::read(&path).unwrap();
    let (header, pixels) = decode_qoi_file(&qoif).unwrap();

    image::RgbaImage
        ::from_vec(
            header.wide, header.high,
            bytemuck::cast_vec(pixels),
        )
        .unwrap()
        .save(format!("{path}.png"))
        .unwrap();
}

