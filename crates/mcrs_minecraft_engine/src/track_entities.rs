//! The per-dim system that keeps the [`EntityTracker`] in line with the players of the world and
//! sends each viewer the spawns, moves and removes of its visible set.

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, SpawnBundle, to, to_target};
use crate::entity_motion::{DeltaBase, Keyframe, Motion};
use crate::entity_tracker::{EntityTracker, TrackCell, TrackedSubject, ViewPair};
use crate::game_mode::ListedGameMode;
use crate::network_id::player_network_id;
use crate::player_list::{PlayerList, ProfileNeed};
use crate::session::GameProfile;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{DetectChanges, Entity, Query, Ref, ResMut, With};
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundAddEntity, ClientboundRemoveEntities};
use std::collections::BTreeMap;

/// A moving subject sends its absolute position at least this often, in ticks, so a client that
/// lost a relative move under the drop policy of its queue comes back to the right place.
const KEYFRAME_INTERVAL_TICKS: u64 = 100;

/// A player that the tracker does not hold yet.
struct Arrival {
    entity: Entity,
    cell: TrackCell,
    session: PlayerSession,
    transform: Transform,
}

/// A tracked player that moved to another cell.
struct Relocation {
    entity: Entity,
    cell: TrackCell,
}

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::track_entities", skip_all)
)]
pub fn track_entities(
    mut tracker: ResMut<EntityTracker>,
    mut list: ResMut<PlayerList>,
    players: Query<(Entity, &InDimension, Ref<Transform>, &Owner), With<Player>>,
    profiles: Query<(Option<&GameProfile>, Option<&ListedGameMode>)>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    tracker.advance_tick();
    let tick = tracker.tick();
    let mut arrivals: Vec<Arrival> = Vec::new();
    let mut relocations: Vec<Relocation> = Vec::new();
    for (entity, in_dimension, transform, &Owner(session)) in &players {
        let cell = TrackCell {
            dimension: in_dimension.0,
            section: transform.translation.into(),
        };
        let Some(subject) = tracker.get_mut(entity) else {
            arrivals.push(Arrival {
                entity,
                cell,
                session,
                transform: *transform,
            });
            continue;
        };
        if !transform.is_changed() {
            continue;
        }
        let keyframe = if tick.wrapping_sub(subject.keyframe_tick) >= KEYFRAME_INTERVAL_TICKS {
            Keyframe::Due
        } else {
            Keyframe::NotDue
        };
        let motion = subject.base.advance(subject.wire_id, &transform, keyframe);
        if matches!(motion.body, Some(PacketPayload::EntityPosSync(_))) {
            subject.keyframe_tick = tick;
        }
        if subject.cell != cell {
            relocations.push(Relocation { entity, cell });
        }
        send_motion(&tracker, entity, motion, &mut writer);
    }
    for Relocation { entity, cell } in relocations {
        tracker.relocate(entity, cell);
    }
    for arrival in arrivals {
        let mut subject = TrackedSubject::new(
            arrival.cell,
            arrival.session,
            VarInt(player_network_id(arrival.session)),
            DeltaBase::new(&arrival.transform),
        );
        subject.keyframe_tick = tick;
        tracker.insert(arrival.entity, subject);
        if let Ok((Some(profile), _)) = profiles.get(arrival.entity) {
            list.join(arrival.entity, arrival.session, profile.id);
        }
    }
    send_removes(&mut tracker, &mut list, &mut writer);
    send_spawns(&mut tracker, &mut list, &profiles, &mut writer);
}

/// Removes a player that despawns, or stops being a player, from the tracker and from the
/// player list.
pub fn forget_removed_player(
    remove: On<Remove, Player>,
    mut tracker: ResMut<EntityTracker>,
    mut list: ResMut<PlayerList>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let player = remove.event().entity;
    tracker.forget(player);
    if let Some(packet) = list.leave(player) {
        writer.write(packet);
    }
}

/// Sends `motion` of `subject` once to all its viewers.
fn send_motion(
    tracker: &EntityTracker,
    subject: Entity,
    motion: Motion,
    writer: &mut MessageWriter<OutboundPlayerPacket>,
) {
    if motion.is_empty() {
        return;
    }
    let viewers = tracker.viewer_sessions(subject);
    if viewers.is_empty() {
        return;
    }
    for payload in [motion.body, motion.head].into_iter().flatten() {
        writer.write(to_target(PacketTarget::PlayerSet(viewers.clone()), payload));
    }
}

/// Sends each viewer one remove packet with every subject that it lost.
fn send_removes(
    tracker: &mut EntityTracker,
    list: &mut PlayerList,
    writer: &mut MessageWriter<OutboundPlayerPacket>,
) {
    let tick = tracker.tick();
    let mut by_viewer: BTreeMap<Entity, Vec<VarInt>> = BTreeMap::new();
    for (ViewPair { viewer, subject }, wire_id) in tracker.take_left() {
        list.release(viewer, subject, tick);
        by_viewer.entry(viewer).or_default().push(wire_id);
    }
    for (viewer, entity_ids) in by_viewer {
        let Some(viewer) = tracker.get(viewer) else {
            continue;
        };
        writer.write(to(
            viewer.session,
            PacketPayload::RemoveEntities(ClientboundRemoveEntities { entity_ids }),
        ));
    }
}

/// Sends each viewer the spawn bundle of every subject that it gained.
fn send_spawns(
    tracker: &mut EntityTracker,
    list: &mut PlayerList,
    profiles: &Query<(Option<&GameProfile>, Option<&ListedGameMode>)>,
    writer: &mut MessageWriter<OutboundPlayerPacket>,
) {
    for ViewPair { viewer, subject } in tracker.take_entered() {
        let (Some(viewer_subject), Some(tracked)) = (tracker.get(viewer), tracker.get(subject))
        else {
            continue;
        };
        let (profile, game_mode) = profiles.get(subject).unwrap_or_default();
        let profile_packet = match (profile, list.acquire(viewer, subject)) {
            (Some(profile), ProfileNeed::Send) => list.profile_packet(
                subject,
                profile,
                game_mode.map(|mode| mode.0).unwrap_or_default(),
            ),
            (_, ProfileNeed::Skip) | (None, ProfileNeed::Send) => None,
        };
        writer.write(to(
            viewer_subject.session,
            PacketPayload::SpawnBundle(spawn_bundle(
                tracked,
                profile.map(|profile| profile.id),
                profile_packet,
            )),
        ));
    }
}

fn spawn_bundle(
    subject: &TrackedSubject,
    uuid: Option<Uuid>,
    profile: Option<Bytes>,
) -> SpawnBundle {
    let base = subject.base;
    SpawnBundle {
        profile,
        entity: ClientboundAddEntity {
            id: subject.wire_id,
            uuid: uuid.unwrap_or(Uuid::nil()),
            kind: EntityType::Player,
            pos: base.position,
            movement: LpVec3(DVec3::ZERO),
            pitch: base.pitch,
            yaw: base.yaw,
            head_yaw: base.head_yaw,
            data: VarInt(0),
        },
    }
}
