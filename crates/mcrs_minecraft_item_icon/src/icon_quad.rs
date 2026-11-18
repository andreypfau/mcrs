use std::f32::consts::PI;

use glam::{EulerRot, Mat3, Mat4, Quat, Vec3};
use mcrs_minecraft_model::item_model_bake::{BakedItemModel, ItemQuad};

use crate::gui_vertex::GuiVertex;
use crate::icon_layer::IconLayer;
use crate::light_directions::{light_directions, light_factor};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconQuad {
    pub vertices: [GuiVertex; 4],
    pub shade: f32,
    pub tint: Option<u32>,
}

impl IconQuad {
    #[must_use]
    pub fn color(&self, tints: &[u32]) -> [u8; 4] {
        let tint = self
            .tint
            .and_then(|index| tints.get(usize::try_from(index).ok()?).copied())
            .unwrap_or(IconLayer::UNTINTED);
        let channel = |shift: u32| (((tint >> shift) & 0xFF) as f32 * self.shade).round() as u8;
        [channel(16), channel(8), channel(0), (tint >> 24) as u8]
    }

    #[must_use]
    pub fn tinted(&self, tints: &[u32]) -> [GuiVertex; 4] {
        let color = self.color(tints);
        self.vertices.map(|vertex| GuiVertex { color, ..vertex })
    }
}

pub fn icon_quads(model: &BakedItemModel) -> impl Iterator<Item = IconQuad> + '_ {
    let matrix = gui_matrix(model);
    let normals = Mat3::from_mat4(matrix).inverse().transpose();
    let lights = light_directions(model.gui_light);
    model
        .quads
        .iter()
        .map(move |quad| pose(quad, matrix, normals, lights))
}

fn pose(quad: &ItemQuad, matrix: Mat4, normals: Mat3, lights: [Vec3; 2]) -> IconQuad {
    let normal = (normals * quad.normal).normalize();
    let positions = quad
        .positions
        .map(|position| matrix.transform_point3(position));
    let [first, second, third, fourth] = positions;
    let [uv_first, uv_second, uv_third, uv_fourth] = quad.uvs;
    let vertex = |position: Vec3, uv: [f32; 2]| GuiVertex {
        pos: position.to_array(),
        uv,
        color: [u8::MAX; 4],
        sprite: u32::from(quad.sprite),
    };
    let mut vertices = [
        vertex(first, uv_first),
        vertex(second, uv_second),
        vertex(third, uv_third),
        vertex(fourth, uv_fourth),
    ];
    if (second - first).cross(third - first).dot(normal) > 0.0 {
        vertices.reverse();
    }
    IconQuad {
        vertices,
        shade: light_factor(normal, lights),
        tint: quad.tint,
    }
}

fn gui_matrix(model: &BakedItemModel) -> Mat4 {
    let display = model.display;
    let radians = display.rotation_deg * (PI / 180.0);
    let posed = Mat4::from_translation(display.translation)
        * Mat4::from_quat(Quat::from_euler(
            EulerRot::XYZ,
            radians.x,
            radians.y,
            radians.z,
        ))
        * Mat4::from_scale(display.scale)
        * Mat4::from_translation(Vec3::splat(-0.5));
    Mat4::from_translation(Vec3::new(8.0, 8.0, 0.0))
        * Mat4::from_scale(Vec3::new(16.0, -16.0, 16.0))
        * posed
}
