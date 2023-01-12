//mod atlas;
mod font;
mod road;
mod image_set;
mod model;

use {
    std::collections::HashMap,
    anyhow::{Result as Anyhow, Context as _},
    camino::{Utf8Path as Path, Utf8PathBuf as PathBuf},
};

pub struct Config {
    pub wipeout_dir: PathBuf,
    pub out_path:    PathBuf,
}

pub fn make_bundle(config: Config) -> Anyhow<()> {
    let mut bundler = Bundler::new(config)?;

    let fonts = make_fonts(&mut bundler, "fonts".into())?;
    let tracks = make_tracks(&mut bundler, "".into())?; // TODO tracks subdir
    let (ship_mset, ship_iset) = make_ships(&mut bundler)?;
    make_music(&mut bundler, "music".into())?;

    bundler.bake(move |aux_table| crate::Root {
        tracks,
        ship_mset,
        ship_iset,
        fonts,
        aux_table,
    })
}

fn make_music(bundler: &mut Bundler, music_dir: &Path) -> Anyhow<()> {
    log::info!("making music");
    for entry in bundler.asset_dir(music_dir).read_dir_utf8()? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {continue}
        let Some(name) = entry.file_name().strip_suffix(".opus") else {continue};
        bundler.aux_blob(&format!("music/{name}"), entry.path())?;
    }
    Ok(())
}




fn make_tracks(bundler: &mut Bundler, tracks_dir: &Path)
    -> Anyhow<HashMap<String, crate::Track>>
{
    log::info!("making tracks");
    let mut tracks = HashMap::new();
    for entry in bundler.asset_dir(tracks_dir).read_dir_utf8()? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {continue}
        let entry_name = entry.file_name();
        if !entry_name.starts_with("track") {continue}
        let track = make_track(bundler, entry_name.into())
            .with_context(|| format!("track: '{entry_name}'"))?;
        tracks.insert(entry_name.into(), track);
    }
    Ok(tracks)
}

fn make_track(bundler: &mut Bundler, track_name: &Path) -> Anyhow<crate::Track> {
    let (sky_mset, sky_iset) = make_sky(bundler, track_name)?;
    let scenery_scene = bundler.scene(&track_name.join("scene.prm"))?;
    let scenery_iset = bundler.image_set(&track_name.join("scene.cmp"), None)?;

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

    Ok(crate::Track {
        road_model, road_iset,
        scenery_scene, scenery_iset,
        sky_mset, sky_iset,
        graph,
    })
}


fn make_fonts(bundler: &mut Bundler, fonts_dir: &Path)
    -> Anyhow<HashMap<String, crate::Font>>
{
    log::info!("making fonts");
    let mut fonts = HashMap::new();
    for entry in bundler.asset_dir(fonts_dir).read_dir_utf8()
        .with_context(|| fonts_dir.to_string())?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() {continue}
        let Some(name) = entry.path().file_stem() else {continue};
        let ttf = bundler.load(entry.path())?;
        let font = match font::make_font(&ttf) {
            Err(font::Error::NotAFont) => continue,
            Err(font::Error::Other(e)) => return Err(e),
            Ok(font) => font
        };
        fonts.insert(name.into(), font);
    }
    Ok(fonts)
}

fn make_sky(bundler: &mut Bundler, track_name: &Path)
    -> Anyhow<(crate::ModelSet, crate::ImageSet)>
{
    let iset = bundler.image_set(&track_name.join("sky.cmp"), None)?;
    let mset = bundler.model_set(&track_name.join("sky.prm"))?;
    Ok((mset, iset))
}

fn make_ships(bundler: &mut Bundler) -> Anyhow<(crate::ModelSet, crate::ImageSet)> {
    log::info!("making ships");
    let iset = bundler.image_set("common/allsh.cmp".into(), None)?;
    let mset = bundler.model_set("common/allsh.prm".into())?;
    Ok((mset, iset))
}

struct Bundler {
    config: Config,
    aux_file: std::io::BufWriter<std::fs::File>,
    aux_tab: HashMap<String, u64>,
}

impl Bundler {
    fn new(config: Config) -> Anyhow<Self> {
        let aux_path = config.out_path.with_file_name("bundle-aux");
        let aux_file = std::io::BufWriter::with_capacity(
            0x10_0000,
            std::fs::File::create(&aux_path)
                .with_context(|| format!("aux_path: {aux_path}"))?
        );
        let aux_tab = HashMap::new();
        Ok(Bundler{config, aux_file, aux_tab})
    }

    fn asset_dir(&self, rel: &Path) -> PathBuf {
        self.config.wipeout_dir.join(rel)
    }

    fn bake(self, f: impl FnOnce(HashMap<String, u64>) -> crate::Root) -> Anyhow<()> {
        let root = f(self.aux_tab);
        let mut buffer = Vec::with_capacity(128 << 20);
        root.bake(&mut buffer)?;
        let compressed = lz4_flex::compress_prepend_size(&buffer);
        std::fs::write(self.config.out_path, compressed)?;
        Ok(())
    }

    fn load(&self, path: impl AsRef<Path>) -> Anyhow<Vec<u8>> {
        let path = self.config.wipeout_dir.join(path);
        use anyhow::Context as _;
        let bytes = std::fs::read(&path).context(path)?;
        Ok(bytes)
    }

    // TODO dedup
    fn image_set(&mut self, cmp_path: &Path, ttf_path: Option<&Path>) -> Anyhow<crate::ImageSet> {
        let cmp = self.load(cmp_path)?;
        let ttf = ttf_path.map(|p| self.load(p)).transpose()?;
        let label = cmp_path.as_str().replace("/", "_");
        image_set::build(&label, &cmp, ttf.as_deref())
    }

    /*fn atlas(&mut self, cmp_path: &Path)
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
    }*/

    fn scene(&mut self, prm_path: &Path) -> Anyhow<crate::Scene> {
        let prm = self.load(prm_path)?;
        //let debug_label = prm_path.as_str();
        model::build_scene(&prm)
    }

    fn model_set(&mut self, prm_path: &Path) -> Anyhow<crate::ModelSet> {
        let prm = self.load(prm_path)?;
        //let debug_label = prm_path.as_str();
        model::build(&prm)
    }

    fn aux_blob(&mut self, name: &str, path: &Path) -> Anyhow<()> {
        use std::io::Seek as _;
        let start = self.aux_file.stream_position()?;
        let mut reader = std::io::BufReader::with_capacity(0x10_0000, std::fs::File::open(path)?);
        std::io::copy(&mut reader, &mut self.aux_file)?;
        self.aux_tab.insert(name.into(), start);
        Ok(())
    }

    /*fn scene(&mut self, prm_path: &Path, atlas: &Atlas) -> Anyhow<Rc<crate::Scene>> {
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
    }*/
}

#[repr(C, align(4))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RawRgbx([u8; 4]);

impl RawRgbx {
    //const MAGENTA: Self = Self([0x80, 0x00, 0x80, 0xff]);
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

