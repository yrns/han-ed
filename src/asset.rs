use std::path::*;

use bevy::{
    asset::{Asset, AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    utils::BoxedFuture,
};

use crate::reffect::REffect;
use crate::*;

#[derive(Default)]
pub struct HanAssetLoader;

impl AssetLoader for HanAssetLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let re: REffect = ron::de::from_bytes(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(re));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["han"]
    }
}

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
}

pub fn spawn_circle(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut reffects: ResMut<Assets<REffect>>,
) {
    let mut gradient = Gradient::new();
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
        render_particle_texture: Some("plus.png".into()),
        render_color_over_lifetime: Some(ColorOverLifetimeModifier { gradient }),
        render_size_over_lifetime: Some(SizeOverLifetimeModifier {
            gradient: Gradient::constant([0.2; 2].into()),
        }),
        ..default()
    };

    // Save both asset handles.
    commands.spawn((
        ParticleEffectBundle::new(effects.add(effect.to_effect_asset(&asset_server))),
        LiveEffect(reffects.add(effect)),
    ));
}
