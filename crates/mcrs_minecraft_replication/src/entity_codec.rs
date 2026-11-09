use crate::delivery::Delivery;
use crate::delta_base::DeltaBase;
use crate::encode::record;
use crate::hold_set::HoldSet;
use crate::list_packets::{Profile, game_mode_packet};
use crate::mob_report::MobReport;
use crate::mob_trackers::MobTrackers;
use crate::motion::Motion;
use crate::player_data::PlayerData;
use crate::player_input::PlayerInput;
use crate::player_list::{Dispatch, Member, PlayerList, ProfileNeed};
use crate::player_spawn::PlayerSpawn;
use crate::replication_data::ReplicationData;
use crate::wire_id::wire_id;
use bytes::{Bytes, BytesMut};
use glam::DVec3;
use mcrs_engine_muxer::{
    BlobKind, EntityEvent, EntityId, Hold, ReplicationCodec, SubjectEvent, SubjectRange, Tier,
    TileBlob, TileDelta, ViewerBytes, ViewerId,
};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_profile::Property;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundBundleDelimiter;
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundAddEntity, ClientboundRemoveEntities};
use std::collections::BTreeMap;
use std::mem;
use uuid::Uuid;

/// The Minecraft side of a [`Muxer`](mcrs_engine_muxer::Muxer) for the players and the mobs of
/// one world. The [`EntityId`] of a subject is its id on the wire.
///
/// For a player, it keeps one delta base for each tier, so every viewer of a tier shares its
/// moves, and the profile entries of each client. The spawn bundle of a player is the same at
/// each tier: the profile entry and the spawn, with no equipment or entity data yet. A mob has
/// one tracker that all its viewers share. Every packet that it gives is a record (see
/// [`record`](crate::record)).
#[derive(Debug, Default)]
pub struct EntityCodec {
    list: PlayerList,
    subjects: BTreeMap<EntityId, Subject>,
    /// The own subject of each viewer.
    owners: BTreeMap<ViewerId, EntityId>,
    holds: HoldSet,
    latency: BTreeMap<ViewerId, i32>,
    tick: u64,
    /// Changes whenever a player joins, leaves or changes tile.
    generation: u64,
    delimiter: Option<Bytes>,
    joined: Vec<Message>,
    delivery: Delivery,
    dispatches: Vec<Dispatch>,
    mobs: MobTrackers,
}

#[derive(Debug)]
struct Subject {
    viewer: ViewerId,
    uuid: Uuid,
    username: String,
    properties: Vec<Property>,
    motion: Motion,
    game_mode: GameMode,
}

impl Subject {
    fn profile(&self) -> Profile<'_> {
        Profile {
            id: self.uuid,
            username: &self.username,
            properties: &self.properties,
        }
    }
}

#[derive(Debug)]
struct Message {
    subject: EntityId,
    packet: Bytes,
}

impl EntityCodec {
    fn delimiter(&mut self) -> Result<Bytes> {
        if let Some(delimiter) = &self.delimiter {
            return Ok(delimiter.clone());
        }
        Ok(self
            .delimiter
            .insert(record(&ClientboundBundleDelimiter)?)
            .clone())
    }

    fn join(&mut self, subject: EntityId, spawn: &PlayerSpawn) {
        self.generation = self.generation.wrapping_add(1);
        self.owners.insert(spawn.viewer, subject);
        self.delivery.remember(spawn.viewer);
        self.list.join(subject, spawn.viewer.0, spawn.uuid);
        self.subjects.insert(
            subject,
            Subject {
                viewer: spawn.viewer,
                uuid: spawn.uuid,
                username: spawn.username.clone(),
                properties: spawn.properties.clone(),
                motion: Motion::new(&spawn.pose, self.tick),
                game_mode: spawn.game_mode,
            },
        );
    }

