pub mod asset;
pub mod reffect;

use std::{fs::File, io::Write, mem::discriminant};

use asset::*;

use bevy::{
    core_pipeline::bloom::BloomSettings,
    log::LogPlugin,
    prelude::*,
    reflect::serde::ReflectSerializer,
    render::{render_resource::WgpuFeatures, settings::WgpuSettings, RenderPlugin},
    tasks::IoTaskPool,
};
use bevy_egui::{
    egui::{self, widgets::DragValue, CollapsingHeader},
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
        .register_type::<InitPosition>()
        .register_type::<InitVelocity>()
        .register_type::<Option<InitVelocity>>()
        .register_type::<UpdateAccel>()
        .register_type::<Option<UpdateAccel>>()
        //.register_type::<REffect>() add_asset::<T> registers Handle<T>
        .add_asset::<REffect>()
        .register_asset_reflect::<REffect>()
        .init_asset_loader::<asset::HanLoader>()
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
    type_registry: Res<AppTypeRegistry>,
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
                        DragValue::new(&mut bloom.intensity)
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
                                CollapsingHeader::new(path.to_string_lossy())
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("Name");
                                            ui.text_edit_singleline(&mut re.name);

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
                                            // only enable if there are unsaved changes
                                            if ui.button("Save").clicked() {
                                                #[cfg(not(target_arch = "wasm32"))]
                                                let file_name = format!("assets/{}.han", re.name);
                                                // Clone so that we can serialize in a different thread?
                                                let effect = re.clone();
                                                let tr = type_registry.clone(); // Arc
                                                IoTaskPool::get()
                                                    .spawn(async move {
                                                        let tr = tr.read();
                                                        let rs =
                                                            ReflectSerializer::new(&effect, &tr);
                                                        let ron = ron::ser::to_string_pretty(
                                                            &rs,
                                                            ron::ser::PrettyConfig::new(),
                                                        )
                                                        .unwrap();
                                                        //let ron = serialize_ron(effect).unwrap();
                                                        File::create(file_name)
                                                            .and_then(|mut file| {
                                                                file.write(ron.as_bytes())
                                                            })
                                                            .map_err(|e| error!("{}", e))
                                                    })
                                                    .detach();
                                            }

                                            // TODO
                                            _ = ui.button("Clone");
                                            _ = ui.button("-");
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label("Capacity");
                                            ui.add(DragValue::new(&mut re.capacity));
                                        });

                                        ui_spawner(&mut re.spawner, ui);
                                    });
                            }
                            None => {
                                ui.spinner(); // loading still
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

fn ui_spawner(spawner: &mut Spawner, ui: &mut egui::Ui) -> egui::Response {
    CollapsingHeader::new("Spawner")
        .default_open(true)
        .show(ui, |ui| {
            ui_value("Particles", &mut spawner.num_particles, "", ui)
                | ui_value("Spawn Time", &mut spawner.spawn_time, "s", ui)
                | ui_value("Period", &mut spawner.period, "s", ui)
                | ui.checkbox(&mut spawner.starts_active, "Starts Active")
                | ui.checkbox(&mut spawner.starts_immediately, "Starts Immediately")
        })
        .body_response
        .unwrap()
}

// TODO hover descriptions
// TODO left-click 0, right-click INF?
fn ui_value(name: &str, value: &mut Value<f32>, suffix: &str, ui: &mut egui::Ui) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(name);

        let cb = egui::ComboBox::from_id_source(ui.id().with(name))
            .selected_text(match value {
                Value::Single(_) => "Single",
                Value::Uniform(_) => "Uniform",
                _ => "Unhandled",
            })
            .show_ui(ui, |ui| {
                let mut single = ui.selectable_label(
                    discriminant(value) == discriminant(&Value::Single(0.0)),
                    "Single",
                );

                if single.clicked() {
                    match value {
                        Value::Uniform((v, _)) => {
                            *value = Value::Single(*v);
                            single.mark_changed();
                        }
                        _ => (),
                    }
                }

                let mut uniform = ui.selectable_label(
                    discriminant(value) == discriminant(&Value::Uniform((0.0, 0.0))),
                    "Uniform",
                );

                if uniform.clicked() {
                    match value {
                        Value::Single(v) => {
                            *value = Value::Uniform((*v, *v));
                            uniform.mark_changed();
                        }
                        _ => (),
                    }
                }

                single | uniform
            })
            .response;

        if cb.changed {
            dbg!(cb.changed);
        }

        cb | match value {
            Value::Single(ref mut v) => ui.add(DragValue::new(v).suffix(suffix)),
            Value::Uniform(v) => {
                ui.add(DragValue::new(&mut v.0).clamp_range(0.0..=v.1))
                    | ui.label("-")
                    | ui.add(
                        DragValue::new(&mut v.1)
                            .clamp_range(v.0..=f32::MAX)
                            .suffix(suffix),
                    )
            }
            _ => ui.colored_label(ui.visuals().error_fg_color, "unhandled value type"),
        }
    })
    .response
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