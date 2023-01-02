mod atlas;
mod font;
mod prm;
mod road;
mod image_set;

use {
    atlas::Atlas,
    prm::Prm,
    std::{rc::Rc, collections::HashMap},
    anyhow::{Result as Anyhow, Context as _},
    camino::{Utf8Path as Path, Utf8PathBuf as PathBuf},
};

pub fn make_bundle(wipeout_dir: impl AsRef<Path>) -> Anyhow<Vec<u8>> {
    let wipeout_dir = wipeout_dir.as_ref();
    let mut bundler = Bundler::new(wipeout_dir);

    make_fonts(&mut bundler, &wipeout_dir.join("fonts"))?;
    make_tracks(&mut bundler, wipeout_dir)?;
    make_ships(&mut bundler)?;

    bundler.bake()
}

fn make_tracks(bundler: &mut Bundler, tracks_dir: &Path) -> Anyhow<()> {
    log::info!("making tracks");
    for entry in tracks_dir.read_dir_utf8()? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {continue}
        let entry_name = entry.file_name();
        if !entry_name.starts_with("track") {continue}
        make_track(bundler, entry_name.into())
            .with_context(|| format!("track: '{entry_name}'"))?;
    }
    Ok(())
}

fn make_fonts(bundler: &mut Bundler, fonts_dir: &Path) -> Anyhow<()> {
    log::info!("making fonts");
    for entry in fonts_dir.read_dir_utf8().with_context(|| fonts_dir.to_string())? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {continue}
        let Some(name) = entry.path().file_stem() else {continue};
        let ttf = bundler.load(entry.path())?;
        let font = match font::make_font(&ttf, name) {
            Err(font::Error::NotAFont) => continue,
            Err(font::Error::Other(e)) => return Err(e),
            Ok(font) => font
        };
        bundler.fonts.insert(name.to_owned(), font);
    }
    log::info!("got {} fonts", bundler.fonts.len());
    Ok(())
}

fn make_sky(bundler: &mut Bundler, track_name: &Path)
    -> Anyhow<(Rc<crate::Scene>, Rc<crate::Image>)>
{
    let (image, atlas) = bundler.atlas(&track_name.join("sky.cmp"))?;
    let scene = bundler.scene(&track_name.join("sky.prm"), &atlas)?;
    Ok((scene, image))
}

fn make_track(bundler: &mut Bundler, track_name: &Path) -> Anyhow<()> {
    let (sky_scene, sky_image) = make_sky(bundler, track_name)?;
    let (scenery_image, scenery_atlas) = bundler.atlas(&track_name.join("scene.cmp"))?;
    let scenery_scene = bundler.scene(&track_name.join("scene.prm"), &scenery_atlas)?;

    let (road_model, graph, road_iset) = {
        let iset = bundler.image_set(
            &track_name.join("library.cmp"),
            Some(&track_name.join("library.ttf"))
        )?;

        let trv = bundler.load(track_name.join("track.trv"))?;
        let trf = bundler.load(track_name.join("track.trf"))?;
        let trs = bundler.load(track_name.join("track.trs"))?;
        let (model, graph) = road::make_road(&trv, &trf, &trs)?;
        (model, graph, iset)
    };

    let track = crate::Track {
        road_model, road_iset,
        scenery_scene, scenery_image,
        sky_scene, sky_image,
        graph,
    };

    bundler.tracks.insert(track_name.to_string(), track);
    Ok(())
}

fn make_ships(bundler: &mut Bundler) -> Anyhow<()> {
    log::info!("making ships");
    let (image, atlas) = bundler.atlas("common/allsh.cmp".into())?;
    let scene = bundler.scene("common/allsh.prm".into(), atlas.as_ref())?;
    bundler.ship_image = Some(image);
    bundler.ship_scene = Some(scene);
    Ok(())
}

#[derive(Default)]
struct Bundler {
    wipeout_dir: PathBuf,
    images: HashMap<u64, (Rc<crate::Image>, Rc<Atlas>)>,
    meshes: HashMap<u64, Rc<crate::Mesh>>,
    scenes: HashMap<u64, Rc<crate::Scene>>,
    fonts: HashMap<String, crate::Font>,

