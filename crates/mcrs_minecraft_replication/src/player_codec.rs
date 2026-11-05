use crate::delta_base::DeltaBase;
use crate::encode::record;
use crate::list_packets::{Profile, game_mode_packet};
use crate::motion::Motion;
use crate::player_data::PlayerData;
use crate::player_input::PlayerInput;
use crate::player_list::{Clock, Dispatch, Member, PlayerList, ProfileNeed};
use crate::player_spawn::PlayerSpawn;
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
use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use uuid::Uuid;

/// The Minecraft side of a [`Muxer`](mcrs_engine_muxer::Muxer) for the players of one world.
/// The [`EntityId`] of a player is its id on the wire.
///
/// It keeps one delta base for each subject and tier, so every viewer of a tier shares its moves,
/// and the profile entries of each client. The spawn bundle is the same at each tier: the profile
/// entry and the spawn, with no equipment or entity data yet. Every packet that it gives is a record (see
/// [`record`](crate::record)).
#[derive(Debug, Default)]
pub struct PlayerCodec {
    list: PlayerList,
    subjects: BTreeMap<EntityId, Subject>,
    /// The own subject of each viewer.
    owners: BTreeMap<ViewerId, EntityId>,
    /// The subjects that each client holds.
    holds: BTreeMap<ViewerId, BTreeSet<EntityId>>,
    /// The viewers whose client holds each subject.
    held_by: BTreeMap<EntityId, BTreeSet<ViewerId>>,
    latency: BTreeMap<ViewerId, i32>,
    tick: u64,
    /// Changes whenever a player joins, leaves or changes tile.
    generation: u64,
    delimiter: Option<Bytes>,
    joined: Vec<Message>,
    owed: BTreeMap<ViewerId, Vec<Bytes>>,
    /// The viewers that left the muxer while their own subject is still in this world. They
    /// get no more bytes from it.
    forgotten: BTreeSet<ViewerId>,
    dispatches: Vec<Dispatch>,
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

impl PlayerCodec {
    fn owe(&mut self, viewer: ViewerId, packet: Bytes) {
        if !self.forgotten.contains(&viewer) {
            self.owed.entry(viewer).or_default().push(packet);
        }
    }

    fn join(&mut self, subject: EntityId, spawn: &PlayerSpawn) {
        self.generation = self.generation.wrapping_add(1);
        self.owners.insert(spawn.viewer, subject);
        self.forgotten.remove(&spawn.viewer);
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
            self.owe(viewer, packet.clone());
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
            self.forgotten.remove(&viewer);
            self.latency.remove(&viewer);
            for held in self.holds.remove(&viewer).unwrap_or_default() {
                if let Some(viewers) = self.held_by.get_mut(&held) {
                    viewers.remove(&viewer);
                }
            }
        }
        for holder in self.held_by.remove(&subject).unwrap_or_default() {
            if let Some(held) = self.holds.get_mut(&holder) {
                held.remove(&subject);
            }
        }
        if let Some(packet) = self.list.leave(subject)? {
            self.owe(viewer, packet);
        }
        Ok(())
    }

