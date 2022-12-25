mod prm;
mod atlas;
mod road;

use {
    atlas::Atlas,
    prm::Prm,
    crate::bundle, 
    std::{collections::HashMap, rc::Rc},
    anyhow::Result as Anyhow,
    camino::{Utf8Path as Path, Utf8PathBuf as PathBuf},
};

pub fn make_bundle(wipeout_dir: &Path) -> Anyhow<Vec<u8>> {
    let mut bundler = Bundler::default();

    // tracks
    // NOTE track15 seems to be unfinished/busted - its scene.prm contains a prim with type 7
    // (quad, gouraud, untextured, unlit), but the data that follows is way off-point
    for track_i in 1..=14 {
        let track_name = format!("track{track_i:02}");
        let track_dir = wipeout_dir.join(&track_name);
        make_track(&mut bundler, &track_dir, &track_name)?;
    }

    // finalize
    bundler.bake()
}

fn make_track(
    bundler: &mut Bundler,
    track_path: &Path,
    asset_path: &str,
) -> Anyhow<bundle::Id/*<bundle::Track>*/>
{
    let (sky_image, sky_atlas) = bundler.atlas(&track_path.join("sky.cmp"), None)?;
    let sky_scene = bundler.scene(&track_path.join("sky.prm"), &sky_atlas)?;

    let (scenery_image, scenery_atlas) = bundler.atlas(&track_path.join("scene.cmp"), None)?;
    let scenery_scene = bundler.scene(&track_path.join("scene.prm"), &scenery_atlas)?;

    let (road_mesh, road_image) = {
        let (image, atlas) = bundler.atlas(
            &track_path.join("library.cmp"),
            Some(&track_path.join("library.ttf"))
        )?;

        let trv = std::fs::read(track_path.join("track.trv"))?;
        let trf = std::fs::read(track_path.join("track.trf"))?;
        let mesh = road::make_mesh(&trv, &trf, &atlas)?;
        let mesh = bundler.meshes.add(bundle::asset_id(&asset_path), mesh);
        (mesh, image)
    };

    let track = bundle::Track{
        road_mesh, road_image,
        scenery_scene, scenery_image,
        sky_scene, sky_image,
    };
    Ok(bundler.tracks.add(bundle::asset_id(&asset_path), track))
}


#[derive(Default)]
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
    pub fn bake(self) -> Anyhow<Vec<u8>> {
        let mut buffer = Vec::with_capacity(128 << 20);
        self.bundle.bake(&mut buffer)?;
        let compressed = lz4_flex::compress_prepend_size(&buffer);
        Ok(compressed)
    }

    pub fn atlas(&mut self, cmp_path: &Path, ttf_path: Option<&Path>)
        -> Anyhow<(bundle::Id, Rc<Atlas>)>
    {
        let cmp = std::fs::read(self.wipeout_dir.join(cmp_path))?;
        let hash = util::fnv1a_64(&cmp);
        if let Some((id, uvs)) = self.atlas_log.get(&hash) { return Ok((*id, uvs.clone())); }

        let images = formats::load_cmp(&cmp)?;
        let (image, atlas) = if let Some(ttf_path) = ttf_path {
            let ttf = std::fs::read(self.wipeout_dir.join(ttf_path))?;
            let mip_mappings = atlas::parse_ttf(&ttf);
            atlas::Atlas::make_for_road(&images, &mip_mappings)?
        }
        else {
            atlas::Atlas::make(&images)?
        };

        let id = bundle::Id(hash);
        self.bundle.images.add(id, image);
        let atlas = Rc::new(atlas);
        self.atlas_log.insert(hash, (id, atlas.clone()));
        Ok((id, atlas))
    }

    pub fn scene(&mut self, prm_path: &Path, atlas: &Atlas) -> Anyhow<bundle::Id> {
        let prm = std::fs::read(self.wipeout_dir.join(prm_path))?;
        let hash = util::fnv1a_64(&prm);
        if let Some(&id) = self.scene_log.get(&hash) { return Ok(id); }

        let id = bundle::Id(hash);
        let prm = Prm::load(&prm, atlas)?;
        let mesh = self.bundle.meshes.add(id, prm.mesh);
        let scene = self.bundle.scenes.add(id, bundle::Scene{mesh, objects: prm.objects});
        Ok(scene)
    }
}


