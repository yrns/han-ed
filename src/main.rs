pub mod asset;
pub mod change;
pub mod gradient;
pub mod reffect;

use std::{
    any::Any,
    borrow::Cow,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use asset::*;

use anyhow::Result;
use bevy::{
    core_pipeline::bloom::BloomSettings,
    log::LogPlugin,
    prelude::*,
    render::{render_resource::WgpuFeatures, settings::WgpuSettings, RenderPlugin},
    tasks::IoTaskPool,
};
use bevy_egui::{
    egui::{self, widgets::DragValue, CollapsingHeader},
    EguiContexts, EguiPlugin,
};
use bevy_hanabi::prelude::*;

use crate::change::*;
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
        hl!($label, $ui, |ui| ui_value(
            id,
            &mut $value,
            $suffix,
            ui,
            value_f32
        ))
    }};
}

// So we don't have to explicitly set the type for body in hl!
#[doc(hidden)]
#[inline]
fn __contents<R: Into<Change>>(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui) -> R) -> Change {
    f(ui).into()
}

/// Horizontal, with label.
macro_rules! hl {
    ($label:expr, $ui:ident, $body:expr) => {
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
                    filter: "bevy_hanabi=warn,han-ed=debug".to_string(),
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
                    if ui.button("New").clicked() {
                        // Add a new default effect.
                    }

                    ui.add_enabled_ui(false, |ui| {
                        if ui.button("Random").clicked() {
                            // TODO spawn random
                        }
                    });
                });
                ui.separator();

                for (root_path, path, handle, saved) in reffect_paths.iter_mut() {
                    match handle {
                        Some(handle) => match reffects.get_mut(&handle) {
                            Some(re) => {
                                let live_entity = live_effect(&handle);

                                let mut re_changed = false;

                                let effect_header = match path.file_name() {
                                    Some(_) => format!("{}: ({})", re.name, path.display()),
                                    None => re.name.to_owned(),
                                };

                                CollapsingHeader::new(effect_header)
                                    .default_open(true)
                                    // If we don't set the source, it uses the header text, which potentially changes.
                                    .id_source(&handle)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("Name");
                                            re_changed |= ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut re.name)
                                                        .id_source("name"),
                                                )
                                                .changed();

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

                                            // Move to AssetPaths?
                                            // TODO confirm overwrite if the name has changed
                                            #[cfg(not(target_arch = "wasm32"))]
                                            if ui
                                                .add_enabled(!*saved, egui::Button::new("Save"))
                                                .clicked()
                                            {
                                                // Clone some things so they can be processed in a different thread.
                                                match save_effect(
                                                    re.clone(),
                                                    (root_path, path),
                                                    type_registry.clone(),
                                                    &asset_server,
                                                ) {
                                                    Ok(_) => *saved = true,
                                                    // This does not capture all the errors - in
                                                    // order to get the other ones we'd have to use
                                                    // a channel or an event.
                                                    Err(e) => {
                                                        error!("error saving: {:?}", e)
                                                    }
                                                }
                                            }

                                            // TODO
                                            _ = ui.add_enabled(false, egui::Button::new("Clone"));
                                            _ = ui.add_enabled(false, egui::Button::new("🗙"));
                                        });

                                        _ = edit_path(path, ui, |path| {
                                            validate_path(path, "han", root_path)
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
                                                ) | ui_option_reflect(
                                                    "Velocity",
                                                    &mut re.init_velocity,
                                                    &mut env,
                                                    ui,
                                                ) | ui_option_reflect(
                                                    "Size",
                                                    &mut re.init_size,
                                                    &mut env,
                                                    ui,
                                                ) | ui_option_reflect(
                                                    "Age",
                                                    &mut re.init_age,
                                                    &mut env,
                                                    ui,
                                                ) | ui_init_lifetime(
                                                    &mut re.init_lifetime,
                                                    &mut env,
                                                    ui,
                                                )
                                            })
                                            | header!(ui, "Update Modifiers", |ui| {
                                                ui_option(
                                                    "Acceleration",
                                                    &mut re.update_accel,
                                                    ui,
                                                    ui_update_accel,
                                                ) | ui_option_reflect(
                                                    "Force Field",
                                                    &mut re.update_force_field,
                                                    &mut env,
                                                    ui,
                                                ) | ui_option_reflect(
                                                    "Linear Drag",
                                                    &mut re.update_linear_drag,
                                                    &mut env,
                                                    ui,
                                                ) | ui_option_reflect(
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
                                                ) | ui_option(
                                                    "Set Color",
                                                    &mut re.render_set_color,
                                                    ui,
                                                    ui_set_color,
                                                ) | ui_option(
                                                    "Color Over Lifetime",
                                                    &mut re.render_color_over_lifetime,
                                                    ui,
                                                    |g, ui| g.show(ui),
                                                ) | ui_option_reflect(
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
                                                    | ui_option_reflect(
                                                        "Orient Along Velocity",
                                                        &mut re.render_orient_along_velocity,
                                                        &mut env,
                                                        ui,
                                                    )
                                            }))
                                        .changed();
                                    });

                                if re_changed {
                                    *saved = false;

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
                            hl!(path.to_string_lossy(), ui, |ui| {
                                let response = ui.button("Load");
                                if response.clicked() {
                                    *handle = Some(asset_server.load(path.as_path()));
                                }
                                // impl Into<Change> for ()?
                                response
                            });
                        }
                    }
                }
            });
    });
}

