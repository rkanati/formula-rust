
fn main() {
    let path = camino::Utf8PathBuf::from(
        std::env::args().nth(1).expect("usage: uncmp input.cmp")
    );
    let cmp = std::fs::read(&path).unwrap();
    let images = formats::load_cmp(&cmp).unwrap();

    let stem = path.file_name().unwrap();
    for (i, image) in images.into_iter().enumerate() {
        let name = format!("{stem}.{i:03}.png");
        eprintln!("{name}");
        image.save(&name).unwrap();
    }
}

