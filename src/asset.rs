use std::{borrow::Cow, path::*};

use ::serde::de::DeserializeSeed;
use anyhow::{anyhow, Result};
use bevy::{
    asset::{Asset, AssetLoader, AssetPath, LoadContext, LoadedAsset},
    prelude::*,
    reflect::{serde::UntypedReflectDeserializer, TypeRegistryArc},
    utils::BoxedFuture,
};
use bevy_hanabi::EffectAsset;
use relative_path::*;

use crate::{gradient::*, reffect::*, LiveEffect};

// This is basically a dupe of SceneLoader.
pub struct HanLoader {
    type_registry: TypeRegistryArc,
}

impl FromWorld for HanLoader {
    fn from_world(world: &mut World) -> Self {
        let type_registry = world.resource::<AppTypeRegistry>();
        Self {
            type_registry: type_registry.0.clone(),
        }
    }
}

impl AssetLoader for HanLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<()>> {
        Box::pin(async move {
            // This is way easier, but requires deriving Deserialize directly.
            //let re: REffect = ron::de::from_bytes(bytes)?;

            let mut deserializer = ron::de::Deserializer::from_bytes(bytes)?;
            let type_registry = self.type_registry.read();
            let rde = UntypedReflectDeserializer::new(&type_registry);
            let re = rde.deserialize(&mut deserializer).map_err(|e| {
                let span_error = deserializer.span_error(e);
                anyhow!(
                    "{} at {}:{}",
                    span_error.code,
                    load_context.path().display(),
                    span_error.position,
                )
            })?;

            let mut reff =
                <REffect as FromReflect>::take_from_reflect(re).expect("reflect to reffect");

            // Load the particle texture, if set.
            let loaded_asset = match reff.render_particle_texture {
                ParticleTexture::Path(path) => {
                    let rel_path = RelativePath::from_path(&path)?;
                    // This looks silly, but it just converts the platform-independent relative path
                    // into a native one.
                    let path = rel_path.to_path("");
                    let asset_path = AssetPath::new_ref(&path, None);
                    let handle = load_context.get_handle(asset_path.clone());
                    reff.render_particle_texture = ParticleTexture::Texture(handle);
                    LoadedAsset::new(reff).with_dependency(asset_path)
                }
                _ => LoadedAsset::new(reff),
            };

            load_context.set_default_asset(loaded_asset);

            Ok(())
        })
    }

    // Should .ron be reserved for non-reflect?
    fn extensions(&self) -> &[&str] {
        &["han", "han.ron"]
    }
}

// Does it make sense to merge this with the loader?
// TODO: add a preview, e.g. thumbnail for images
#[derive(Resource)]
pub struct AssetPaths<T: Asset> {
    pub root_path: PathBuf,
    pub extension: &'static str,
    pub paths: Vec<(PathBuf, Option<Handle<T>>, bool)>,
}

impl<T: Asset> AssetPaths<T> {
    pub fn new(extension: &'static str) -> Self {
        // TODO read asset dir
        let root_path = PathBuf::from("assets").canonicalize().unwrap();

        // TODO read from asset io instead of glob - similarly, can we read all known assets by
        // extension?
        let pat = format!("{}/**/*.{}", root_path.to_str().unwrap(), extension);
        let paths = glob::glob(&pat)
            .map_err(|e| error!("failed to find asset paths: {:?}", e))
            .map(|paths| {
                paths
                    .map(|path| {
                        path.map_err(|e| error!("error: {:?}", e)).and_then(|path| {
                            // We want the paths stored relative to assets, not the root.
                            path.strip_prefix(&root_path)
                                .map(|path| path.to_path_buf())
                                .map_err(|e| error!("error: {:?}", e))
                        })
                    })
                    // Filter out errors.
                    .flatten()
                    .map(|path| (path, None, true))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            root_path,
            extension,
            paths,
        }
    }

    // Iterate all paths with handles. Is this needed?
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &Handle<T>)> {
        self.paths
            .iter()
            .filter_map(|(p, h, ..)| h.as_ref().map(|h| (p.as_path(), h)))
    }

    // This is just to get around multiple borrows. Needs revision.
    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (&Path, &mut PathBuf, &mut Option<Handle<T>>, &mut bool)> {
        self.paths
            .iter_mut()
            .map(|(p, h, saved)| (self.root_path.as_ref(), p, h, saved))
    }
}

// Make sure multiple assets don't point to the same path?
pub fn validate_path<'a>(
    path: &'a str,
    ext: &str,
    root_path: &Path,
    //asset_server: &AssetServer,
) -> Result<Cow<'a, Path>> {
    use path_absolutize::path_dedot::*;

    // No wasm/android:
    // let asset_io = asset_server
    //     .asset_io()
    //     .downcast_ref::<FileAssetIo>()
    //     .ok_or_else(|| anyhow!("not FileAssetIo"))?;
    // let asset_dir = asset_io.root_path();

    let path = Path::new(path);

    // Check empty path/file name.
    if path.as_os_str().len() == 0 {
        return Err(anyhow!("empty path"));
    } else if path.file_name().is_none() {
        return Err(anyhow!("no file name: {}", path.display()));
    }

    // Contain the path to assets. This can make relative paths absolute by using "..".
    let path = path.parse_dot_from(root_path)?;
    let path = strip_prefix(path, root_path)?;

    // Make sure it's relative to assets. This should never be true after stripping the prefix but
    // who knows.
    if path.is_absolute() {
        return Err(anyhow!("path not relative: {}", path.display()));
    }

    // Ensure extension.
    let path = with_extension(path.into(), ext);

    Ok(path)
}