    fn change_game_mode(&mut self, subject: EntityId, game_mode: GameMode) -> Result<()> {
        let Some(state) = self.subjects.get_mut(&subject) else {
            return Ok(());
        };
        if state.game_mode == game_mode {
            return Ok(());
        }
        state.game_mode = game_mode;
        let packet = game_mode_packet(state.uuid, game_mode)?;
        let holders: Vec<ViewerId> = self
            .list
            .holders(subject)
            .filter_map(|holder| self.subjects.get(&holder))
            .map(|holder| holder.viewer)
            .collect();
        for viewer in holders {
            self.delivery.owe(viewer, packet.clone());
        }
        Ok(())
    }

    fn despawn(&mut self, subject: EntityId) -> Result<()> {
        self.generation = self.generation.wrapping_add(1);
        let Some(state) = self.subjects.remove(&subject) else {
            return Ok(());
        };
        let viewer = state.viewer;
        if self.owners.get(&viewer) == Some(&subject) {
            self.owners.remove(&viewer);
            self.delivery.remember(viewer);
            self.latency.remove(&viewer);
            let mobs = &self.mobs;
            self.holds
                .release_where(viewer, |held| !mobs.contains(held));
        }
        self.holds.remove_subject(subject);
        if let Some(packet) = self.list.leave(subject)? {
            self.delivery.owe(viewer, packet);
        }
        Ok(())
    }