fn ui_init_lifetime(
    v: &mut Option<InitLifetimeModifier>,
    env: &mut InspectorUi,
    ui: &mut egui::Ui,
) -> Change {
    ui.horizontal(|ui| {
        let change = ui_option_reflect("Lifetime", v, env, ui);

        if v.is_none() {
            ui.label("⚠").on_hover_text_at_pointer(
                "Effects require a lifetime unless provided via InitAttributeModifier.",
            );
        }

        change
    })
    .merge()
}

// Probably way easier to validate on save.
fn edit_path(
    path: &mut PathBuf,
    ui: &mut egui::Ui,
    validate: impl Fn(&str) -> Result<Cow<Path>>,
) -> Change {
    hl!("Path", ui, |ui| {
        // We have to edit as a string since PathBuf doesn't impl TextBuffer.
        let mut path_str = path.to_string_lossy().to_string();

        // id_source isn't necessary any more.
        let response = ui.add(egui::TextEdit::singleline(&mut path_str).id_source("path"));
        if response.gained_focus() {
            // Save a backup of the path in case validation fails.
            ui.memory_mut(|memory| memory.data.insert_temp::<PathBuf>(ui.id(), path.clone()));
        }

        // Require enter to validate and update path?
        //ui.input(|i| i.key_pressed(egui::Key::Enter))
        if response.lost_focus() {
            match validate(&path_str) {
                Ok(p) => {
                    match p {
                        Cow::Borrowed(_) => info!("path valid: {}", p.display()), // It's good as is.
                        Cow::Owned(p) => {
                            info!("path revised: {}", p.display());
                            *path = p;
                        }
                    }
                    return response;
                }

                Err(e) => error!("not a valid path: {:?}", e),
            }

            // Restore prior path.
            if let Some(p) = ui.memory_mut(|memory| memory.data.get_temp::<PathBuf>(ui.id())) {
                *path = p;
            }
        } else if response.changed() {
            *path = path_str.into();
        }

        response.into()
    })
}

fn short_circuit(
    _env: &mut InspectorUi,
    value: &mut dyn Reflect,
    ui: &mut egui::Ui,
    id: egui::Id,
    _options: &dyn Any,
) -> Option<bool> {
    if let Some(mut v) = value.downcast_mut::<Value<f32>>() {
        // Is this id unique enough?
        return Some(ui_value(id.with("valuef32"), &mut v, "", ui, value_f32).changed());
    }

    None
}

macro_rules! variant_label {
    ($ui:expr, $value:expr, $label:literal, $variant:pat, $default:expr) => {{
        let selected = matches!($value, $variant);
        let label = $ui.selectable_label(selected, $label);
        if label.clicked() && !selected {
            *$value = $default;
        }
        label
    }};
}

// Not recreating a reflective wheel...
fn ui_update_accel(accel: &mut UpdateAccel, ui: &mut egui::Ui) -> Change {
    egui::ComboBox::from_id_source(ui.id().with("update_accel"))
        .selected_text(match accel {
            UpdateAccel::Linear(_) => "Linear",
            UpdateAccel::Radial(_) => "Radial",
            UpdateAccel::Tangent(_) => "Tangent",
        })
        .show_ui(ui, |ui| {
            (variant_label!(
                ui,
                accel,
                "Linear",
                UpdateAccel::Linear(_),
                UpdateAccel::Linear(AccelModifier::constant(Vec3::ZERO))
            ) | variant_label!(
                ui,
                accel,
                "Radial",
                UpdateAccel::Radial(_),
                UpdateAccel::Radial(RadialAccelModifier::constant(Vec3::ZERO, 1.0))
            ) | variant_label!(
                ui,
                accel,
                "Tangent",
                UpdateAccel::Tangent(_),
                UpdateAccel::Tangent(TangentAccelModifier::constant(Vec3::ZERO, Vec3::Y, 1.0))
            ))
            .into()
        })
        .merge()
        | match accel {
            UpdateAccel::Linear(linear) => ui_linear_accel(linear, ui),
            UpdateAccel::Radial(radial) => ui_radial_accel(radial, ui),
            UpdateAccel::Tangent(tangent) => ui_tangent_accel(tangent, ui),
        }
}

