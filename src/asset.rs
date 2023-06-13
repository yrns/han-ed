use std::path::*;

use ::serde::de::DeserializeSeed;
use anyhow::{anyhow, Result};
use bevy::{
    asset::{Asset, AssetLoader, AssetPath, LoadContext, LoadedAsset},
    prelude::*,
    reflect::{serde::UntypedReflectDeserializer, TypeRegistryArc},
    utils::BoxedFuture,
};
use bevy_hanabi::EffectAsset;

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

    fn extensions(&self) -> &[&str] {
        &["han", "han.ron"]
    }
}

// Does it make sense to merge this with the loader?
// TODO: add a preview, e.g. thumbnail for images
#[derive(Resource)]
pub struct AssetPaths<T: Asset> {
    pub extension: &'static str,
    pub paths: Vec<(PathBuf, Option<Handle<T>>)>,
}

impl<T: Asset> AssetPaths<T> {
    pub fn new(extension: &'static str) -> Self {
        let paths = glob::glob(&format!("assets/**/*.{}", extension))
            .map_err(|e| error!("failed to find asset paths: {:?}", e))
            .map(|paths| {
                paths
                    .map(|path| {
                        path.map_err(|e| error!("error: {:?}", e)).and_then(|path| {
                            // We want the paths stored relative to assets, not the root.
                            path.strip_prefix("assets")
                                .map(|path| path.to_path_buf())
                                .map_err(|e| error!("error: {:?}", e))
                        })
                    })
                    // Filter out errors.
                    .flatten()
                    .map(|path| (path, None))
                    .collect()
            })
            .unwrap_or_default();

        Self { extension, paths }
    }

    // Iterate all paths with handles.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &Handle<T>)> {
        self.paths
            .iter()
            .filter_map(|(p, h)| h.as_ref().map(|h| (p.as_path(), h)))
    }
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