    fn apply_events(&mut self, delta: &TileDelta<ReplicationData>) -> Result<()> {
        let mut entered = Vec::new();
        for SubjectEvent { subject, event } in &delta.events {
            let subject = *subject;
            match event {
                EntityEvent::Spawn(ReplicationData::Player(PlayerData::Spawn(spawn))) => {
                    self.join(subject, spawn);
                }
                EntityEvent::Spawn(ReplicationData::Mob(MobReport::Spawn(spawn))) => {
                    self.mobs.spawn(subject, spawn, self.tick);
                }
                EntityEvent::Data(ReplicationData::Mob(MobReport::Signal(signal))) => {
                    self.mobs.signal(subject, *signal, &self.holds)?;
                }
                EntityEvent::Move(ReplicationData::Mob(MobReport::Move(state))) => {
                    self.mobs.record(subject, *state);
                }
                EntityEvent::Enter { .. } | EntityEvent::Despawn if self.mobs.contains(subject) => {
                    if matches!(event, EntityEvent::Despawn) {
                        self.mobs.despawn(subject, &mut self.holds);
                    }
                }
                EntityEvent::Move(ReplicationData::Player(PlayerData::Pose(pose))) => {
                    if let Some(state) = self.subjects.get_mut(&subject) {
                        state.motion.record(self.tick, pose);
                    }
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::GameMode(game_mode))) => {
                    self.change_game_mode(subject, *game_mode)?;
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Joined(packet))) => {
                    self.joined.push(Message {
                        subject,
                        packet: packet.clone(),
                    });
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Left(packet))) => {
                    for &viewer in self.holds.holders(subject) {
                        self.delivery.owe(viewer, packet.clone());
                    }
                }
                EntityEvent::Enter { .. } => {
                    self.generation = self.generation.wrapping_add(1);
                    entered.push(subject);
                }
                EntityEvent::Despawn => self.despawn(subject)?,
                _ => {}
            }
        }
        for subject in entered {
            self.refresh(subject);
        }
        Ok(())
    }

    /// The bytes of `subjects` at `tier`, each written by `write`, with the range of each.
    fn write_blob(
        &mut self,
        subjects: &[EntityId],
        mut write: impl FnMut(&mut Motion, VarInt, &mut BytesMut) -> Result<()>,
    ) -> Result<TileBlob> {
        let mut bytes = BytesMut::new();
        let mut ranges = Vec::new();
        for &subject in subjects {
            let Some(state) = self.subjects.get_mut(&subject) else {
                continue;
            };
            let start = bytes.len();
            write(&mut state.motion, wire_id(subject), &mut bytes)?;
            if bytes.len() > start {
                ranges.push(SubjectRange {
                    subject,
                    range: start..bytes.len(),
                });
            }
        }
        Ok(TileBlob {
            bytes: bytes.freeze(),
            ranges,
        })
    }

    fn release_all(&mut self, viewer: ViewerId, subjects: &[EntityId]) -> Option<Bytes> {
        let own = self.owners.get(&viewer).copied();
        for &subject in subjects {
            self.holds.remove(viewer, subject);
            if self.mobs.lose(viewer, subject) {
                continue;
            }
            if let Some(own) = own {
                self.list.release(own, subject, self.tick);
            }
        }
        let entity_ids: Vec<VarInt> = subjects.iter().map(|&subject| wire_id(subject)).collect();
        match record(&ClientboundRemoveEntities { entity_ids }) {
            Ok(removal) => Some(removal),
            Err(error) => {
                tracing::warn!("a removal did not encode: {error:#}");
                None
            }
        }
    }

    fn spawn_chunks(
        &mut self,
        viewer: ViewerId,
        subject: EntityId,
        tier: Tier,
        chunks: &mut Vec<Bytes>,
    ) -> Result<()> {
        let delimiter = self.delimiter()?;
        let Some(state) = self.subjects.get_mut(&subject) else {
            return Ok(());
        };
        let uuid = state.uuid;
        let Some(add_player) = state
            .motion
            .spawn(tier, |base| add_player(wire_id(subject), uuid, base))?
        else {
            return Ok(());
        };
        let need = match self.owners.get(&viewer) {
            Some(&own) => self.list.acquire(own, subject),
            None => ProfileNeed::Send,
        };
        let profile = match need {
            ProfileNeed::Send => Some(self.list.profile_packet(
                subject,
                &state.profile(),
                state.game_mode,
            )?),
            ProfileNeed::Skip => None,
        };
        // The client applies the packets between two delimiters in one tick, and it does not
        // create a player without its profile entry, so the entry goes first.
        chunks.push(delimiter.clone());
        chunks.extend(profile);
        chunks.push(add_player);
        chunks.push(delimiter);
        self.holds.insert(viewer, subject);
        Ok(())
    }

    fn update_list(&mut self) -> Result<()> {
        let mut roster: Vec<Member<'_>> = Vec::new();
        if self.list.reads_roster(self.tick, self.generation) {
            roster.extend(self.subjects.iter().map(|(&subject, state)| {
                Member {
                    player: subject,
                    position: state
                        .motion
                        .latest()
                        .map_or_else(Default::default, |pose| pose.position),
                    profile: state.profile(),
                    game_mode: state.game_mode,
                    latency: self.latency.get(&state.viewer).copied().unwrap_or(0),
                }
            }));
        }
        let holds = &self.holds;
        let subjects = &self.subjects;
        let sees = |viewer: EntityId, subject: EntityId| {
            subjects
                .get(&viewer)
                .is_some_and(|state| holds.contains(state.viewer, subject))
        };
        let result = self.list.update(
            self.tick,
            self.generation,
            &roster,
            sees,
            &mut self.dispatches,
        );
        let mut dispatches = mem::take(&mut self.dispatches);
        for Dispatch { viewer, packet } in dispatches.drain(..) {
            if let Some(state) = self.subjects.get(&viewer) {
                self.delivery.owe(state.viewer, packet);
            }
        }
        self.dispatches = dispatches;
        result
    }
}

impl ReplicationCodec for EntityCodec {
    type Data = ReplicationData;
    type Input = PlayerInput;

    fn apply(&mut self, tick: u64, delta: &TileDelta<ReplicationData>) {
        self.tick = tick;
        if let Err(error) = self.apply_events(delta) {
            tracing::warn!("the events of a tile did not encode: {error:#}");
        }
    }

