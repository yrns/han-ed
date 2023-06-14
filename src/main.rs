pub mod asset;
mod gradient;
pub mod reffect;

use std::{any::Any, fs::File, io::Write, mem::discriminant};

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

use bevy_inspector_egui::{reflect_inspector::*, DefaultInspectorConfigPlugin};
use gradient::{ColorGradient, Gradient, SizeGradient};
use reffect::*;

/// Collapsing header and body.
macro_rules! header {
    ($ui:ident, $label:literal, $body:expr) => {{
        CollapsingHeader::new($label)
            .default_open(true)
            .show($ui, $body)
            .merge()
    }};
}

/// Label and value.
macro_rules! value {
    ($label:literal, $ui:ident, $value:expr, $suffix:literal) => {{
        let id = $ui.id().with($label);
        hl!($label, $ui, |ui| ui_value(id, &mut $value, $suffix, ui))
    }};
}

// So we don't have to explicitly set the type for body in hl!
#[doc(hidden)]
fn __contents(
    ui: &mut egui::Ui,
    f: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    f(ui)
}

/// Horizontal, with label.
macro_rules! hl {
    ($label:literal, $ui:ident, $body:expr) => {
        $ui.horizontal(|ui| {
            ui.label($label);
            __contents(ui, $body)
        })
        .inner
    };
}

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
        .register_type::<ColorGradient>()
        .register_type::<Option<ColorGradient>>()
        .register_type::<Vec<(f32, Vec4)>>()
        .register_type::<(f32, Vec4)>()
        .register_type::<SizeGradient>()
        .register_type::<Option<SizeGradient>>()
        .register_type::<Vec<(f32, Vec2)>>()
        .register_type::<(f32, Vec2)>()
        .register_type::<ParticleTexture>()
        .register_type::<Option<UpdateAccel>>()
        //.register_type::<REffect>() add_asset::<T> registers Handle<T>
        .add_asset::<REffect>()
        .register_asset_reflect::<REffect>()
        .init_asset_loader::<asset::HanLoader>()
        .insert_resource(AssetPaths::<REffect>::new("han"))
        .insert_resource(AssetPaths::<Image>::new("png"))
        .add_plugin(EguiPlugin)
        .add_plugin(DefaultInspectorConfigPlugin)
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
    // if let Ok(assets) = asset_server.load_folder(".") {
    //     dbg!(assets.len());
    // }

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
    image_paths: ResMut<AssetPaths<Image>>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut reffects: ResMut<Assets<REffect>>,
    mut live_effects: Query<(
        Entity,
        &Name,
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

    let window = egui::Window::new("han-ed").vscroll(true);
    window.show(contexts.ctx_mut(), |ui| {
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

                // TODO add more tooltips
                let mut show_tooltips = ui.ctx().style().explanation_tooltips;
                if ui.checkbox(&mut show_tooltips, "Show tooltips").changed() {
                    let mut style = (*ui.ctx().style()).clone();
                    style.explanation_tooltips = show_tooltips;
                    ui.ctx().set_style(style);
                }

                let mut debug = ui.ctx().debug_on_hover();
                if ui.checkbox(&mut debug, "Debug").changed() {
                    ui.ctx().set_debug_on_hover(debug);
                }
            });

        // We want to keep this around so that we can package these live effects into a scene later?
        CollapsingHeader::new("Live")
            .default_open(true)
            .show(ui, |ui| {
                for (entity, name, mut spawner, _effect, _live_effect) in live_effects.iter_mut() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} ({:?}): active: {} particles: {}",
                            name,
                            entity,
                            spawner.is_active(),
                            spawner.spawn_count(),
                        ));
                        if ui.button("Reset").clicked() {
                            spawner.reset();
                        }
                        if ui.small_button("🗙").clicked() {
                            commands.get_entity(entity).unwrap().despawn();
                        }
                    });
                }
            });

        // Find the live entity that corresponds to this REffect handle.
        let live_effect = |h: &Handle<REffect>| {
            live_effects
                .iter()
                .find_map(|(entity, _, _, _, e)| (&e.0 == h).then_some(entity))
        };

        CollapsingHeader::new("Effects")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // TODO
                    ui.add_enabled_ui(false, |ui| {
                        if ui.button("New").clicked() {
                            // spawn new
                        }
                        if ui.button("Random").clicked() {
                            // spawn random
                        }
                    });
                });
                ui.separator();

                // duplicate, remove?, does rename work?
                for (path, handle) in reffect_paths.paths.iter_mut() {
                    match handle {
                        Some(handle) => match reffects.get_mut(&handle) {
                            Some(re) => {
                                let live_entity = live_effect(&handle);

                                let mut re_changed = false;

                                CollapsingHeader::new(path.to_string_lossy())
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("Name");
                                            ui.text_edit_singleline(&mut re.name);

                                            if let Some(entity) = live_entity {
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
                                                        Name::new(re.name.clone()),
                                                    ));
                                                }
                                            }

                                            // TODO confirm overwrite if the name has changed
                                            // only enable if there are unsaved changes
                                            if ui.button("Save").clicked() {
                                                #[cfg(not(target_arch = "wasm32"))]
                                                let file_name = format!("assets/{}.han", re.name);

                                                // Clone so that we can serialize in a different
                                                // thread? Also, convert texture to asset path:
                                                let mut effect = re.clone();
                                                match &mut effect.render_particle_texture {
                                                    ParticleTexture::Texture(handle) => {
                                                        if let Some(path) = asset_server
                                                            .get_handle_path(handle.id())
                                                        {
                                                            effect.render_particle_texture =
                                                                ParticleTexture::Path(
                                                                    path.path().to_path_buf(),
                                                                );
                                                        }
                                                    }
                                                    _ => (),
                                                }

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
                                            _ = ui.add_enabled(false, egui::Button::new("Clone"));
                                            _ = ui.add_enabled(false, egui::Button::new("🗙"));
                                        });

                                        // Set up context for reflect values.
                                        let mut cx = Context::default();
                                        let tr = type_registry.read();
                                        let mut env = InspectorUi::new(
                                            &tr,
                                            &mut cx,
                                            Some(short_circuit),
                                            None,
                                            None,
                                        );

                                        re_changed |= (hl!("Capacity", ui, |ui| ui
                                            .add(DragValue::new(&mut re.capacity)))
                                            | ui_spawner(&mut re.spawner, ui)
                                            | ui_reflect(
                                                "Simulation Space",
                                                &mut re.simulation_space,
                                                &mut env,
                                                ui,
                                            )
                                            | ui_reflect(
                                                "Simulation Condition",
                                                &mut re.simulation_condition,
                                                &mut env,
                                                ui,
                                            )
                                            | header!(ui, "Initial Modifiers", |ui| {
                                                ui_reflect(
                                                    "Position",
                                                    &mut re.init_position,
                                                    &mut env,
                                                    ui,
                                                ) | ui_reflect(
                                                    "Velocity",
                                                    &mut re.init_velocity,
                                                    &mut env,
                                                    ui,
                                                ) | ui_reflect(
                                                    "Size",
                                                    &mut re.init_size,
                                                    &mut env,
                                                    ui,
                                                ) | ui_reflect(
                                                    "Age",
                                                    &mut re.init_age,
                                                    &mut env,
                                                    ui,
                                                ) | ui_reflect(
                                                    "Lifetime",
                                                    &mut re.init_lifetime,
                                                    &mut env,
                                                    ui,
                                                )
                                            })
                                            | header!(ui, "Update Modifiers", |ui| {
                                                ui_reflect(
                                                    "Acceleration",
                                                    &mut re.update_accel,
                                                    &mut env,
                                                    ui,
                                                ) | ui_reflect(
                                                    "Force Field",
                                                    &mut re.update_force_field,
                                                    &mut env,
                                                    ui,
                                                ) | ui_reflect(
                                                    "Linear Drag",
                                                    &mut re.update_linear_drag,
                                                    &mut env,
                                                    ui,
                                                ) | ui_reflect(
                                                    "AABB Kill",
                                                    &mut re.update_aabb_kill,
                                                    &mut env,
                                                    ui,
                                                )
                                            })
                                            | header!(ui, "Render Modifiers", |ui| {
                                                ui_particle_texture(
                                                    "Particle Texture",
                                                    &mut re.render_particle_texture,
                                                    &asset_server,
                                                    &image_paths,
                                                    ui,
                                                ) | ui_reflect(
                                                    "Set Color",
                                                    &mut re.render_set_color,
                                                    &mut env,
                                                    ui,
                                                ) | ui_option(
                                                    "Color Over Lifetime",
                                                    &mut re.render_color_over_lifetime,
                                                    ui,
                                                    |g, ui| g.show(ui),
                                                ) | ui_reflect(
                                                    "Set Size",
                                                    &mut re.render_set_size,
                                                    &mut env,
                                                    ui,
                                                ) | ui_option(
                                                    "Size Over Lifetime",
                                                    &mut re.render_size_over_lifetime,
                                                    ui,
                                                    |g, ui| g.show(ui),
                                                ) | ui
                                                    .checkbox(&mut re.render_billboard, "Billboard")
                                                    | ui_reflect(
                                                        "Orient Along Velocity",
                                                        &mut re.render_orient_along_velocity,
                                                        &mut env,
                                                        ui,
                                                    )
                                            }))
                                        .changed;
                                    });

                                if re_changed {
                                    // Regenerate (if live).
                                    if let Some(entity) = live_entity {
                                        // This is just hide/show. Can we swap something inside the
                                        // bundle instead?
                                        commands.get_entity(entity).unwrap().despawn();

                                        commands.spawn((
                                            ParticleEffectBundle::new(
                                                effects.add(re.to_effect_asset(&asset_server)),
                                            ),
                                            LiveEffect(handle.clone()),
                                            Name::new(re.name.clone()),
                                        ));
                                    }
                                }
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

