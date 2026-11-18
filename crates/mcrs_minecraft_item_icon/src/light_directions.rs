use std::f32::consts::PI;

use glam::{EulerRot, Mat3, Vec3};
use mcrs_minecraft_model_file::model_file::GuiLight;

const LIGHT_0: Vec3 = Vec3::new(0.2, 1.0, -0.7);
const LIGHT_1: Vec3 = Vec3::new(-0.2, 1.0, 0.7);

#[must_use]
pub fn light_directions(gui_light: GuiLight) -> [Vec3; 2] {
    let base = Mat3::from_euler(EulerRot::YXZ, -PI / 8.0, PI * 3.0 / 4.0, 0.0);
    let pose = match gui_light {
        GuiLight::Front => base,
        GuiLight::Side => {
            Mat3::from_diagonal(Vec3::new(1.0, -1.0, 1.0))
                * Mat3::from_euler(EulerRot::YXZ, 1.082_104_1, 3.237_585_8, 0.0)
                * base
        }
    };
    [pose * LIGHT_0.normalize(), pose * LIGHT_1.normalize()]
}

#[must_use]
pub fn light_factor(normal: Vec3, lights: [Vec3; 2]) -> f32 {
    let [first, second] = lights;
    let diffuse = normal.dot(first).max(0.0) + normal.dot(second).max(0.0);
    (diffuse * 0.6 + 0.4).min(1.0)
}