fn ui_linear_accel(linear: &mut AccelModifier, ui: &mut egui::Ui) -> Change {
    match &mut linear.accel {
        ValueOrProperty::Value(graph::Value::Float3(v)) => value_vec3_single(v, "", ui),
        // ValueOrProperty::Property(_) => todo!(),
        // ValueOrProperty::ResolvedProperty(_) => todo!(),
        _ => ui_error(ui, "unhandled"),
    }
    .into()
}

fn ui_radial_accel(radial: &mut RadialAccelModifier, ui: &mut egui::Ui) -> Change {
    match &mut radial.accel {
        ValueOrProperty::Value(graph::Value::Float(v)) => {
            ui.add(drag_value(v, ""))
                | ui.label("Origin")
                | value_vec3_single(&mut radial.origin, "", ui)
        }
        _ => ui_error(ui, "unhandled"),
    }
    .into()
}

fn ui_tangent_accel(tangent: &mut TangentAccelModifier, ui: &mut egui::Ui) -> Change {
    match &mut tangent.accel {
        ValueOrProperty::Value(graph::Value::Float(v)) => {
            egui::Grid::new("tangent_accel")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Accel.");
                    let accel = ui.add(drag_value(v, ""));
                    ui.end_row();

                    ui.label("Origin");
                    let origin = value_vec3_single(&mut tangent.origin, "", ui);
                    ui.end_row();

                    ui.label("Axis");
                    let axis = value_vec3_single(&mut tangent.axis, "", ui);

                    accel | origin | axis
                })
                .inner
        }

        _ => ui_error(ui, "unhandled"),
    }
    .into()
}

fn ui_particle_texture(
    label: &str,
    data: &mut ParticleTexture,
    asset_server: &AssetServer,
    image_paths: &AssetPaths<Image>,
    ui: &mut egui::Ui,
) -> Change {
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
                for (path, handle, ..) in image_paths.paths.iter() {
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
                        return Some(resp.into());
                    }
                }

                None
            })
            .merge()
    })
    .inner
}

fn ui_option<T: Default>(
    label: &str,
    data: &mut Option<T>,
    ui: &mut egui::Ui,
    f: impl FnOnce(&mut T, &mut egui::Ui) -> Change,
) -> Change {
    ui.horizontal(|ui| {
        //ui.label(label);
        let mut opt = data.is_some();
        let mut response = ui.checkbox(&mut opt, label);
        if response.clicked() {
            *data = if opt { Some(T::default()) } else { None };
            response.mark_changed();
        };

        match data {
            Some(v) => f(v, ui) | response,
            None => response.into(),
        }
    })
    .inner
}

fn ui_reflect<T: Reflect>(
    label: &str,
    value: &mut T,
    env: &mut InspectorUi,
    ui: &mut egui::Ui,
    //options: &dyn Any
) -> Change {
    ui.horizontal(|ui| {
        ui.label(label);
        env.ui_for_reflect_with_options(value, ui, ui.id().with(label), &())
    })
    .inner
    .into()
}

fn ui_option_reflect<T: Reflect + Default>(
    label: &str,
    value: &mut Option<T>,
    env: &mut InspectorUi,
    ui: &mut egui::Ui,
    //options: &dyn Any
) -> Change {
    ui_option(label, value, ui, |value, ui| {
        env.ui_for_reflect_with_options(value, ui, ui.id().with(label), &())
            .into()
    })
    .into()
}