trait Merge {
    fn merge(self) -> egui::Response;
}

impl Merge for egui::InnerResponse<egui::Response> {
    fn merge(self) -> egui::Response {
        self.inner | self.response
    }
}

// For ComboBox, we only return the item response that's changed, or the header when closed.
impl Merge for egui::InnerResponse<Option<Option<egui::Response>>> {
    fn merge(self) -> egui::Response {
        self.inner.flatten().unwrap_or(self.response)
    }
}

// Return the inner response or the header when closed. We don't want the body response since it
// will never be marked changed.
impl Merge for egui::containers::CollapsingResponse<egui::Response> {
    fn merge(self) -> egui::Response {
        //self.body_response.unwrap_or(self.header_response)
        self.body_returned.unwrap_or(self.header_response)
    }
}

// #[inline]
// fn merge(ir: egui::InnerResponse<egui::Response>) -> egui::Response {
//     ir.response | ir.inner
// }

fn short_circuit(
    _env: &mut InspectorUi,
    value: &mut dyn Reflect,
    ui: &mut egui::Ui,
    id: egui::Id,
    _options: &dyn Any,
) -> Option<bool> {
    if let Some(mut v) = value.downcast_mut::<Value<f32>>() {
        // Is this id unique enough?
        return Some(ui_value(id.with("valuef32"), &mut v, "", ui).changed);
    }

    None
}

