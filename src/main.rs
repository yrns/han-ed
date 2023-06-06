pub mod asset;
pub mod reffect;

use std::{fs::File, io::Write};

use asset::*;

use bevy::{
    core_pipeline::bloom::BloomSettings,
    log::LogPlugin,
    prelude::*,
    render::{render_resource::WgpuFeatures, settings::WgpuSettings, RenderPlugin},
    scene::*,
    tasks::IoTaskPool,
};
use bevy_egui::{
    egui::{self, CollapsingHeader},
    EguiContexts, EguiPlugin,
};
use bevy_hanabi::prelude::*;

use reffect::*;

#[derive(Component)]
pub struct LiveEffect(Handle<REffect>);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut wgpu_settings = WgpuSettings::default();
    wgpu_settings
        .features
        .set(WgpuFeatures::VERTEX_WRITABLE_STORAGE, true);

    App::default()
        .insert_resource(ClearColor(Color::DARK_GRAY))
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    level: bevy::log::Level::WARN,
                    filter: "bevy_hanabi=warn,spawn=trace".to_string(),
                })
                // .set(AssetPlugin {
                //     watch_for_changes: ChangeWatcher::with_delay(Duration::from_millis(400)),
                //     ..default()
                // })
                .set(RenderPlugin { wgpu_settings })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "han-ed".to_string(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_system(bevy::window::close_on_esc)
        .add_plugin(HanabiPlugin)
        //.register_type::<REffect>() add_asset::<T> registers Handle<T>
        .add_asset::<REffect>()
        .init_asset_loader::<asset::HanAssetLoader>()
        .insert_resource(AssetPaths::<REffect>::new("han"))
        .add_plugin(EguiPlugin)
        // .add_plugin(bevy_inspector_egui::quick::AssetInspectorPlugin::<
        //     EffectAsset,
        // >::default())
        .add_startup_system(setup)
        .add_system(han_ed_ui)
        .run();

    Ok(())
}

fn setup(
    //asset_server: Res<AssetServer>,
    mut commands: Commands,
    //mut effect_assets: ResMut<EffectAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera.
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(3.0, 3.0, 5.0)
                .looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
            ..default()
        },
        BloomSettings::default(),
        FogSettings::default(),
    ));

    // Ground plane.
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane {
                size: 8.0,
                ..default()
            })),
            material: materials.add(Color::GRAY.into()),
            ..Default::default()
        })
        .insert(Name::new("ground"));
}

fn han_ed_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut cameras: Query<(&mut Camera, &mut BloomSettings)>,
    asset_server: Res<AssetServer>,
    _images: Res<Assets<Image>>,
    mut reffect_paths: ResMut<AssetPaths<REffect>>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut reffects: ResMut<Assets<REffect>>,
    mut live_effects: Query<(
        Entity,
        &mut EffectSpawner,
        &mut ParticleEffect,
        &mut LiveEffect,
    )>,
) {
    // let mut ctx = world
    //     .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
    //     .single(world)
    //     .clone();
    // ctx.get_mut();

    egui::Window::new("han-ed").show(contexts.ctx_mut(), |ui| {
        // show/hide, pause, slow time? reset
        // move entity w/ mouse?
        CollapsingHeader::new("Global")
            .default_open(true)
            .show(ui, |ui| {
                let (mut c, mut bloom) = cameras.single_mut();
                ui.checkbox(&mut c.hdr, "HDR");
                ui.horizontal(|ui| {
                    ui.label("Bloom:");
                    ui.add(
                        egui::widgets::DragValue::new(&mut bloom.intensity)
                            .clamp_range(0.0..=1.0)
                            .speed(0.01),
                    );
                });
            });

        // We want to keep this around so that we can package these live effects into a scene later?
        CollapsingHeader::new("Live")
            .default_open(true)
            .show(ui, |ui| {
                for (entity, mut spawner, effect, live_effect) in live_effects.iter_mut() {
                    ui.label(format!(
                        "{:?}: {:?}, {:?}",
                        entity, effect.handle, live_effect.0
                    ));
                    if ui.button("Reset").clicked() {
                        spawner.reset();
                    }
                }
            });

        CollapsingHeader::new("Effects")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("New").clicked() {
                        // spawn new
                    };
                    if ui.button("Random").clicked() {
                        // spawn random
                    }
                });
                ui.separator();

                // duplicate, remove?, does rename work?
                for (path, handle) in reffect_paths.paths.iter_mut() {
                    match handle {
                        Some(handle) => match reffects.get_mut(&handle) {
                            Some(re) => {
                                CollapsingHeader::new(&re.name)
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(format!("{}", re.name));

                                            if let Some((entity, ..)) = live_effects
                                                .iter()
                                                .find(|(_, _, _, e)| &e.0 == handle)
                                            {
                                                if ui.button("Hide").clicked() {
                                                    // Despawn the live effect.
                                                    commands.get_entity(entity).unwrap().despawn();
                                                }
                                            } else {
                                                if ui.button("Show").clicked() {
                                                    // Spawn new live effect.
                                                    commands.spawn((
                                                        ParticleEffectBundle::new(effects.add(
                                                            re.to_effect_asset(&asset_server),
                                                        )),
                                                        LiveEffect(handle.clone()),
                                                    ));
                                                }
                                            }

                                            // TODO confirm overwrite if the name has changed
                                            if ui.button("Save").clicked() {
                                                #[cfg(not(target_arch = "wasm32"))]
                                                let file_name = format!("assets/{}.han", re.name);
                                                let effect = re.clone();
                                                IoTaskPool::get()
                                                    .spawn(async move {
                                                        let ron = serialize_ron(effect).unwrap();
                                                        File::create(file_name)
                                                            .and_then(|mut file| {
                                                                file.write(ron.as_bytes())
                                                            })
                                                            .map_err(|e| error!("{}", e))
                                                    })
                                                    .detach();
                                            }
                                        });
                                    });
                            }
                            None => {
                                ui.label("..."); // loading still
                            }
                        },
                        None => {
                            ui.label(format!("{}", path.display()));
                            if ui.button("Load").clicked() {
                                *handle = Some(asset_server.load(path.as_path()));
                            }
                        }
                    }
                }
            });
    });
}

#[allow(unused)]
pub fn save_scene(world: &mut World) {
    //if ui.button("save scene").clicked()

    let registry = world.resource::<AppTypeRegistry>();

    dbg!(registry
        .write()
        .get_type_info(std::any::TypeId::of::<ParticleEffect>()));
    for ty in registry.write().iter() {
        dbg!(ty);
    }
    let scene = DynamicScene::from_world(&world, registry);
    let serialized_scene = scene.serialize_ron(registry).unwrap();

    info!("{}", serialized_scene);

    #[cfg(not(target_arch = "wasm32"))]
    IoTaskPool::get()
        .spawn(async move {
            File::create(format!("assets/test.ron"))
                .and_then(|mut file| file.write(serialized_scene.as_bytes()))
                .expect("Error while writing scene to file");
        })
        .detach();
}
