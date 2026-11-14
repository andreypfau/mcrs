use bevy::math::{IRect, IVec2, Mat4, UVec2, Vec2, Vec3};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_render::{EntityViews, FigureFrame, FigureView, FramePosed};
use mcrs_minecraft_entity_visual::{
    Age, ClientEntity, EntityLight, Equipment, HostLight, Living, Motion, Placement, SyncedData,
};
use uuid::Uuid;

use crate::inventory_screen::origin;
use crate::scene::{GuiScene, window_framebuffer};

const GUI_UNITS_PER_BLOCK: f32 = 30.0;
const RECT_LEFT: i32 = 26;
const RECT_TOP: i32 = 8;
const RECT_WIDTH: i32 = 49;
const RECT_HEIGHT: i32 = 70;
const CENTRE: Vec2 = Vec2::new(50.5, 43.0);
const FEET_DROP_BLOCKS: f32 = 0.0625;
const TURN_DISTANCE: f32 = 40.0;
const TURN_DEGREES_PER_RADIAN: f32 = 20.0;
// The turned and tilted figure stays within about 2.1 blocks of its feet along the view axis.
const DEPTH_HALF_RANGE_BLOCKS: f32 = 4.0;
const LIGHT_0: Vec3 = Vec3::new(0.2, 1.0, 1.0);
const LIGHT_1: Vec3 = Vec3::new(-0.2, 1.0, 0.0);
// No packet of a server names an entity with this id.
const FIGURE_NETWORK_ID: i32 = i32::MIN;
const FULL_LIGHT: EntityLight = EntityLight { block: 15, sky: 15 };

/// The player figure of the inventory screen. Only the figure view draws it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryFigure;

/// The figure of the local player with the UUID `uuid`: a player at the origin of figure space,
/// in full light, that takes the skin of the profile with that UUID.
#[must_use]
pub fn inventory_figure(uuid: Uuid) -> impl Bundle {
    let placement = Placement::default();
    (
        InventoryFigure,
        ClientEntity {
            network_id: FIGURE_NETWORK_ID,
            uuid,
            kind: EntityType::Player,
        },
        Motion::new(placement, placement.position),
        Age::default(),
        SyncedData::default(),
        Living::default(),
        Equipment::default(),
        EntityViews::FIGURE,
        FramePosed,
        HostLight,
        FULL_LIGHT,
    )
}

/// Turns the figure to the cursor and gives the figure view its frame while the inventory
/// screen is open, and no frame otherwise or while the local player is invisible.
pub(crate) fn pose_inventory_figure(
    screen: Res<Screen>,
    scene: Res<GuiScene>,
    window: Single<&Window, With<PrimaryWindow>>,
    player: Query<Option<&SyncedData>, With<Player>>,
    mut figures: Query<(&mut Living, &mut Motion), With<InventoryFigure>>,
    mut view: ResMut<FigureView>,
) {
    let invisible = player.iter().flatten().any(SyncedData::is_invisible);
    if *screen != Screen::Inventory || invisible {
        view.set_if_neq(FigureView(None));
        return;
    }
    let standing = FigureBox {
        height: EntityTypeData::PLAYER.height,
        eye_height: EntityTypeData::PLAYER.eye_height,
    };
    let points = figure_points(origin(scene.size), standing);
    let angles = figure_angles(points.eye, scene.cursor.as_vec2());
    let frame = clip_from_figure(
        points.feet,
        angles.tilt,
        window_framebuffer(&window),
        scene.size.scale,
    )
    .map(|clip_from_figure| FigureFrame {
        clip_from_figure,
        lights: figure_lights(angles.tilt),
    });
    view.set_if_neq(FigureView(frame));

    // A yaw of 0 faces +Z, toward the viewer, and a positive yaw turns the front to -X.
    let body_yaw = -angles.turn;
    let head_yaw = 2.0 * body_yaw;
    let pitch = -angles.tilt;
    for (mut living, motion) in &mut figures {
        if living.body_yaw != body_yaw || living.head_yaw != head_yaw {
            living.body_yaw = body_yaw;
            living.head_yaw = head_yaw;
        }
        motion
            .map_unchanged(|motion| &mut motion.current.pitch)
            .set_if_neq(pitch);
    }
}

/// The box height and the eye height of the local player in its pose, in blocks, before the
/// scale attribute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigureBox {
    pub(crate) height: f32,
    pub(crate) eye_height: f32,
}

/// The feet point and the eye point of the figure, in GUI units with `y` down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigurePoints {
    pub(crate) feet: Vec2,
    pub(crate) eye: Vec2,
}

/// The turn of the figure to the mouse, in degrees. A positive `turn` turns the front to the
/// right of the screen; a positive `tilt` turns the head up and the top of the model away from
/// the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigureAngles {
    pub(crate) turn: f32,
    pub(crate) tilt: f32,
}

/// The rectangle that clips the figure, in GUI units, for the origin of the inventory image.
#[must_use]
pub(crate) fn figure_rect(origin: IVec2) -> IRect {
    let min = origin + IVec2::new(RECT_LEFT, RECT_TOP);
    IRect::from_corners(min, min + IVec2::new(RECT_WIDTH, RECT_HEIGHT))
}