fn ui_particle_texture(
    label: &str,
    data: &mut ParticleTexture,
    asset_server: &AssetServer,
    image_paths: &AssetPaths<Image>,
    ui: &mut egui::Ui,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);

        // In the loop below we already have the path, but here we have to fetch it from assets for
        // the selected texture (if any).
        let selected = match data.handle() {
            Some(handle) => asset_server
                .get_handle_path(handle.id())
                .map(|asset_path| {
                    let path = asset_path.path().display();
                    match asset_path.label() {
                        // Is there ever a label?
                        Some(label) => format!("{} ({})", path, label),
                        None => format!("{}", path),
                    }
                })
                .unwrap_or_else(|| "??? (no path for asset handle)".to_string()),
            None => "None".into(),
        };

        egui::ComboBox::from_id_source(ui.id().with(label))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                // None is the first option.
                let none = ui.selectable_value(data, ParticleTexture::None, "None");
                if none.changed {
                    return Some(none);
                }

                // We need to filter out textures that don't work for effects like D3 textures.
                //for (id, _image) in (*images).iter() {
                for (path, handle) in image_paths.paths.iter() {
                    // Can an effect point to an unloaded image?
                    let checked = handle
                        .as_ref()
                        .zip(data.handle())
                        .map(|(a, b)| a == b)
                        .unwrap_or_default();

                    // Show thumbnails?
                    let mut resp = ui.selectable_label(checked, format!("{}", path.display()));

                    if resp.clicked() && !checked {
                        // Is this really be the only way to make a strong handle from an id?
                        // let mut texture = Handle::weak(id);
                        // texture.make_strong(&*images);
                        let texture = match handle {
                            Some(h) => h.clone(),
                            None => asset_server.load(path.as_path()),
                        };

                        *data = ParticleTexture::Texture(texture);
                        resp.mark_changed();
                        return Some(resp);
                    }
                }

                None
            })
            .merge()
    })
    .inner
}

