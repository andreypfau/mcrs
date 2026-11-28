use bevy::math::proj;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_client_world::local_motion::LocalMotion;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_client_world::session::InSession;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_render::{EntityViews, FigureFrame, FigureView, FramePosed};
use mcrs_minecraft_entity_visual::{
    EntityLight, HostLight, Living, Motion, Placement, SyncedData, host_living_entity,
};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::join::ServerProfile;
use mcrs_minecraft_pose::{Dimensions, Pose};
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use uuid::Uuid;

use crate::menu_screen;
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
const DEPTH_HALF_RANGE_BLOCKS: f32 = 4.0;
const LIGHT_0: Vec3 = Vec3::new(0.2, 1.0, 1.0);
const LIGHT_1: Vec3 = Vec3::new(-0.2, 1.0, 0.0);
const FULL_LIGHT: EntityLight = EntityLight { block: 15, sky: 15 };

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryFigure;

#[must_use]
pub fn inventory_figure(uuid: Uuid) -> Option<impl Bundle> {
    let host = host_living_entity(EntityType::Player, uuid, Placement::default())?;
    Some((
        host,
        InventoryFigure,
        EntityViews::FIGURE,
        FramePosed,
        HostLight,
        FULL_LIGHT,
    ))
}

pub(crate) fn spawn_inventory_figure(
    event: On<Received<ClientboundLogin>>,
    profiles: Query<&ServerProfile>,
    figures: Query<(), With<InventoryFigure>>,
    mut commands: Commands,
) {
    if !figures.is_empty() {
        return;
    }
    let Some(figure) = profiles
        .get(event.entity)
        .ok()
        .and_then(|profile| inventory_figure(profile.id))
    else {
        return;
    };
    commands.spawn((figure, InSession(event.entity)));
}

pub(crate) fn pose_inventory_figure(
    screen: Res<Screen>,
    scene: Res<GuiScene>,
    window: Single<&Window, With<PrimaryWindow>>,
    player: Query<(&SyncedData, Option<&LocalMotion>, Option<&Menus>), With<Player>>,
    mut figures: Query<
        (&mut Living, &mut Motion, &mut SyncedData),
        (With<InventoryFigure>, Without<Player>),
    >,
    mut view: ResMut<FigureView>,
) {
    let invisible = player.iter().any(|(data, ..)| data.is_invisible());
    let inventory_open = player
        .iter()
        .filter_map(|(.., menus)| menus)
        .any(|menus| menus.open().is_some() && menus.open() == menus.iter().next());
    if *screen != Screen::Menu || !inventory_open || invisible {
        view.set_if_neq(FigureView(None));
        return;
    }
    let pose = player
        .iter()
        .find_map(|(_, motion, _)| motion)
        .map_or(Pose::Standing, |motion| motion.0.pose);
    let points = figure_points(
        menu_screen::inventory_window_corner(scene.size),
        pose.player_dimensions(),
    );
    let angles = figure_angles(points.eye, scene.cursor.as_vec2());
    let frame = FigureFrame {
        clip_from_figure: clip_from_figure(
            points.feet,
            angles.tilt,
            window_framebuffer(&window),
            scene.size.scale,
        ),
        lights: figure_lights(angles.tilt),
    };
    view.set_if_neq(FigureView(Some(frame)));

    let body_yaw = -angles.turn;
    let head_yaw = 2.0 * body_yaw;
    let pitch = -angles.tilt;
    for (mut living, motion, data) in &mut figures {
        data.map_unchanged(|data| &mut data.pose).set_if_neq(pose);
        if living.body_yaw != body_yaw || living.head_yaw != head_yaw {
            living.body_yaw = body_yaw;
            living.head_yaw = head_yaw;
        }
        motion
            .map_unchanged(|motion| &mut motion.current.pitch)
            .set_if_neq(pitch);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigurePoints {
    pub(crate) feet: Vec2,
    pub(crate) eye: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigureAngles {
    pub(crate) turn: f32,
    pub(crate) tilt: f32,
}

#[must_use]
pub(crate) fn figure_rect(origin: IVec2) -> IRect {
    let min = origin + IVec2::new(RECT_LEFT, RECT_TOP);
    IRect::from_corners(min, min + IVec2::new(RECT_WIDTH, RECT_HEIGHT))
}

#[must_use]
pub(crate) fn figure_points(origin: IVec2, dimensions: Dimensions) -> FigurePoints {
    let centre = origin.as_vec2() + CENTRE;
    let feet =
        centre + Vec2::Y * (dimensions.height / 2.0 + FEET_DROP_BLOCKS) * GUI_UNITS_PER_BLOCK;
    let eye = feet - Vec2::Y * dimensions.eye_height * GUI_UNITS_PER_BLOCK;
    FigurePoints { feet, eye }
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
pub(crate) fn clip_from_figure(feet: Vec2, tilt_degrees: f32, window: UVec2, scale: u32) -> Mat4 {
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
        * Mat4::from_scale(Vec3::new(GUI_UNITS_PER_BLOCK, -GUI_UNITS_PER_BLOCK, 1.0))
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
    use mcrs_minecraft_item_icon::light_factor;

    const ORIGIN: IVec2 = IVec2::new(125, 37);

    #[test]
    fn the_figure_of_a_session_ends_with_the_session() {
        let mut world = World::new();
        world.add_observer(spawn_inventory_figure);
        let session = world
            .spawn(ServerProfile {
                id: Uuid::nil(),
                username: "host".to_owned(),
            })
            .id();
        for _ in 0..2 {
            world.trigger(Received::<ClientboundLogin>::new(
                session,
                Vec::new().into(),
            ));
            world.flush();
        }
        let mut figures = world.query_filtered::<(), With<InventoryFigure>>();
        assert_eq!(figures.iter(&world).count(), 1, "a second login keeps it");
        world.despawn(session);
        assert_eq!(figures.iter(&world).count(), 0);
    }

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
            (Pose::Standing.player_dimensions(), 71.875, 23.275),
            (Pose::Crouching.player_dimensions(), 67.375, 29.275),
        ];
        for (dimensions, feet_y, eye_y) in poses {
            let points = figure_points(ORIGIN, dimensions);
            let expected = FigurePoints {
                feet: Vec2::new(175.5, 37.0 + feet_y),
                eye: Vec2::new(175.5, 37.0 + eye_y),
            };
            assert!(
                points.feet.abs_diff_eq(expected.feet, 1e-4)
                    && points.eye.abs_diff_eq(expected.eye, 1e-4),
                "{dimensions:?}: {points:?} is not {expected:?}"
            );
        }

        let eye = figure_points(ORIGIN, Pose::Standing.player_dimensions()).eye;
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
        let feet = figure_points(ORIGIN, standing).feet;
        let upright = clip_from_figure(feet, 0.0, window, 2);
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

        let tilted = clip_from_figure(feet, 15.0, window, 2);
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
