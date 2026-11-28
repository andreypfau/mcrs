use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_client_world::local_motion::LocalMotion;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_client_world::session::InSession;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_render::{
    EntitySlotSystems, EntityViews, FigureFrame, FigureView, FramePosed,
};
use mcrs_minecraft_entity_visual::{
    EntityLight, HostLight, Living, Motion, Placement, SyncedData, host_living_entity,
};
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_client::shows_inventory_menu;
use mcrs_minecraft_menu_geometry::{CreativeGeometry, MenuGeometry};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::join::ServerProfile;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use uuid::Uuid;

use crate::creative_screen_shown::shows_creative_screen;
use crate::creative_screen_state::CreativeScreenState;
use crate::figure_placement::FigurePlacement;
use crate::figure_projection;
use crate::gui_pointer::GuiPointer;
use crate::gui_size::GuiSize;
use crate::gui_size::window_framebuffer;
use crate::irect;

const FULL_LIGHT: EntityLight = EntityLight { block: 15, sky: 15 };

pub struct InventoryFigurePlugin;

impl Plugin for InventoryFigurePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_inventory_figure).add_systems(
            PostUpdate,
            pose_inventory_figure.before(EntitySlotSystems).run_if(
                resource_exists::<FigureView>
                    .and_then(resource_equals(Screen::Menu).or_else(resource_changed::<Screen>)),
            ),
        );
    }
}

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
    size: Res<GuiSize>,
    pointer: Res<GuiPointer>,
    creative: Option<Res<CreativeScreenState>>,
    window: Single<&Window, With<PrimaryWindow>>,
    player: Query<
        (
            &SyncedData,
            Option<&LocalMotion>,
            &Menus,
            Option<&Abilities>,
        ),
        With<Player>,
    >,
    mut figures: Query<
        (&mut Living, &mut Motion, &mut SyncedData),
        (With<InventoryFigure>, Without<Player>),
    >,
    mut view: ResMut<FigureView>,
) {
    let shown = player
        .single()
        .ok()
        .and_then(|(data, motion, menus, abilities)| {
            let placed = placed_figure(*screen, menus, abilities, creative.as_deref(), *size)?;
            (!data.is_invisible()).then_some((placed, motion))
        });
    let Some((placed, motion)) = shown else {
        view.set_if_neq(FigureView(None));
        return;
    };
    let pose = motion.map_or(Pose::Standing, |motion| motion.0.pose);
    let placement = placed.placement;
    let points = placement.points(placed.origin, pose.player_dimensions());
    let angles =
        figure_projection::figure_angles(points.eye, irect::point(pointer.point()).as_vec2());
    let frame = FigureFrame {
        clip_from_figure: figure_projection::clip_from_figure(
            points.feet,
            angles.tilt,
            window_framebuffer(&window),
            size.scale,
            placement.units_per_block,
        ),
        lights: figure_projection::figure_lights(angles.tilt),
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
struct PlacedFigure {
    origin: IVec2,
    placement: FigurePlacement,
}

fn placed_figure(
    screen: Screen,
    menus: &Menus,
    abilities: Option<&Abilities>,
    creative: Option<&CreativeScreenState>,
    size: GuiSize,
) -> Option<PlacedFigure> {
    if shows_creative_screen(screen, menus, abilities) {
        let placement = FigurePlacement::for_creative_tab(creative?.selected_tab().kind)?;
        Some(PlacedFigure {
            origin: irect::point(CreativeGeometry::window(size.width, size.height)),
            placement,
        })
    } else if screen == Screen::Menu && shows_inventory_menu(menus) {
        Some(PlacedFigure {
            origin: inventory_window_corner(size),
            placement: FigurePlacement::SURVIVAL,
        })
    } else {
        None
    }
}

#[must_use]
pub(crate) fn inventory_window_corner(size: GuiSize) -> IVec2 {
    irect::point(MenuGeometry::INVENTORY.window(size.width, size.height))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