#[must_use]
pub(crate) fn figure_points(origin: IVec2, figure_box: FigureBox) -> FigurePoints {
    let centre = origin.as_vec2() + CENTRE;
    let feet =
        centre + Vec2::Y * (figure_box.height / 2.0 + FEET_DROP_BLOCKS) * GUI_UNITS_PER_BLOCK;
    let eye = feet - Vec2::Y * figure_box.eye_height * GUI_UNITS_PER_BLOCK;
    FigurePoints { feet, eye }
}

/// The angles for the mouse at `mouse`, in GUI units, and the eye point `eye`.
#[must_use]
pub(crate) fn figure_angles(eye: Vec2, mouse: Vec2) -> FigureAngles {
    let angle = |offset: f32| TURN_DEGREES_PER_RADIAN * (offset / TURN_DISTANCE).atan();
    FigureAngles {
        turn: angle(mouse.x - eye.x),
        tilt: angle(eye.y - mouse.y),
    }
}

/// The parallel projection from figure space to clip space: `x` to the right, `y` up and `z` to
/// the viewer, one block for each unit, the feet at the origin. Clip depth grows toward the
/// viewer. `None` for a window with no pixels.
#[must_use]
pub(crate) fn clip_from_figure(
    feet: Vec2,
    tilt_degrees: f32,
    window: UVec2,
    scale: u32,
) -> Option<Mat4> {
    if window.x == 0 || window.y == 0 {
        return None;
    }
    let gui_size = window.as_vec2() / scale.max(1) as f32;
    let clip_from_gui = Mat4::from_translation(Vec3::new(-1.0, 1.0, 0.5))
        * Mat4::from_scale(Vec3::new(
            2.0 / gui_size.x,
            -2.0 / gui_size.y,
            0.5 / DEPTH_HALF_RANGE_BLOCKS,
        ));
    let gui_from_figure = Mat4::from_translation(feet.extend(0.0))
        * Mat4::from_scale(Vec3::new(GUI_UNITS_PER_BLOCK, -GUI_UNITS_PER_BLOCK, 1.0))
        * Mat4::from_rotation_x(-tilt_degrees.to_radians());
    Some(clip_from_gui * gui_from_figure)
}

/// The two light directions in figure space. They turn against the tilt, so they stay fixed on
/// the screen.
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
    use crate::item_resolve::light_factor;

    const ORIGIN: IVec2 = IVec2::new(125, 37);

    fn near(actual: f32, expected: f32, tolerance: f32) -> bool {
        (actual - expected).abs() <= tolerance
    }

    #[test]
    fn layout_matches_the_inventory_of_an_854_by_480_window() {
        assert_eq!(
            figure_rect(ORIGIN),
            IRect::new(151, 45, 200, 115),
            "the figure rectangle of the experiment"
        );

        let poses = [
            (
                FigureBox {
                    height: 1.8,
                    eye_height: 1.62,
                },
                71.875,
                23.275,
            ),
            (
                FigureBox {
                    height: 1.5,
                    eye_height: 1.27,
                },
                67.375,
                29.275,
            ),
        ];
        for (figure_box, feet_y, eye_y) in poses {
            let points = figure_points(ORIGIN, figure_box);
            let expected = FigurePoints {
                feet: Vec2::new(175.5, 37.0 + feet_y),
                eye: Vec2::new(175.5, 37.0 + eye_y),
            };
            assert!(
                points.feet.abs_diff_eq(expected.feet, 1e-4)
                    && points.eye.abs_diff_eq(expected.eye, 1e-4),
                "{figure_box:?}: {points:?} is not {expected:?}"
            );
        }

        let eye = figure_points(
            ORIGIN,
            FigureBox {
                height: 1.8,
                eye_height: 1.62,
            },
        )
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
    fn projection_puts_the_feet_on_their_window_pixel_and_keeps_the_light_on_the_screen() {
        let window = UVec2::new(854, 480);
        let feet = figure_points(
            ORIGIN,
            FigureBox {
                height: 1.8,
                eye_height: 1.62,
            },
        )
        .feet;
        let Some(upright) = clip_from_figure(feet, 0.0, window, 2) else {
            panic!("a window with pixels has a projection");
        };
        let window_from_clip =
            |clip: Vec3| Vec2::new((clip.x + 1.0) / 2.0, (1.0 - clip.y) / 2.0) * window.as_vec2();
        let feet_pixel = window_from_clip(upright.project_point3(Vec3::ZERO));
        assert!(
            feet_pixel.abs_diff_eq(Vec2::new(351.0, 217.75), 1e-3),
            "the feet are at {feet_pixel} window pixels"
        );
        let eye_pixel = window_from_clip(upright.project_point3(Vec3::Y * 1.62));
        assert!(
            eye_pixel.abs_diff_eq(Vec2::new(351.0, 120.55), 1e-3),
            "the eye is at {eye_pixel} window pixels"
        );

        let Some(tilted) = clip_from_figure(feet, 15.0, window, 2) else {
            panic!("a window with pixels has a projection");
        };
        let feet_depth = tilted.project_point3(Vec3::ZERO).z;
        let top_depth = tilted.project_point3(Vec3::Y * 1.8).z;
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
