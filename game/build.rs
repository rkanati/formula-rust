
use gl_generator::*;

fn main() {
    let out_dir = camino::Utf8PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let mut file = std::fs::File::create(out_dir.join("gl_binds.rs")).unwrap();
    Registry::new(Api::Gles2, (3, 2), Profile::Core, Fallbacks::All, [])
        .write_bindings(StructGenerator, &mut file)
        .unwrap();

    let bundle_path = out_dir.join("bundle");
    println!("cargo:rustc-env=BUNDLE_PATH={bundle_path}");
    let wipeout = concat!(env!("CARGO_MANIFEST_DIR"), "/wipeout");
    println!("cargo:rerun-if-changed={wipeout}");
    let bundle = bundle::bundler::make_bundle(wipeout.into()).unwrap();
    std::fs::write(&bundle_path, bundle).unwrap();
}

