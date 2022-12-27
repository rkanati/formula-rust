mod prm;
mod atlas;
mod road;

use {
    atlas::Atlas,
    prm::Prm,
    crate::bundle, 
    std::{collections::HashMap, rc::Rc},
    anyhow::{Result as Anyhow, Context as _},
    camino::{Utf8Path as Path, Utf8PathBuf as PathBuf},
};

pub fn make_bundle(wipeout_dir: impl AsRef<Path>) -> Anyhow<Vec<u8>> {
    let wipeout_dir = wipeout_dir.as_ref();
    let mut bundler = Bundler::new(wipeout_dir);

    // tracks
    for subdir in wipeout_dir.read_dir_utf8()? {
        let subdir = subdir?;
        let subdir_name = subdir.file_name();
        if !subdir_name.starts_with("track") {continue}
        make_track(&mut bundler, subdir_name.into())
            .with_context(|| format!("track: '{subdir_name}'"))?;
    }

    // ships
    make_ships(&mut bundler)?;

    // finalize
    bundler.bake()
}

fn make_sky(bundler: &mut Bundler, track_name: &Path)
    -> Anyhow<(Rc<bundle::Scene>, Rc<bundle::Image>)>
{
    let (image, atlas) = bundler.atlas(&track_name.join("sky.cmp"), None)?;
    let scene = bundler.scene(&track_name.join("sky.prm"), &atlas)?;
    Ok((scene, image))
}

fn make_track(bundler: &mut Bundler, track_name: &Path) -> Anyhow<()> {
    let (sky_scene, sky_image) = make_sky(bundler, track_name)?;
    let (scenery_image, scenery_atlas) = bundler.atlas(&track_name.join("scene.cmp"), None)?;
    let scenery_scene = bundler.scene(&track_name.join("scene.prm"), &scenery_atlas)?;

    let id = bundle::asset_id(track_name.as_str());

    let (road_mesh, road_image) = {
        let (image, atlas) = bundler.atlas(
            &track_name.join("library.cmp"),
            Some(&track_name.join("library.ttf"))
        )?;

        let trv = bundler.load(track_name.join("track.trv"))?;
        let trf = bundler.load(track_name.join("track.trf"))?;
        let mesh = road::make_mesh(&trv, &trf, &atlas)?;
        let mesh = Rc::new(mesh);
        bundler.meshes.add(id, mesh.clone());
        (mesh, image)
    };

    let track = bundle::Track {
        road_mesh, road_image,
        scenery_scene, scenery_image,
        sky_scene, sky_image,
    };

    bundler.tracks.add(id, track);
    Ok(())
}

fn make_ships(bundler: &mut Bundler) -> Anyhow<()> {
    log::info!("making ships");
    let (image, atlas) = bundler.atlas("common/allsh.cmp".into(), None)?;
    let scene = bundler.scene("common/allsh.prm".into(), atlas.as_ref())?;
    bundler.ship_image = Some(image);
    bundler.ship_scene = Some(scene);
    Ok(())
}

struct Bundler {
    wipeout_dir: PathBuf,
    images: bundle::Assets<(Rc<bundle::Image>, Rc<Atlas>)>,
    meshes: bundle::Assets<Rc<bundle::Mesh>>,
    scenes: bundle::Assets<Rc<bundle::Scene>>,

    tracks: bundle::Assets<bundle::Track>,
    ship_scene: Option<Rc<bundle::Scene>>,
    ship_image: Option<Rc<bundle::Image>>,
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
        let images = bundle::Assets::default();
        let meshes = bundle::Assets::default();
        let scenes = bundle::Assets::default();
        let tracks = bundle::Assets::default();
        let ship_scene = None;
        let ship_image = None;
        Bundler{wipeout_dir, images, meshes, scenes, tracks, ship_scene, ship_image}
    }

    fn bake(self) -> Anyhow<Vec<u8>> {
        let Bundler{tracks, ship_scene, ship_image, ..} = self;
        let ship_scene = ship_scene.unwrap();
        let ship_image = ship_image.unwrap();
        let root = bundle::Root{tracks, ship_scene, ship_image};
        let mut buffer = Vec::with_capacity(128 << 20);
        root.bake(&mut buffer)?;
        let compressed = lz4_flex::compress_prepend_size(&buffer);
        Ok(compressed)
    }

    fn atlas(&mut self, cmp_path: &Path, ttf_path: Option<&Path>)
        -> Anyhow<(Rc<bundle::Image>, Rc<Atlas>)>
    {
        let cmp = self.load(cmp_path)?;
        let hash = util::fnv1a_64(&cmp);
        let id = bundle::Id(hash);
        if let Some((image, atlas)) = self.images.get(id) {
            return Ok((image.clone(), atlas.clone()));
        }

        let images = formats::load_cmp(&cmp)?;
        let label = format!("{hash:016x}");
        let (image, atlas) = if let Some(ttf_path) = ttf_path {
            let ttf = self.load(ttf_path)?;
            let mip_mappings = atlas::parse_ttf(&ttf);
            Atlas::make_for_road(&label, &images, &mip_mappings)?
        }
        else {
            Atlas::make(&label, &images)?
        };
        let image = Rc::new(image);
        let atlas = Rc::new(atlas);

        self.images.add(id, (image.clone(), atlas.clone()));
        Ok((image, atlas))
    }

    fn scene(&mut self, prm_path: &Path, atlas: &Atlas) -> Anyhow<Rc<bundle::Scene>> {
        let prm = self.load(prm_path)?;
        let hash = util::fnv1a_64(&prm);
        let id = bundle::Id(hash);
        if let Some(scene) = self.scenes.get(id) {
            return Ok(scene.clone());
        }

        let prm = Prm::load(&prm, atlas).context(prm_path.to_owned())?;
        let mesh = Rc::new(prm.mesh);
        self.meshes.add(id, mesh.clone());
        let scene = Rc::new(bundle::Scene{mesh, objects: prm.objects});
        self.scenes.add(id, scene.clone());
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

