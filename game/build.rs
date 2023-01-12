
use gl_generator::*;

fn main() {
    log_init();

    let out_dir = camino::Utf8PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let mut file = std::fs::File::create(out_dir.join("gl_binds.rs")).unwrap();
    let extensions = ["GL_EXT_texture_filter_anisotropic"];
    Registry::new(Api::Gles2, (3, 2), Profile::Core, Fallbacks::All, extensions)
        .write_bindings(StructGenerator, &mut file)
        .unwrap();

    let bundle_path = out_dir.join("bundle");
    println!("cargo:rustc-env=BUNDLE_PATH={bundle_path}");

    let wipeout_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets");
    println!("cargo:rerun-if-changed={wipeout_dir}");

    let config = bundle::bundler::Config {
        wipeout_dir: wipeout_dir.into(),
        out_path: bundle_path,
    };
    bundle::bundler::make_bundle(config).unwrap();
}


fn log_init() {
    use simplelog::*;
    let simple = TermLogger::new(
        log::LevelFilter::Debug,
        Config::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    );
    let file = WriteLogger::new(
        log::LevelFilter::Debug,
        Config::default(),
        std::fs::File::create(concat!(env!("CARGO_MANIFEST_DIR"), "/build-script.log")).unwrap()
    );
    CombinedLogger::init(vec![simple, file]).unwrap();
}