// AFAIK, there is no way to (without unsafe) easily replace the reference inside a Cow with a
// shorter reference even if they both point to the same memory. So we copy the stripped path into a
// new PathBuf.
fn strip_prefix<'a>(path: Cow<'a, Path>, prefix: &Path) -> Result<Cow<'a, Path>> {
    Ok(if path.is_absolute() {
        Cow::Owned(path.strip_prefix(prefix)?.into())
    } else {
        path
    })
}

// Like Path::with_extension but Cow-like.
pub fn with_extension<'a>(path: Cow<'a, Path>, extension: &str) -> Cow<'a, Path> {
    if path.extension().is_some_and(|ext| ext == extension) {
        path
    } else {
        Cow::from(path.with_extension(extension))
    }
}

// Make unique path for new assets.
pub fn unique_path<'a>(path_buf: &'a PathBuf, ext: &str) -> Result<Cow<'a, Path>> {
    //use path_absolutize::*;

    if !path_buf.symlink_metadata().is_ok() {
        Ok(Cow::from(path_buf))
    } else {
        let file_prefix = path_buf
            .with_extension("") // this clones
            .file_name()
            .ok_or_else(|| anyhow!("no file name: {}", path_buf.display()))?
            .to_string_lossy()
            .to_string();

        let mut path_buf = path_buf.clone();
        for i in 1..=64 {
            path_buf.set_file_name(format!("{}{}.{}", file_prefix, i, ext));
            if !path_buf.symlink_metadata().is_ok() {
                return Ok(Cow::from(path_buf));
            }
        }

        Err(anyhow!(
            "failed to make unique path: {}",
            path_buf.display()
        ))
    }
}

pub fn save_effect(
    mut effect: REffect,
    // Root and relative path to asset.
    (root_path, path): (&Path, &Path),
    type_registry: AppTypeRegistry,
    asset_server: &AssetServer,
) -> Result<()> {
    use bevy::{reflect::serde::ReflectSerializer, tasks::IoTaskPool};
    use std::{fs::File, io::Write};

    // Convert texture to asset path:
    match &mut effect.render_particle_texture {
        ParticleTexture::Texture(handle) => {
            if let Some(path) = asset_server.get_handle_path(handle.id()) {
                // Write platform-independent relative path.
                let rel_path = RelativePathBuf::from_path(path.path())?;
                effect.render_particle_texture = ParticleTexture::Path(rel_path.into_string());
            }
        }
        _ => (),
    }

    // Clone to move.
    let effect_path = root_path.join(path);

    IoTaskPool::get()
        .spawn(async move {
            let ron = {
                let type_registry = type_registry.read();
                let rs = ReflectSerializer::new(&effect, &type_registry);
                ron::ser::to_string_pretty(&rs, ron::ser::PrettyConfig::new())
                    .map_err(|e| error!("failed to serialize: {:?}", e))
            };

            // Should this handle creation of directories or just error?
            ron.and_then(|ron| {
                File::create(&effect_path)
                    .and_then(|mut file| file.write(ron.as_bytes()))
                    .map_err(|e| error!("{}", e))
                    .map(|bytes| info!("saved effect ({} bytes): {:?}", bytes, effect_path))
            })
        })
        .detach();

    Ok(())
}

pub fn spawn_circle(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut reffects: ResMut<Assets<REffect>>,
) {
    use bevy_hanabi::*;

    let mut gradient = bevy_hanabi::Gradient::new();
    gradient.add_key(0.0, Vec4::splat(1.0));
    gradient.add_key(0.5, Vec4::splat(1.0));
    gradient.add_key(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0));

    // spawn default effect
    let effect = REffect {
        name: "default".to_owned(),
        capacity: 32768,
        spawner: Spawner::once(32.0.into(), true),
        init_position: InitPosition::Circle(InitPositionCircleModifier {
            center: Vec3::Y * 0.1,
            axis: Vec3::Y,
            radius: 0.4,
            ..default()
        }),
        init_velocity: Some(InitVelocity::Circle(InitVelocityCircleModifier {
            axis: Vec3::Y,
            speed: Value::Uniform((1.0, 1.5)),
            ..default()
        })),
        init_lifetime: Some(InitLifetimeModifier {
            lifetime: 5_f32.into(),
        }),
        render_particle_texture: asset_server.load("plus.png").into(),
        render_color_over_lifetime: Some(ColorGradient::default()),
        ..default()
    };

    // Save both asset handles.
    commands.spawn((
        ParticleEffectBundle::new(effects.add(effect.to_effect_asset(&asset_server))),
        LiveEffect(reffects.add(effect)),
    ));
}
