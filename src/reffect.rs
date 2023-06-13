use std::path::PathBuf;

use bevy::{prelude::*, reflect::TypeUuid};
//use bevy::reflect::*;
use crate::gradient::{ColorGradient, SizeGradient};
use bevy_hanabi::prelude::*;

// This is all to get around the fact that EffectAsset cannot be serialized.
#[derive(Default, Clone, TypeUuid, Reflect, FromReflect)]
#[uuid = "2933798f-a750-44c4-b7f9-0b7055368944"]
pub struct REffect {
    pub name: String,
    pub capacity: u32,
    pub spawner: Spawner,
    pub z_layer_2d: f32,
    pub simulation_space: SimulationSpace,
    pub simulation_condition: SimulationCondition,

    // skip properties for now...
    // skip motion_integration

    // InitModifier(s)
    pub init_position: InitPosition,
    pub init_velocity: Option<InitVelocity>,
    pub init_size: Option<InitSizeModifier>,
    pub init_age: Option<InitAgeModifier>,
    // So this is required unless lifetime is a property? Remove the Option?
    pub init_lifetime: Option<InitLifetimeModifier>,

    // UpdateModifiers(s)
    pub update_accel: Option<UpdateAccel>,
    pub update_force_field: Option<ForceFieldModifier>,
    pub update_linear_drag: Option<LinearDragModifier>,
    pub update_aabb_kill: Option<AabbKillModifier>,

    // RenderModifier(s)
    pub render_particle_texture: ParticleTexture,
    pub render_set_color: Option<SetColorModifier>,
    pub render_color_over_lifetime: Option<ColorGradient>,
    pub render_set_size: Option<SetSizeModifier>,
    pub render_size_over_lifetime: Option<SizeGradient>,
    pub render_billboard: bool,
    pub render_orient_along_velocity: Option<OrientAlongVelocityModifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, FromReflect)]
pub enum InitPosition {
    Circle(InitPositionCircleModifier),
    Sphere(InitPositionSphereModifier),
    Cone(InitPositionCone3dModifier),
}

impl Default for InitPosition {
    fn default() -> Self {
        Self::Circle(InitPositionCircleModifier {
            axis: Vec3::Z,
            radius: 1.0,
            ..default()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect, FromReflect)]
pub enum InitVelocity {
    Circle(InitVelocityCircleModifier),
    Sphere(InitVelocitySphereModifier),
    Cone(InitVelocityTangentModifier),
}

impl Default for InitVelocity {
    fn default() -> Self {
        Self::Circle(InitVelocityCircleModifier {
            axis: Vec3::Z,
            speed: 1.0.into(),
            ..default()
        })
    }
}

#[derive(Debug, Clone, PartialEq, Reflect, FromReflect)]
pub enum UpdateAccel {
    Linear(AccelModifier),
    Radial(RadialAccelModifier),
    Tangent(TangentAccelModifier),
}

/// Unfortunately, AFAIK, Bevy does not resolve sub-assets referenced in assets serialized via
/// reflection. It serializes the textures as weak handles which have some correspondence to the
/// actual asset, but it order to check (compare ids), we'd have to load all the textures in the
/// asset directory. So instead we serialize the path and swap it out in the asset loader.
#[derive(Debug, Default, Clone, PartialEq, Reflect, FromReflect)]
pub enum ParticleTexture {
    #[default]
    None,
    Path(PathBuf),
    Texture(Handle<Image>),
}

impl From<Handle<Image>> for ParticleTexture {
    fn from(handle: Handle<Image>) -> Self {
        Self::Texture(handle)
    }
}

impl ParticleTexture {
    /// Return a handle to the texture if it exists.
    pub fn handle(&self) -> Option<&Handle<Image>> {
        match self {
            ParticleTexture::None => None,
            ParticleTexture::Path(path) => {
                error!(
                    "texture path for loaded effect asset should not happen: {}",
                    path.display()
                );
                None
            }
            ParticleTexture::Texture(handle) => Some(handle),
        }
    }
}

impl Default for UpdateAccel {
    fn default() -> Self {
        Self::Linear(AccelModifier::constant(Vec3::Z))
    }
}

impl REffect {
    // We need to asset server to load the texture.
    pub fn to_effect_asset(&self, _asset_server: &AssetServer) -> EffectAsset {
        let mut effect = EffectAsset {
            name: self.name.clone(),
            capacity: self.capacity,
            spawner: self.spawner,
            z_layer_2d: self.z_layer_2d,
            modifiers: vec![match self.init_position {
                InitPosition::Circle(m) => m.boxed_clone(),
                InitPosition::Sphere(m) => m.boxed_clone(),
                InitPosition::Cone(m) => m.boxed_clone(),
            }],
            simulation_space: self.simulation_space,
            simulation_condition: self.simulation_condition,

            ..default()
        };

        if let Some(m) = self.init_velocity.as_ref() {
            match m {
                InitVelocity::Circle(m) => effect = effect.init(m.clone()),
                InitVelocity::Sphere(m) => effect = effect.init(m.clone()),
                InitVelocity::Cone(m) => effect = effect.init(m.clone()),
            };
        }

        if let Some(m) = self.init_size.as_ref() {
            effect = effect.init(m.clone());
        }

        if let Some(m) = self.init_age.as_ref() {
            effect = effect.init(m.clone());
        }

        if let Some(m) = self.init_lifetime.as_ref() {
            effect = effect.init(m.clone());
        }

        if let Some(m) = self.update_accel.as_ref() {
            match m {
                UpdateAccel::Linear(m) => effect = effect.update(m.clone()),
                UpdateAccel::Radial(m) => effect = effect.update(m.clone()),
                UpdateAccel::Tangent(m) => effect = effect.update(m.clone()),
            };
        }

        if let Some(m) = self.update_force_field.as_ref() {
            effect = effect.update(m.clone());
        }

        if let Some(m) = self.update_linear_drag.as_ref() {
            effect = effect.update(m.clone());
        }

        if let Some(m) = self.update_aabb_kill.as_ref() {
            effect = effect.update(m.clone());
        }

        match self.render_particle_texture {
            ParticleTexture::Path(ref path) => {
                // This should never happen since the texture is loaded when the asset is loaded.
                error!("particle texture not loaded: {}", path.display())
            }
            ParticleTexture::Texture(ref handle) => {
                effect = effect.render(ParticleTextureModifier {
                    texture: handle.clone(),
                });
            }
            _ => (),
        }

        if let Some(m) = self.render_set_color.as_ref() {
            effect = effect.render(m.clone());
        }
        if let Some(m) = self.render_color_over_lifetime.as_ref() {
            effect = effect.render(ColorOverLifetimeModifier::from(m.clone()))
        }
        if let Some(m) = self.render_set_size.as_ref() {
            effect = effect.render(m.clone());
        }
        if let Some(m) = self.render_size_over_lifetime.as_ref() {
            effect = effect.render(SizeOverLifetimeModifier::from(m.clone()));
        }
        if self.render_billboard {
            effect = effect.render(BillboardModifier);
        }
        if let Some(m) = self.render_orient_along_velocity.as_ref() {
            effect = effect.render(m.clone());
        }

        effect
    }
}
