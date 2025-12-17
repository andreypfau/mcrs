use crate::client_info::ClientViewDistance;
use crate::login::GameProfile;
use crate::world::entity::player::column_view::ColumnViewPlugin;
use crate::world::entity::player::digging::DiggingPlugin;
use crate::world::entity::player::movement::MovementPlugin;
use crate::world::entity::player::player_action::PlayerActionPlugin;
use crate::world::entity::{EntityBundle, MinecraftEntityType};
use bevy_app::Plugin;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Changed, Commands, Query, With};
use bevy_math::DVec3;
use mcrs_engine::entity::EntityNetworkAddEvent;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::{PlayerChunkObserver, PlayerViewDistance};
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::world::dimension::{Dimension, InDimension};
use mcrs_network::{ConnectionState, ServerSideConnection};
use mcrs_protocol::entity::player::PlayerSpawnInfo;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundAddEntity, ClientboundGameEvent, ClientboundLogin, ClientboundPlayerInfoUpdate,
};
use mcrs_protocol::profile::{PlayerListActions, PlayerListEntry};
use mcrs_protocol::{ByteAngle, GameEventKind, GameMode, VarInt, WritePacket, ident};
use movement::TeleportState;

pub mod ability;
pub mod attribute;
mod column_view;
pub mod digging;
pub mod movement;
pub mod player_action;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(DiggingPlugin);
        app.add_plugins(PlayerActionPlugin);
        app.add_plugins(MovementPlugin);
        app.add_plugins(ColumnViewPlugin);
        app.add_systems(bevy_app::Update, spawn_player);
        app.add_observer(network_add);
        app.add_observer(player_joined);
    }
}

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub teleport_state: TeleportState,
    pub view_distance: PlayerViewDistance,
    pub reposition: Reposition,
    pub abilities: ability::PlayerAbilitiesBundle,
    pub attributes: attribute::PlayerAttributesBundle,
    pub marker: Player,
}

fn spawn_player(
    dimensions: Query<(Entity), With<Dimension>>,
    mut query: Query<
        (
            Entity,
            &ClientViewDistance,
            &ConnectionState,
            &GameProfile,
            &mut ServerSideConnection,
        ),
        Changed<ConnectionState>,
    >,
    mut commands: Commands,
) {
    query
        .iter_mut()
        .for_each(|(entity, distance, con_state, profile, mut con)| {
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
                    .with_uuid(profile.id)
                    .with_transform(Transform::default().with_translation(pos)),
                PlayerBundle {
                    view_distance: PlayerViewDistance {
                        distance: **distance,
                        vert_distance: **distance,
                    },
                    ..Default::default()
                },
            ));
            commands.trigger(PlayerJoinEvent { player: entity });
        });
}

#[derive(EntityEvent)]
pub struct PlayerJoinEvent {
    #[event_target]
    pub player: Entity,
}

fn network_add(
    event: On<EntityNetworkAddEvent>,
    added_player: Query<(Entity, &GameProfile, &Transform), With<Player>>,
    mut player: Query<(&mut ServerSideConnection, &Reposition, &GameProfile), With<Player>>,
) {
    let Ok((entity, profile, transform)) = added_player.get(event.entity) else {
        return;
    };
    let Ok((mut connection, reposition, viewer_profile)) = player.get_mut(event.player) else {
        return;
    };

    let pkt = ClientboundAddEntity {
        id: VarInt(entity.index() as i32),
        uuid: profile.id,
        kind: VarInt(MinecraftEntityType::Player as i32),
        pos: reposition.convert_dvec3(transform.translation),
        velocity: VarInt(0),
        yaw: ByteAngle::from_degrees(transform.rotation.y),
        pitch: ByteAngle::from_degrees(transform.rotation.x),
        head_yaw: ByteAngle::from_degrees(transform.rotation.y),
        data: VarInt(0),
    };
    connection.write_packet(&pkt);
    println!(
        "send player {:?} add entity for player viewer: {:?}",
        profile.username, viewer_profile.username
    );
}

fn player_joined(
    event: On<PlayerJoinEvent>,
    mut players: Query<(&mut ServerSideConnection, &GameProfile)>,
) {
    let Ok((_, joined_player)) = players.get(event.player) else {
        return;
    };
    println!("Player {:?} has joined the game.", joined_player.username);

    let names = players
        .iter()
        .map(|(_, profile)| profile.username.clone())
        .collect::<Vec<_>>();
    let entries: Vec<PlayerListEntry> = players
        .iter()
        .zip(names.iter())
        .map(|((_, profile), name)| PlayerListEntry {
            username: name.as_str(),
            player_uuid: profile.id,
            properties: profile.properties.iter().cloned().collect(),
            listed: true,
            game_mode: GameMode::Survival,
            ..Default::default()
        })
        .collect();

    let pkt = ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new()
            .with_add_player(true)
            .with_update_game_mode(true)
            .with_update_listed(true),
        entries: entries.into(),
    };

    players
        .iter_mut()
        .for_each(|(mut connection, _)| connection.write_packet(&pkt));
}
