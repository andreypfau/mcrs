use bevy::math::proj;
use bevy::prelude::*;

const TURN_DISTANCE: f32 = 40.0;
const TURN_DEGREES_PER_RADIAN: f32 = 20.0;
const DEPTH_HALF_RANGE_BLOCKS: f32 = 4.0;
const LIGHT_0: Vec3 = Vec3::new(0.2, 1.0, 1.0);
const LIGHT_1: Vec3 = Vec3::new(-0.2, 1.0, 0.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigureAngles {
    pub(crate) turn: f32,
    pub(crate) tilt: f32,
}

#[must_use]
pub(crate) fn figure_angles(eye: Vec2, mouse: Vec2) -> FigureAngles {
    let angle = |offset: f32| TURN_DEGREES_PER_RADIAN * (offset / TURN_DISTANCE).atan();
    FigureAngles {
        turn: angle(mouse.x - eye.x),
        tilt: angle(eye.y - mouse.y),
    }
}

#[must_use]
pub(crate) fn clip_from_figure(
    feet: Vec2,
    tilt_degrees: f32,
    window: UVec2,
    scale: u32,
    units_per_block: f32,
) -> Mat4 {
    let gui_size = window.as_vec2() / scale.max(1) as f32;
    let clip_from_gui = proj::orthographic(
        0.0,
        gui_size.x,
        gui_size.y,
        0.0,
        DEPTH_HALF_RANGE_BLOCKS,
        -DEPTH_HALF_RANGE_BLOCKS,
    );
    let gui_from_figure = Mat4::from_translation(feet.extend(0.0))
        * Mat4::from_scale(Vec3::new(units_per_block, -units_per_block, 1.0))
        * Mat4::from_rotation_x(-tilt_degrees.to_radians());
    clip_from_gui * gui_from_figure
}

#[must_use]
pub(crate) fn figure_lights(tilt_degrees: f32) -> [Vec3; 2] {
    let untilt = Mat4::from_rotation_x(tilt_degrees.to_radians());
    [
        untilt.transform_vector3(LIGHT_0.normalize()),
        untilt.transform_vector3(LIGHT_1.normalize()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure_placement::FigurePlacement;
    use mcrs_minecraft_item_icon::light_factor;
    use mcrs_minecraft_pose::Pose;

    const ORIGIN: IVec2 = IVec2::new(125, 37);

    fn near(actual: f32, expected: f32, tolerance: f32) -> bool {
        (actual - expected).abs() <= tolerance
    }

    #[test]
    fn angles_follow_the_mouse_around_the_eye() {
        let eye = FigurePlacement::SURVIVAL
            .points(ORIGIN, Pose::Standing.player_dimensions())
            .eye;
        let mice = [
            (Vec2::new(175.0, 60.0), -0.25, 0.14),
            (Vec2::new(215.0, 60.0), 15.58, 0.14),
            (Vec2::new(135.0, 60.0), -15.83, 0.14),
            (Vec2::new(175.0, 20.0), -0.25, 15.78),
            (Vec2::new(175.0, 100.0), -0.25, -15.64),
            (Vec2::new(426.0, 0.0), 28.25, 19.70),
        ];
        for (mouse, turn, tilt) in mice {
            let angles = figure_angles(eye, mouse);
            assert!(
                near(angles.turn, turn, 0.006) && near(angles.tilt, tilt, 0.006),
                "mouse {mouse}: {angles:?} is not turn {turn}, tilt {tilt}"
            );
        }
    }

    #[test]
    fn projection_puts_feet_on_their_pixel_and_light_on_screen() {
        let window = UVec2::new(854, 480);
        let standing = Pose::Standing.player_dimensions();
        let feet = FigurePlacement::SURVIVAL.points(ORIGIN, standing).feet;
        let upright = clip_from_figure(feet, 0.0, window, 2, 30.0);
        let window_from_clip =
            |clip: Vec3| Vec2::new((clip.x + 1.0) / 2.0, (1.0 - clip.y) / 2.0) * window.as_vec2();
        let feet_pixel = window_from_clip(upright.project_point3(Vec3::ZERO));
        assert!(
            feet_pixel.abs_diff_eq(Vec2::new(351.0, 217.75), 1e-3),
            "the feet are at {feet_pixel} window pixels"
        );
        let eye_pixel = window_from_clip(upright.project_point3(Vec3::Y * standing.eye_height));
        assert!(
            eye_pixel.abs_diff_eq(Vec2::new(351.0, 120.55), 1e-3),
            "the eye is at {eye_pixel} window pixels"
        );

        let tilted = clip_from_figure(feet, 15.0, window, 2, 30.0);
        let feet_depth = tilted.project_point3(Vec3::ZERO).z;
        let top_depth = tilted.project_point3(Vec3::Y * standing.height).z;
        assert!(
            top_depth < feet_depth,
            "a positive tilt moves the top away from the viewer: {top_depth} >= {feet_depth}"
        );

        let faces = [
            (Vec3::Z, 0.8201),
            (Vec3::Y, 1.0),
            (Vec3::X, 0.4840),
            (Vec3::NEG_X, 0.5177),
            (Vec3::NEG_Y, 0.4),
        ];
        for (normal, factor) in faces {
            let actual = light_factor(normal, figure_lights(0.0));
            assert!(
                near(actual, factor, 1e-4),
                "face {normal}: factor {actual} is not {factor}"
            );
        }
        for (screen, tilted_light) in figure_lights(0.0).into_iter().zip(figure_lights(15.0)) {
            let on_screen_upright = upright.transform_vector3(screen);
            let on_screen_tilted = tilted.transform_vector3(tilted_light);
            assert!(
                on_screen_tilted.abs_diff_eq(on_screen_upright, 1e-5),
                "a light turns with the tilt: {on_screen_tilted} is not {on_screen_upright}"
            );
        }
    }
}
