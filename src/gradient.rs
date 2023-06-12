use std::cmp::Ordering;

use bevy::{
    prelude::Vec4,
    reflect::{FromReflect, Reflect},
};
use bevy_egui::egui::{widgets::color_picker::*, *};
use bevy_hanabi::{ColorOverLifetimeModifier, Gradient};

#[derive(Clone, Reflect, FromReflect)]
pub struct ColorGradient {
    keys: Vec<(f32, Vec4)>,
}

impl Default for ColorGradient {
    fn default() -> Self {
        Self {
            keys: vec![(0.5, Vec4::splat(1.0))],
        }
    }
}

impl ColorGradient {
    // The starting color is the first key (if non-zero) or the last zero-value key.
    fn initial_color(&self) -> Color32 {
        if self.keys[0].0 > 0.0 {
            rgba(&self.keys[0].1).into()
        } else if let Some((_k, color)) = self.keys.iter().take_while(|k| k.0 == 0.0).last() {
            rgba(color).into()
        } else {
            Color32::TEMPORARY_COLOR
        }
    }
}

// This assumes keys are sorted and there's at least one.
pub fn color_gradient(gradient: &mut ColorGradient, ui: &mut Ui) -> Response {
    color_gradient_picker(gradient, ui) | color_pickers(gradient, ui)
}

pub fn color_gradient_picker(gradient: &mut ColorGradient, ui: &mut Ui) -> Response {
    let desired_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
    let (rect, mut response) = ui.allocate_at_least(desired_size, Sense::hover());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let w = rect.width();

        let mut mesh = start_strip(rect, gradient.initial_color());

        let keys = &mut gradient.keys;
        assert!(keys.len() > 0);

        let mut last_k = 0.0;
        for (key, color) in keys.iter_mut().skip_while(|(k, _)| *k == 0.0) {
            add_segment(
                &mut mesh,
                (key.min(1.0) - last_k) * w,
                Some(rgba(color).into()),
            );
            last_k = *key;
        }
        if last_k < 1.0 {
            add_segment(&mut mesh, (1.0 - last_k) * w, None);
        }

        ui.painter().add(Shape::mesh(mesh));
        ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke);

        // Add draggable keys. The scope is to paper over the layered space allocations. Following
        // widgets will get placed after the last (inset) allocation without it.
        if ui
            .scope(|ui| {
                let mut sort = false;
                let mut changed = false;
                for i in 0..keys.len() {
                    let (key, color) = &mut keys[i];
                    let re = ui.allocate_rect(
                        Rect::from_center_size(
                            pos2(lerp(rect.x_range(), *key), rect.center().y),
                            Vec2::splat(rect.height() / 2.0),
                        ),
                        Sense::click_and_drag(),
                    );
                    let visuals = ui.style().interact(&re);
                    ui.painter().add(epaint::CircleShape {
                        center: re.rect.center(),
                        radius: re.rect.size().x / 2.0,
                        fill: rgba(color).into(),
                        stroke: visuals.fg_stroke,
                    });
                    if re.clicked_by(PointerButton::Secondary) {
                        // Delete the key.
                        keys.remove(i);
                        changed = true;
                        break;
                    } else if re.dragged() {
                        // In this one particular case we don't register the change until release, I
                        // suppose because you can see the color already.
                        if let Some(p) = ui.ctx().pointer_interact_pos() {
                            let x = (p - rect.min).x / rect.width();
                            *key = x.clamp(0.0, 1.0);
                        }
                    } else if re.drag_released() {
                        // Don't sort until the drag is released otherwise it starts
                        // flickering. Probably because the ids get swapped?
                        sort = true;
                    }
                }

                if sort {
                    keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
                }
                sort || changed
            })
            .inner
        {
            response.mark_changed();
        }
    }

    response
}

fn color_pickers(gradient: &mut ColorGradient, ui: &mut Ui) -> Response {
    let keys = &mut gradient.keys;

    let mut changed = false;

    let mut response = ui
        .horizontal(|ui| {
            // Make the buttons smaller.
            ui.spacing_mut().interact_size = Vec2::splat(12.0);

            for (_key, color) in keys.iter_mut() {
                let mut color32 = rgba(color).into();
                if color_edit_button_srgba(ui, &mut color32, Alpha::Opaque).changed() {
                    *color = Vec4::from_slice(&Rgba::from(color32).to_array());
                    changed = true;
                }
            }

            if ui.small_button("+").clicked() {
                keys.push((1.0, Vec4::ZERO));
            }
        })
        .response;

    if changed {
        response.mark_changed();
    }

    response
}

impl From<ColorGradient> for ColorOverLifetimeModifier {
    fn from(keys: ColorGradient) -> Self {
        let mut gradient = Gradient::new();
        for (key, color) in keys.keys {
            gradient.add_key(key, color);
        }

        ColorOverLifetimeModifier { gradient }
    }
}

fn rgba(c: &Vec4) -> Rgba {
    Rgba::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

// Start a strip with two vertices.
fn start_strip(rect: Rect, color: Color32) -> Mesh {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.min, color);
    mesh.colored_vertex(rect.min + vec2(0.0, rect.height()), color);
    mesh
}

// Add two vertices and fill with two triangles.
fn add_segment(mesh: &mut Mesh, width: f32, color: Option<Color32>) {
    let v1 = (mesh.vertices.len() - 1) as u32;
    let v2 = v1 - 1;
    let p1 = mesh.vertices[v1 as usize].pos;
    let p2 = mesh.vertices[v2 as usize].pos;

    // Use the last color if no color is provided.
    let color = color.unwrap_or_else(|| mesh.vertices[v1 as usize].color);

    mesh.colored_vertex(p2 + vec2(width, 0.0), color);
    mesh.colored_vertex(p1 + vec2(width, 0.0), color);

    // v2--n2 (v2->n2->v1) (v1->n2->n1)
    // v1--n1 winding order apparently doesn't matter
    mesh.add_triangle(v2, v1 + 1, v1);
    mesh.add_triangle(v1, v1 + 1, v1 + 2);
}
