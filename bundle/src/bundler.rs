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
    -> Anyhow<(bundle::Id, bundle::Id)>
{
    let (image, atlas) = bundler.atlas(&track_name.join("sky.cmp"), None)?;
    let scene = bundler.scene(&track_name.join("sky.prm"), &atlas)?;
    Ok((scene, image))
}

fn make_track(bundler: &mut Bundler, track_name: &Path) -> Anyhow<bundle::Id> {
    let sky = make_sky(bundler, track_name).ok();
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
        let mesh = bundler.meshes.add(id, mesh);
        (mesh, image)
    };


    let track = bundle::Track {
        road_mesh, road_image,
        scenery_scene, scenery_image,
        sky,
    };
    Ok(bundler.tracks.add(id, track))
}

fn make_ships(bundler: &mut Bundler) -> Anyhow<()> {
    let (image, atlas) = bundler.atlas("common/allsh.cmp".into(), None)?;
    let scene = bundler.scene("common/allsh.prm".into(), atlas.as_ref())?;
    Ok(())
}

struct Bundler {
    wipeout_dir: PathBuf,
    bundle: bundle::Root,
    atlas_log: HashMap<u64, (bundle::Id, Rc<Atlas>)>,
    scene_log: HashMap<u64, bundle::Id>,
}

impl std::ops::Deref for Bundler {
    type Target = bundle::Root;
    fn deref(&self) -> &Self::Target {
        &self.bundle
    }
}

impl std::ops::DerefMut for Bundler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bundle
    }
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
        let bundle = bundle::Root::default();
        let atlas_log = HashMap::new();
        let scene_log = HashMap::new();
        Bundler{wipeout_dir, bundle, atlas_log, scene_log}
    }

    fn bake(self) -> Anyhow<Vec<u8>> {
        let mut buffer = Vec::with_capacity(128 << 20);
        self.bundle.bake(&mut buffer)?;
        let compressed = lz4_flex::compress_prepend_size(&buffer);
        Ok(compressed)
    }

    fn atlas(&mut self, cmp_path: &Path, ttf_path: Option<&Path>)
        -> Anyhow<(bundle::Id, Rc<Atlas>)>
    {
        let cmp = self.load(cmp_path)?;
        let hash = util::fnv1a_64(&cmp);
        if let Some((id, uvs)) = self.atlas_log.get(&hash) { return Ok((*id, uvs.clone())); }

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

        let id = bundle::Id(hash);
        self.bundle.images.add(id, image);
        let atlas = Rc::new(atlas);
        self.atlas_log.insert(hash, (id, atlas.clone()));
        Ok((id, atlas))
    }

    fn scene(&mut self, prm_path: &Path, atlas: &Atlas) -> Anyhow<bundle::Id> {
        let prm = self.load(prm_path)?;
        let hash = util::fnv1a_64(&prm);
        if let Some(&id) = self.scene_log.get(&hash) { return Ok(id); }

        let id = bundle::Id(hash);
        let prm = Prm::load(&prm, atlas).context(prm_path.to_owned())?;
        let mesh = self.bundle.meshes.add(id, prm.mesh);
        let scene = self.bundle.scenes.add(id, bundle::Scene{mesh, objects: prm.objects});
        Ok(scene)
    }
}

