use std::cmp::Ordering;

use bevy::{
    prelude::{Vec2, Vec4},
    reflect::{FromReflect, Reflect},
};
use bevy_egui::egui::{self, epaint::Hsva, widgets::color_picker::*, *};
use bevy_hanabi::{ColorOverLifetimeModifier, SizeOverLifetimeModifier};

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

#[derive(Clone, Reflect, FromReflect)]
pub struct SizeGradient {
    keys: Vec<(f32, Vec2)>,
}

impl Default for SizeGradient {
    fn default() -> Self {
        Self {
            keys: vec![(0.5, Vec2::splat(1.0))],
        }
    }
}

trait IntoColor {
    fn into_color(&self) -> Color32;
}

impl IntoColor for Vec4 {
    fn into_color(&self) -> Color32 {
        rgba(self).into()
    }
}

impl IntoColor for Vec2 {
    fn into_color(&self) -> Color32 {
        Color32::GRAY
    }
}

fn initial_value<T>(keys: &Vec<(f32, T)>) -> Option<&T> {
    if keys[0].0 > 0.0 {
        Some(&keys[0].1)
    } else if let Some((_k, v)) = keys.iter().take_while(|k| k.0 == 0.0).last() {
        Some(v)
    } else {
        None
    }
}

/// Add draggable keys.
fn show_keys(keys: &mut Vec<(f32, impl IntoColor)>, rect: Rect, ui: &mut Ui) -> bool {
    let mut sort = false;
    let mut changed = false;
    let count = keys.len();

    // The scope is to paper over the layered space allocations. Following widgets will get
    // placed after the last (inset) allocation without it.
    ui.scope(|ui| {
        for i in 0..count {
            let (key, value) = &mut keys[i];
            let fill = value.into_color();

            let re = ui.allocate_rect(
                Rect::from_center_size(
                    pos2(lerp(rect.x_range(), *key), rect.center().y),
                    egui::Vec2::splat(rect.height() / 2.0),
                ),
                Sense::click_and_drag(),
            );
            let visuals = ui.style().interact(&re);
            ui.painter().add(epaint::CircleShape {
                center: re.rect.center(),
                radius: re.rect.size().x / 2.0,
                fill,
                stroke: visuals.fg_stroke,
            });

            // You need at least one key.
            if count > 1 && re.clicked_by(PointerButton::Secondary) {
                // Delete the key.
                keys.remove(i);
                changed = true;
                break;
            }

            if re.dragged() {
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
    });

    if sort {
        keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    }
    sort || changed
}

pub trait Gradient {
    type Value;

    fn show(&mut self, ui: &mut Ui) -> Response {
        self.show_gradient(ui) | self.show_values(ui)
    }

    fn show_gradient(&mut self, ui: &mut Ui) -> Response;
    fn show_values(&mut self, ui: &mut Ui) -> Response;
}

impl Gradient for ColorGradient {
    type Value = Vec4;

    fn show_gradient(&mut self, ui: &mut Ui) -> Response {
        let desired_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
        let (rect, mut response) = ui.allocate_at_least(desired_size, Sense::hover());

        if ui.is_rect_visible(rect) {
            let w = rect.width();

            let keys = &mut self.keys;
            assert!(keys.len() > 0);

            // The starting color is the first key (if non-zero) or the last zero-value key.
            let color = initial_value(keys).map(rgba).unwrap_or_default();
            let mut mesh = start_strip(rect, color.into());

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

            let visuals = ui.style().interact(&response);
            ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke);

            // if ui.scope(|ui| self.show_keys(ui)).inner {
            //     response.mark_changed();
            // }
            if show_keys(&mut self.keys, rect, ui) {
                response.mark_changed();
            }
        }
        response
    }

    // The color picker from egui is natively HSVA. So there's a lot of unnecessary conversion and
    // weirdness happening. We are getting spammed with changes even when the color is not changing,
    // which I presume has something to do with the conversion to HSVA. Which is why egui caches them?
    // We may have to write our own color picker just for RGBA.
    fn show_values(&mut self, ui: &mut Ui) -> Response {
        let keys = &mut self.keys;

        let mut changed = false;

        let mut response = ui
            .horizontal(|ui| {
                // Make the buttons smaller.
                ui.spacing_mut().interact_size = egui::Vec2::splat(12.0);

                for (_key, color) in keys.iter_mut() {
                    let mut hsva = hsva(color);
                    if color_edit_button_hsva(ui, &mut hsva, Alpha::OnlyBlend).changed() {
                        *color = Vec4::from_slice(&hsva.to_rgba_premultiplied());
                        // TODO only set changed when the popup is closed
                        changed = true;
                    }
                }

                if ui.small_button("+").clicked() {
                    keys.push((1.0, Vec4::ZERO));
                    changed = true;
                }
            })
            .response;

        if changed {
            response.mark_changed();
        }

        response
    }
}

impl Gradient for SizeGradient {
    type Value = Vec2;

    fn show_gradient(&mut self, ui: &mut Ui) -> Response {
        assert!(self.keys.len() > 0);

        let desired_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
        let (rect, mut response) = ui.allocate_at_least(desired_size, Sense::hover());
        let visuals = ui.style().interact(&response);

        if ui.is_rect_visible(rect) {
            let w = rect.width();

            let stroke_x = Stroke::new(visuals.fg_stroke.width, Color32::RED);
            let stroke_y = Stroke::new(visuals.fg_stroke.width, Color32::GREEN);

            let mut max = Vec2::ZERO;

            let initial =
                initial_value(&self.keys).map(|v| (pos2(rect.min.x, v.x), pos2(rect.min.x, v.y)));

            // Add a final key if the last one is < 1.0.
            let last = self
                .keys
                .last()
                .filter(|(k, _)| *k < 1.0)
                .map(|(_, v)| (pos2(rect.max.x, v.x), pos2(rect.max.x, v.y)));

            let (mut line_x, mut line_y): (Vec<_>, Vec<_>) = initial
                .into_iter()
                .chain(self.keys.iter().map(|(k, v)| {
                    max = max.max(*v);
                    let x = rect.min.x + k * w;
                    (pos2(x, v.x), pos2(x, v.y))
                }))
                .chain(last.into_iter())
                .unzip();

            // Scale to fit vertically and offset from rect.
            let max = rect.height() / max.x.max(max.y);
            line_x.iter_mut().for_each(|p| p.y = rect.max.y - p.y * max);
            line_y.iter_mut().for_each(|p| p.y = rect.max.y - p.y * max);

            ui.painter().add(Shape::line(line_x, stroke_x));
            ui.painter().add(Shape::line(line_y, stroke_y));

            ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke);

            if show_keys(&mut self.keys, rect, ui) {
                response.mark_changed();
            }
        }
        response
    }

    fn show_values(&mut self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size = egui::Vec2::splat(4.0);

            let mut response = self
                .keys
                .iter_mut()
                .map(|(_key, value)| {
                    ui.add(
                        egui::DragValue::new(&mut value[0])
                            .prefix("x: ")
                            .speed(0.01)
                            .clamp_range(0.0..=f32::MAX),
                    ) | ui.add(
                        egui::DragValue::new(&mut value[1])
                            .prefix("y: ")
                            .speed(0.01)
                            .clamp_range(0.0..=f32::MAX),
                    )
                })
                .reduce(|a, b| a | b)
                .expect("at least one key");

            if ui.small_button("+").clicked() {
                self.keys.push((1.0, Vec2::ZERO));
                response.mark_changed();
            }
            response
        })
        .inner
    }
}

impl From<ColorGradient> for ColorOverLifetimeModifier {
    fn from(g: ColorGradient) -> Self {
        let mut gradient = bevy_hanabi::Gradient::new();
        for (key, color) in g.keys {
            gradient.add_key(key, color);
        }

        ColorOverLifetimeModifier { gradient }
    }
}

impl From<SizeGradient> for SizeOverLifetimeModifier {
    fn from(g: SizeGradient) -> Self {
        let mut gradient = bevy_hanabi::Gradient::new();
        for (key, size) in g.keys {
            gradient.add_key(key, size);
        }

        SizeOverLifetimeModifier { gradient }
    }
}

// This is still the fastest way to Color32?
fn rgba(c: &Vec4) -> Rgba {
    Rgba::from_rgba_premultiplied(c[0], c[1], c[2], c[3])
}

fn hsva(c: &Vec4) -> Hsva {
    Hsva::from_rgba_premultiplied(c[0], c[1], c[2], c[3])
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