    fn apply_events(&mut self, delta: &TileDelta<PlayerData>) -> Result<()> {
        let mut entered = Vec::new();
        for SubjectEvent { subject, event } in &delta.events {
            let subject = *subject;
            match event {
                EntityEvent::Spawn(PlayerData::Spawn(spawn)) => self.join(subject, spawn),
                EntityEvent::Move(PlayerData::Pose(pose)) => {
                    if let Some(state) = self.subjects.get_mut(&subject) {
                        state.motion.record(self.tick, pose);
                    }
                }
                EntityEvent::Data(PlayerData::GameMode(game_mode)) => {
                    self.change_game_mode(subject, *game_mode)?;
                }
                EntityEvent::Data(PlayerData::Joined(packet)) => self.joined.push(Message {
                    subject,
                    packet: packet.clone(),
                }),
                EntityEvent::Data(PlayerData::Left(packet)) => {
                    let holders: Vec<ViewerId> = self
                        .held_by
                        .get(&subject)
                        .into_iter()
                        .flatten()
                        .copied()
                        .collect();
                    for viewer in holders {
                        self.owe(viewer, packet.clone());
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
            if let Some(own) = own {
                self.list.release(own, subject, self.tick);
            }
            if let Some(held) = self.holds.get_mut(&viewer) {
                held.remove(&subject);
            }
            if let Some(viewers) = self.held_by.get_mut(&subject) {
                viewers.remove(&viewer);
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
        let delimiter = match &self.delimiter {
            Some(delimiter) => delimiter.clone(),
            None => self
                .delimiter
                .insert(record(&ClientboundBundleDelimiter)?)
                .clone(),
        };
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
        self.holds.entry(viewer).or_default().insert(subject);
        self.held_by.entry(subject).or_default().insert(viewer);
        Ok(())
    }

    fn update_list(&mut self) -> Result<()> {
        let roster: Vec<Member<'_>> = self
            .subjects
            .iter()
            .map(|(&subject, state)| Member {
                player: subject,
                position: state
                    .motion
                    .latest()
                    .map_or_else(Default::default, |pose| pose.position),
                profile: state.profile(),
                game_mode: state.game_mode,
                latency: self.latency.get(&state.viewer).copied().unwrap_or(0),
            })
            .collect();
        let holds = &self.holds;
        let subjects = &self.subjects;
        let sees = |viewer: EntityId, subject: EntityId| {
            subjects
                .get(&viewer)
                .and_then(|state| holds.get(&state.viewer))
                .is_some_and(|held| held.contains(&subject))
        };
        let clock = Clock {
            tick: self.tick,
            generation: self.generation,
        };
        let result = self.list.update(clock, &roster, sees, &mut self.dispatches);
        for Dispatch { viewer, packet } in mem::take(&mut self.dispatches) {
            if let Some(state) = self.subjects.get(&viewer) {
                let viewer = state.viewer;
                self.owe(viewer, packet);
            }
        }
        result
    }
}

impl ReplicationCodec for PlayerCodec {
    type Data = PlayerData;
    type Input = PlayerInput;

    fn apply(&mut self, tick: u64, delta: &TileDelta<PlayerData>) {
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
            Hold::Gain { subject, tier } => {
                if let Err(error) = self.spawn_chunks(viewer, subject, tier, chunks) {
                    tracing::warn!("a spawn bundle did not encode: {error:#}");
                }
            }
            Hold::Lose { subjects } => chunks.extend(self.release_all(viewer, subjects)),
            Hold::Forget => {
                self.owed.remove(&viewer);
                if self.owners.contains_key(&viewer) {
                    self.forgotten.insert(viewer);
                }
            }
        }
    }

    fn position(&self, subject: EntityId) -> Option<[f64; 3]> {
        let pose = self.subjects.get(&subject)?.motion.latest()?;
        Some(pose.position.to_array())
    }

    fn finish_tick(&mut self, tick: u64, out: &mut Vec<ViewerBytes>) {
        self.tick = tick;
        for Message { subject, packet } in mem::take(&mut self.joined) {
            let own = self.subjects.get(&subject).map(|state| state.viewer);
            let holders: Vec<ViewerId> = self
                .held_by
                .get(&subject)
                .into_iter()
                .flatten()
                .copied()
                .chain(own)
                .collect();
            for viewer in holders {
                self.owe(viewer, packet.clone());
            }
        }
        if let Err(error) = self.update_list() {
            tracing::warn!("the player list did not encode: {error:#}");
        }
        out.extend(
            mem::take(&mut self.owed)
                .into_iter()
                .map(|(viewer, chunks)| ViewerBytes { viewer, chunks }),
        );
    }

    fn receive(&mut self, input: PlayerInput) {
        match input {
            PlayerInput::Latency { viewer, latency } => {
                self.latency.insert(viewer, latency);
            }
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

/// The id on the wire of `subject`: its low 32 bits. The id of a player is odd and the id of
/// any other entity is even, so the two kinds keep apart on the wire too.
#[must_use]
pub fn wire_id(subject: EntityId) -> VarInt {
    VarInt(subject.0 as u32 as i32)
}