    tracks: crate::Assets<crate::Track>,
    ship_scene: Option<Rc<crate::Scene>>,
    ship_image: Option<Rc<crate::Image>>,
}

impl Bundler {
    fn load(&self, path: impl AsRef<Path>) -> Anyhow<Vec<u8>> {
        let path = self.wipeout_dir.join(path);
        use anyhow::Context as _;
        let bytes = std::fs::read(&path).context(path)?;
        Ok(bytes)
    }

    fn new(wipeout_dir: impl AsRef<Path>) -> Self {
        let wipeout_dir = wipeout_dir.as_ref().to_owned();
        Bundler{wipeout_dir, ..Default::default()}
    }

    fn bake(self) -> Anyhow<Vec<u8>> {
        let Bundler{tracks, ship_scene, ship_image, ..} = self;
        let ship_scene = ship_scene.unwrap();
        let ship_image = ship_image.unwrap();
        let fonts = self.fonts;
        let root = crate::Root{tracks, ship_scene, ship_image, fonts};
        let mut buffer = Vec::with_capacity(128 << 20);
        root.bake(&mut buffer)?;
        let compressed = lz4_flex::compress_prepend_size(&buffer);
        Ok(compressed)
    }

    // TODO dedup
    fn image_set(&mut self, cmp_path: &Path, ttf_path: Option<&Path>) -> Anyhow<crate::ImageSet> {
        let cmp = self.load(cmp_path)?;
        let ttf = ttf_path.map(|p| self.load(p)).transpose()?;
        let label = cmp_path.components().nth(0).unwrap().as_str();
        image_set::build(label, &cmp, ttf.as_deref())
    }

    fn atlas(&mut self, cmp_path: &Path)
        -> Anyhow<(Rc<crate::Image>, Rc<Atlas>)>
    {
        let cmp = self.load(cmp_path)?;
        let hash = util::fnv1a_64(&cmp);
        if let Some((image, atlas)) = self.images.get(&hash) {
            return Ok((image.clone(), atlas.clone()));
        }

        let images = formats::load_cmp(&cmp)?;
        let label = format!("{hash:016x}");
        let (image, atlas) = Atlas::build(&label, &images)?;
        let image = Rc::new(image);
        let atlas = Rc::new(atlas);

        self.images.insert(hash, (image.clone(), atlas.clone()));
        Ok((image, atlas))
    }

    fn scene(&mut self, prm_path: &Path, atlas: &Atlas) -> Anyhow<Rc<crate::Scene>> {
        let prm = self.load(prm_path)?;
        let hash = util::fnv1a_64(&prm);
        if let Some(scene) = self.scenes.get(&hash) {
            return Ok(scene.clone());
        }

        let debug_name = prm_path.as_str();
        let Prm{objects, sprites, mesh} = Prm::load(&prm, debug_name, atlas)
            .with_context(|| prm_path.to_owned())?;
        let mesh = Rc::new(mesh);
        self.meshes.insert(hash, mesh.clone());
        let scene = Rc::new(crate::Scene{mesh, objects, sprites});
        self.scenes.insert(hash, scene.clone());
        Ok(scene)
    }
}

#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RawRgbx([u8; 4]);

impl RawRgbx {
    const MAGENTA: Self = Self([0x80, 0x00, 0x80, 0xff]);
}

impl From<RawRgbx> for [f32; 3] {
    fn from(RawRgbx([r,g,b,_]): RawRgbx) -> Self {
        [r,g,b].map(|x| (x as f32 / 128.))
    }
}

impl From<RawRgbx> for [crate::UNorm8; 4] {
    fn from(RawRgbx([r,g,b,_]): RawRgbx) -> Self {
        [r,g,b,128].map(|x| crate::UNorm8(x.clamp(0, 127) << 1))
    }
}

impl From<RawRgbx> for [crate::UNorm8; 3] {
    fn from(RawRgbx([r,g,b,_]): RawRgbx) -> Self {
        [r,g,b].map(|x| crate::UNorm8(x.clamp(0, 127) << 1))
    }
}

