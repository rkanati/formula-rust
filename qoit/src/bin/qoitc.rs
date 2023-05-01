use qoit::encode_qoi_file;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let image = std::fs::read(&path).unwrap();
    let image = image::load_from_memory(&image).unwrap().into_rgba8();
    let qoif = encode_qoi_file(
        qoit::Header {
            wide: image.width(),
            high: image.height(),
            spec: qoit::ColorSpec::Srgb8A8,
        },
        bytemuck::cast_slice(&image),
    ).unwrap();
    std::fs::write(format!("{path}.qoi"), qoif).unwrap();
}