// Maybe infinite period should be a separate checkbox.
fn ui_spawner(spawner: &mut Spawner, ui: &mut egui::Ui) -> Change {
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
fn ui_value<T: FromReflect + Copy + Default, F>(
    id: egui::Id,
    value: &mut Value<T>,
    suffix: &str,
    ui: &mut egui::Ui,
    mut value_fn: F,
) -> Change
where
    F: FnMut(&mut Value<T>, &str, &mut egui::Ui) -> Change,
{
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
                let mut single = ui.selectable_label(matches!(value, Value::Single(_)), "Single");

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

                let mut uniform =
                    ui.selectable_label(matches!(value, Value::Uniform(_)), "Uniform");

                if uniform.clicked() {
                    match value {
                        Value::Single(v) => {
                            // An infinite uniform doensn't make much sense, nor an infinite
                            // color. Revisit this later.
                            *value = Value::Uniform((*v, *v));

                            // *value = if v.is_finite() {
                            //     Value::Uniform((*v, *v))
                            // } else {
                            //     // FIX this crashes w/o error if the effect is visible
                            //     Value::Uniform(Default::default())
                            // };
                            uniform.mark_changed();
                            return Some(uniform.into());
                        }
                        _ => (),
                    }
                }

                None
            })
            .merge()
            | value_fn(value, suffix, ui)
    })
    .inner
}

#[inline]
fn ui_error(ui: &mut egui::Ui, str: &str) -> egui::Response {
    ui.colored_label(ui.visuals().error_fg_color, str)
}

fn value_f32<'a>(value: &'a mut Value<f32>, suffix: &str, ui: &mut egui::Ui) -> Change {
    match value {
        Value::Single(v) => {
            let mut response = ui.add(drag_value(v, suffix));
            if suffix == "period" && response.clicked_by(egui::PointerButton::Secondary) {
                response.mark_changed();
                *v = f32::INFINITY;
            }
            response
        }
        Value::Uniform(v) => {
            ui.spacing_mut().item_spacing.x = 4.0; // default is 8.0?
            ui.add(drag_value(&mut v.0, suffix).clamp_range(0.0..=v.1))
                | ui.label("-")
                | ui.add(drag_value(&mut v.1, suffix).clamp_range(v.0..=f32::MAX))
        }
        _ => ui_error(ui, "unhandled value type"),
    }
    .into()
}

fn value_vec3_single(v: &mut Vec3, suffix: &str, ui: &mut egui::Ui) -> egui::Response {
    ui.add(drag_value(&mut v.x, suffix))
        | ui.add(drag_value(&mut v.y, suffix))
        | ui.add(drag_value(&mut v.z, suffix))
}

#[allow(unused)]
fn value_vec3<'a>(value: &'a mut Value<Vec3>, suffix: &str, ui: &mut egui::Ui) -> Change {
    match value {
        Value::Single(v) => value_vec3_single(v, suffix, ui),
        Value::Uniform((v0, v1)) => {
            ui.spacing_mut().item_spacing.x = 4.0; // default is 8.0?

            ui.add(drag_value(&mut v0.x, suffix).clamp_range(0.0..=v1.x))
                | ui.add(drag_value(&mut v0.y, suffix).clamp_range(0.0..=v1.y))
                | ui.add(drag_value(&mut v0.z, suffix).clamp_range(0.0..=v1.z))
                | ui.label("-")
                | ui.add(drag_value(&mut v1.x, suffix).clamp_range(v0.x..=f32::MAX))
                | ui.add(drag_value(&mut v1.y, suffix).clamp_range(v0.y..=f32::MAX))
                | ui.add(drag_value(&mut v1.z, suffix).clamp_range(v0.z..=f32::MAX))
        }
        _ => ui_error(ui, "unhandled value type"),
    }
    .into()
}

fn ui_set_color(color: &mut SetColorModifier, ui: &mut egui::Ui) -> Change {
    ui_value(
        ui.id().with("set_color"),
        &mut color.color,
        "",
        ui,
        value_color,
    )
}

fn color_edit_button(color: &mut Vec4, ui: &mut egui::Ui) -> bool {
    use egui::color_picker::*;

    let mut hsva = gradient::hsva(color);
    if color_edit_button_hsva(ui, &mut hsva, Alpha::OnlyBlend).changed() {
        *color = Vec4::from_slice(&hsva.to_rgba_premultiplied());
        true
    } else {
        false
    }
}

fn value_color<'a>(value: &'a mut Value<Vec4>, _suffix: &str, ui: &mut egui::Ui) -> Change {
    match value {
        Value::Single(v) => color_edit_button(v, ui).into(),
        Value::Uniform(v) => {
            ui.spacing_mut().item_spacing.x = 4.0; // default is 8.0?
            let c1 = color_edit_button(&mut v.0, ui);
            ui.label("-");
            let c2 = color_edit_button(&mut v.1, ui);
            (c1 || c2).into()
        }
        _ => ui
            .colored_label(ui.visuals().error_fg_color, "unhandled value type")
            .into(),
    }
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
