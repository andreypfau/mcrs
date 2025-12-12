use crate::client_info::ClientViewDistance;
use crate::world::entity::EntityBundle;
use crate::world::entity::player::column_view::ColumnViewPlugin;
use crate::world::entity::player::movement::MovementPlugin;
use crate::world::entity::player::player_action::PlayerActionPlugin;
use bevy_app::Plugin;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Changed, Commands, Query, With};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::chunk_view::{PlayerChunkObserver, PlayerViewDistance};
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::world::dimension::{Dimension, InDimension};
use mcrs_network::{ConnectionState, ServerSideConnection};
use mcrs_protocol::entity::player::PlayerSpawnInfo;
use mcrs_protocol::packets::game::clientbound::{ClientboundGameEvent, ClientboundLogin};
use mcrs_protocol::{GameEventKind, VarInt, WritePacket, ident};
use movement::TeleportState;

pub mod ability;
pub mod attribute;
mod column_view;
pub mod movement;
pub mod player_action;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(PlayerActionPlugin);
        app.add_plugins(MovementPlugin);
        app.add_plugins(ColumnViewPlugin);
        app.add_systems(bevy_app::Update, spawn_player);
    }
}

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub teleport_state: TeleportState,
    pub view_distance: PlayerViewDistance,
    pub reposition: Reposition,
    pub abilities: ability::PlayerAbilitiesBundle,
    pub attributes: attribute::PlayerAttributesBundle,
}

fn spawn_player(
    dimensions: Query<(Entity), With<Dimension>>,
    mut query: Query<
        (
            Entity,
            &ClientViewDistance,
            &ConnectionState,
            &mut ServerSideConnection,
        ),
        Changed<ConnectionState>,
    >,
    mut commands: Commands,
) {
    query
        .iter_mut()
        .for_each(|(entity, distance, con_state, mut con)| {
            if *con_state != ConnectionState::Game {
                return;
            }
            let Some(dim) = dimensions.iter().next() else {
                println!("No dimension found! Can't spawn player.");
                return;
            };
            con.write_packet(&ClientboundLogin {
                player_id: entity.index() as i32,
                hardcore: false,
                dimensions: vec![ident!("overworld").into()],
                max_players: VarInt(100),
                chunk_radius: VarInt(12),
                simulation_distance: VarInt(12),
                reduced_debug_info: false,
                show_death_screen: false,
                do_limited_crafting: false,
                player_spawn_info: PlayerSpawnInfo::default(),
                enforces_secure_chat: false,
            });
            con.write_packet(&ClientboundGameEvent {
                game_event: GameEventKind::LevelChunksLoadStart,
            });
            let pos = DVec3::new(0.0, 17.0, 0.0);
            commands.entity(entity).insert((
                PlayerChunkObserver {
                    ..Default::default()
                },
                EntityBundle::new(InDimension(dim))
                    .with_transform(Transform::default().with_translation(pos)),
                PlayerBundle {
                    view_distance: PlayerViewDistance {
                        distance: **distance,
                        vert_distance: **distance,
                    },
                    ..Default::default()
                },
            ));
        });
}