    fn blob(&mut self, tick: u64, tier: Tier, subjects: &[EntityId], kind: BlobKind) -> TileBlob {
        match kind {
            BlobKind::Keyframe => self
                .write_blob(subjects, |motion, id, bytes| match motion.base(tier) {
                    Some(base) => base.keyframe(id, bytes),
                    None => Ok(()),
                })
                .unwrap_or_else(|error| {
                    tracing::warn!("a keyframe of a tile did not encode: {error:#}");
                    TileBlob::default()
                }),
            BlobKind::Delta => {
                self.tick = tick;
                self.write_blob(subjects, |motion, id, bytes| {
                    motion.encode(id, tier, tick, bytes)
                })
                .unwrap_or_else(|error| {
                    tracing::warn!("the moves of a tile did not encode: {error:#}");
                    TileBlob::default()
                })
            }
        }
    }

    fn refresh(&mut self, subject: EntityId) {
        if let Some(state) = self.subjects.get_mut(&subject) {
            state.motion.refresh();
        }
    }

    fn hold(&mut self, viewer: ViewerId, change: Hold<'_>, chunks: &mut Vec<Bytes>) {
        match change {
            Hold::Gain { subject, .. } if self.mobs.contains(subject) => {
                let gained = self.delimiter().and_then(|delimiter| {
                    self.mobs.gain(
                        viewer,
                        subject,
                        self.tick,
                        &delimiter,
                        &mut self.holds,
                        chunks,
                    )
                });
                if let Err(error) = gained {
                    tracing::warn!("the spawn bundle of a mob did not encode: {error:#}");
                }
            }
            Hold::Gain { subject, tier } => {
                if let Err(error) = self.spawn_chunks(viewer, subject, tier, chunks) {
                    tracing::warn!("a spawn bundle did not encode: {error:#}");
                }
            }
            Hold::Lose { subjects } => {
                self.mobs.farewell(viewer, subjects, chunks);
                chunks.extend(self.release_all(viewer, subjects));
            }
            Hold::Forget => {
                self.delivery.drop_owed(viewer);
                self.mobs.forget(viewer, &mut self.holds);
                if self.owners.contains_key(&viewer) {
                    self.delivery.forget(viewer);
                }
            }
        }
    }

    fn tracking_range(&self, subject: EntityId) -> Option<u32> {
        self.mobs.tracking_range(subject)
    }

    fn position(&self, subject: EntityId) -> Option<[f64; 3]> {
        if let Some(position) = self.mobs.position(subject) {
            return Some(position);
        }
        let pose = self.subjects.get(&subject)?.motion.latest()?;
        Some(pose.position.to_array())
    }

    fn finish_tick(&mut self, tick: u64, out: &mut Vec<ViewerBytes>) {
        self.tick = tick;
        for Message { subject, packet } in mem::take(&mut self.joined) {
            let own = self.subjects.get(&subject).map(|state| state.viewer);
            for &viewer in self.holds.holders(subject).iter().chain(own.as_ref()) {
                self.delivery.owe(viewer, packet.clone());
            }
        }
        self.mobs.finish_tick(tick, &self.holds, &mut self.delivery);
        if let Err(error) = self.update_list() {
            tracing::warn!("the player list did not encode: {error:#}");
        }
        self.delivery.drain(out);
    }

    fn receive(&mut self, input: PlayerInput) {
        match input {
            PlayerInput::Latency { viewer, latency } => {
                self.latency.insert(viewer, latency);
            }
            PlayerInput::Backlog { viewer, backlog } => self.mobs.set_backlog(viewer, backlog),
        }
    }
}

fn add_player(id: VarInt, uuid: Uuid, base: &DeltaBase) -> Result<Bytes> {
    record(&ClientboundAddEntity {
        id,
        uuid,
        kind: EntityType::Player,
        pos: base.position,
        movement: LpVec3(DVec3::ZERO),
        pitch: base.pitch,
        yaw: base.yaw,
        head_yaw: base.yaw,
        data: VarInt(0),
    })
}