#[allow(unused)]
fn ui_option<T: Default>(
    label: &str,
    data: &mut Option<T>,
    ui: &mut egui::Ui,
    f: impl FnOnce(&mut T, &mut egui::Ui) -> egui::Response,
) -> egui::Response {
    ui.horizontal(|ui| {
        //ui.label(label);
        let mut opt = data.is_some();
        let mut response = ui.checkbox(&mut opt, label);
        if response.clicked() {
            *data = if opt { Some(T::default()) } else { None };
            response.mark_changed();
        };

        match data {
            Some(v) => response | f(v, ui),
            None => response,
        }
    })
    .inner
}

fn ui_reflect<T: Reflect>(
    label: &str,
    data: &mut T,
    env: &mut InspectorUi,
    ui: &mut egui::Ui,
    //options: &dyn Any
) -> egui::Response {
    let mut ir = ui.horizontal(|ui| {
        ui.label(label);
        env.ui_for_reflect_with_options(data, ui, ui.id().with(label), &())
    });
    if ir.inner {
        ir.response.mark_changed()
    }
    ir.response
}

// Maybe infinite period should be a separate checkbox.
fn ui_spawner(spawner: &mut Spawner, ui: &mut egui::Ui) -> egui::Response {
    header!(ui, "Spawner", |ui| {
        value!("Particles", ui, spawner.num_particles, "#")
            | value!("Spawn Time", ui, spawner.spawn_time, "s")
            | value!("Period", ui, spawner.period, "period")
            | ui.checkbox(&mut spawner.starts_active, "Starts Active")
            | ui.checkbox(&mut spawner.starts_immediately, "Starts Immediately")
    })
}

// Configure DragValue based on suffix for now.
fn drag_value<'a>(v: &'a mut f32, suffix: &str) -> DragValue<'a> {
    let fin = if v.is_finite() { "s" } else { "" };
    let dv = DragValue::new(v);
    match suffix {
        // Count.
        "#" => dv.clamp_range(0..=u32::MAX),
        // Seconds.
        "s" => dv.speed(0.01).clamp_range(0.0..=f32::MAX).suffix(suffix),
        // Period (seconds).
        "period" => dv.speed(0.01).clamp_range(0.0..=f32::INFINITY).suffix(fin),
        // ?
        _ => dv.speed(0.1).suffix(suffix),
    }
}

// Values are all different units (time, distance, velocity, acceleration). It would be nice if we
// could tune the DragValues for each case (and suffix). Also, hover information from the doc
// strings would be nice. Maybe this information could be encoded statically in the modifiers.
// TODO infinity for period
fn ui_value(
    id: egui::Id,
    value: &mut Value<f32>,
    suffix: &str,
    ui: &mut egui::Ui,
) -> egui::Response {
    // The horizontal is needed for when this is used within a reflect value. The reflect ui adds
    // some odd spacing.
    ui.horizontal(|ui| {
        // The combo box label is on the right so we never use it, but we need the label for the
        // unique id. (We could also use a label for units.)
        egui::ComboBox::from_id_source(id)
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
                            return Some(single);
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
                            *value = if v.is_finite() {
                                Value::Uniform((*v, *v))
                            } else {
                                // FIX this crashes w/o error if the effect is visible
                                Value::Uniform((0.0, 0.0))
                            };
                            uniform.mark_changed();
                            return Some(uniform);
                        }
                        _ => (),
                    }
                }

                None
            })
            .merge()
            | match value {
                Value::Single(ref mut v) => {
                    let mut dv = ui.add(drag_value(v, suffix));
                    if suffix == "period" && dv.clicked_by(egui::PointerButton::Secondary) {
                        dv.mark_changed();
                        *v = f32::INFINITY;
                    }
                    dv
                }
                Value::Uniform(v) => {
                    ui.spacing_mut().item_spacing.x = 4.0; // default is 8.0?
                    ui.add(drag_value(&mut v.0, suffix).clamp_range(0.0..=v.1))
                        | ui.label("-")
                        | ui.add(drag_value(&mut v.1, suffix).clamp_range(v.0..=f32::MAX))
                }
                _ => ui.colored_label(ui.visuals().error_fg_color, "unhandled value type"),
            }
    })
    .inner
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
