//! The per-dim systems that keep the tile of each player and record its changes as tile events:
//! a spawn, a move, a change of tile or of game mode, a message, a despawn.

use bevy_ecs::component::Component;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, DetectChanges, Entity, Has, Query, Ref, ResMut, With, Without};
use bytes::Bytes;
use mcrs_engine_muxer::{EntityEvent, EntityId, TileKey, ViewerId};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_replication::{PlayerData, PlayerSpawn, Pose};
use mcrs_minecraft_text::{Color, IntoText, RawText, Text};

use crate::cell_crossing::CrossedAt;
use crate::game_mode::ListedGameMode;
use crate::game_profile::GameProfile;
use crate::hot_ticket::CellCrossing;
use crate::network_id::player_network_id;
use crate::replicated::Replicated;
use crate::tile_events::TileEvents;

/// Marks a player that logged in and that its neighbours have not heard of yet. A player that
/// comes from another dimension does not carry it.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AnnounceJoin;

/// The id of the player of `session` in the tile deltas: its id on the wire.
#[must_use]
pub fn player_entity_id(session: PlayerSession) -> EntityId {
    EntityId(u64::from(player_network_id(session) as u32))
}

fn pose(transform: &Transform) -> Pose {
    Pose {
        position: transform.translation,
        look: transform.rotation.into(),
    }
}

/// Records the spawn of each new player, and the move, the change of tile and the change of
/// game mode of each player that has a tile.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::publish_player_tiles", skip_all)
)]
pub fn publish_player_tiles(
    mut events: ResMut<TileEvents>,
    dimensions: Query<(), With<DimensionTypeConfig>>,
    mut players: Query<
        (
            Entity,
            &Owner,
            Ref<Transform>,
            &InDimension,
            Option<&mut Replicated>,
            Option<&GameProfile>,
            Option<&ListedGameMode>,
            Has<AnnounceJoin>,
            Option<&CrossedAt>,
        ),
        With<Player>,
    >,
    mut commands: Commands,
) {
    for (
        player,
        &Owner(session),
        transform,
        dimension,
        replicated,
        profile,
        game_mode,
        announce,
        crossed,
    ) in &mut players
    {
        if crossed.is_some() {
            commands.entity(player).remove::<CrossedAt>();
        }
        if !dimensions.contains(dimension.0) {
            continue;
        }
        let tile = TileKey::at(transform.translation);
        let game_mode = game_mode.map(|mode| mode.0).unwrap_or_default();
        let Some(mut replicated) = replicated else {
            let id = player_entity_id(session);
            events.push(
                tile,
                id,
                EntityEvent::Spawn(
                    PlayerData::Spawn(PlayerSpawn {
                        viewer: ViewerId(session.0),
                        uuid: profile.map_or(Uuid::nil(), |profile| profile.id),
                        username: profile
                            .map(|profile| profile.username.clone())
                            .unwrap_or_default(),
                        properties: profile
                            .map(|profile| profile.properties.clone())
                            .unwrap_or_default(),
                        pose: pose(&transform),
                        game_mode,
                    })
                    .into(),
                ),
            );
            if announce {
                commands.entity(player).remove::<AnnounceJoin>();
                if let Some(message) =
                    profile.and_then(|profile| presence(profile, "multiplayer.player.joined"))
                {
                    events.push(
                        tile,
                        id,
                        EntityEvent::Data(PlayerData::Joined(message).into()),
                    );
                }
            }
            commands.entity(player).insert(Replicated {
                id,
                tile,
                game_mode,
            });
            continue;
        };
        let id = replicated.id;
        let moved = crossed.is_none_or(|crossed| crossed.0 != *transform);
        if transform.is_changed() && moved {
            let from = replicated.tile;
            if from != tile {
                events.push(from, id, EntityEvent::Leave { to: tile });
                events.push(tile, id, EntityEvent::Enter { from });
                replicated.tile = tile;
            }
            events.push(
                tile,
                id,
                EntityEvent::Move(PlayerData::Pose(pose(&transform)).into()),
            );
        }
        if replicated.game_mode != game_mode {
            replicated.game_mode = game_mode;
            events.push(
                tile,
                id,
                EntityEvent::Data(PlayerData::GameMode(game_mode).into()),
            );
        }
    }
}

/// Records the despawn of a player that leaves the world, or stops being published.
pub fn publish_player_despawn(
    remove: On<Remove<Replicated>>,
    players: Query<&Replicated, Without<CellCrossing>>,
    mut events: ResMut<TileEvents>,
) {
    if let Ok(replicated) = players.get(remove.event().entity) {
        events.push(replicated.tile, replicated.id, EntityEvent::Despawn);
    }
}

/// Records the message that the player of `replicated` left, for the viewers that hold it.
pub fn publish_leave_message(
    events: &mut TileEvents,
    replicated: &Replicated,
    profile: &GameProfile,
) {
    if let Some(message) = presence(profile, "multiplayer.player.left") {
        events.push(
            replicated.tile,
            replicated.id,
            EntityEvent::Data(PlayerData::Left(message).into()),
        );
    }
}

/// The yellow system message `key` about `profile`, as a record.
fn presence(profile: &GameProfile, key: &'static str) -> Option<Bytes> {
    let text: Text =
        Text::translate(key, vec![profile.username.clone().into_text()]).color(Color::YELLOW);
    let content = RawText::from_text(&text).ok()?;
    mcrs_minecraft_replication::record(&ClientboundSystemChatPacket {
        content,
        overlay: false,
    })
    .ok()
}
