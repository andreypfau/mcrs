use crate::delta_base::{DeltaBase, Pose};
use crate::encode::record;
use crate::list_packets::{Profile, game_mode_packet};
use crate::list_policy::ListPolicy;
use crate::player_data::PlayerData;
use crate::player_input::PlayerInput;
use crate::player_list::{Clock, Dispatch, Member, PlayerList, ProfileNeed};
use crate::player_spawn::PlayerSpawn;
use bytes::{Bytes, BytesMut};
use glam::DVec3;
use mcrs_engine_muxer::{
    EntityEvent, EntityId, ReplicationCodec, SubjectEvent, SubjectRange, TileBlob, TileDelta,
    ViewerBytes, ViewerId,
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
/// It keeps one delta base for each subject, so every viewer of a tile shares its moves, and the
/// profile entries of each client. Every packet that it gives is a record (see
/// [`record`](crate::record)).
#[derive(Debug, Default)]
pub struct PlayerCodec {
    policy: ListPolicy,
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
    dispatches: Vec<Dispatch>,
}

#[derive(Debug)]
struct Subject {
    viewer: ViewerId,
    uuid: Uuid,
    username: String,
    properties: Vec<Property>,
    base: DeltaBase,
    game_mode: GameMode,
    /// The spawn packet, until the subject next moves.
    add_player: Option<Bytes>,
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
    /// A codec whose player lists show the players that `policy` chooses.
    #[must_use]
    pub fn new(policy: ListPolicy) -> Self {
        Self {
            policy,
            ..Self::default()
        }
    }

    fn owe(&mut self, viewer: ViewerId, packet: Bytes) {
        self.owed.entry(viewer).or_default().push(packet);
    }

    fn join(&mut self, subject: EntityId, spawn: &PlayerSpawn) {
        self.generation = self.generation.wrapping_add(1);
        self.owners.insert(spawn.viewer, subject);
        self.list.join(subject, spawn.viewer.0, spawn.uuid);
        self.subjects.insert(
            subject,
            Subject {
                viewer: spawn.viewer,
                uuid: spawn.uuid,
                username: spawn.username.clone(),
                properties: spawn.properties.clone(),
                base: DeltaBase::new(&spawn.pose, self.tick),
                game_mode: spawn.game_mode,
                add_player: None,
            },
        );
    }

    /// Appends the packets that move `subject` to `pose` to `bytes`.
    fn advance(&mut self, subject: EntityId, pose: &Pose, bytes: &mut BytesMut) -> Result<()> {
        let Some(state) = self.subjects.get_mut(&subject) else {
            return Ok(());
        };
        let start = bytes.len();
        state
            .base
            .advance(wire_id(subject), pose, self.tick, bytes)?;
        if bytes.len() > start {
            state.add_player = None;
        }
        Ok(())
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

    fn encode_events(&mut self, delta: &TileDelta<PlayerData>) -> Result<TileBlob> {
        let mut bytes = BytesMut::new();
        let mut ranges = Vec::new();
        for SubjectEvent { subject, event } in &delta.events {
            let subject = *subject;
            let start = bytes.len();
            match event {
                EntityEvent::Spawn(PlayerData::Spawn(spawn)) => self.join(subject, spawn),
                EntityEvent::Move(PlayerData::Pose(pose)) => {
                    self.advance(subject, pose, &mut bytes)?;
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
                EntityEvent::Enter { .. } => self.generation = self.generation.wrapping_add(1),
                EntityEvent::Despawn => self.despawn(subject)?,
                _ => {}
            }
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

    fn spawn_chunks(
        &mut self,
        viewer: ViewerId,
        subject: EntityId,
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
        let add_player = match &state.add_player {
            Some(packet) => packet.clone(),
            None => {
                let packet = add_player(wire_id(subject), state.uuid, &state.base)?;
                state.add_player.insert(packet).clone()
            }
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
                position: state.base.position,
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
        let result = self
            .list
            .update(clock, self.policy, &roster, sees, &mut self.dispatches);
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

    fn encode(&mut self, tick: u64, delta: &TileDelta<PlayerData>) -> TileBlob {
        self.tick = tick;
        self.encode_events(delta).unwrap_or_else(|error| {
            tracing::warn!("the moves of a tile did not encode: {error:#}");
            TileBlob::default()
        })
    }

    fn spawn(&mut self, viewer: ViewerId, subject: EntityId, chunks: &mut Vec<Bytes>) {
        if let Err(error) = self.spawn_chunks(viewer, subject, chunks) {
            tracing::warn!("a spawn bundle did not encode: {error:#}");
        }
    }

    fn remove(&mut self, viewer: ViewerId, subjects: &[EntityId]) -> Bytes {
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
        record(&ClientboundRemoveEntities { entity_ids }).unwrap_or_else(|error| {
            tracing::warn!("a removal did not encode: {error:#}");
            Bytes::new()
        })
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
        head_yaw: base.head_yaw,
        data: VarInt(0),
    })
}

/// The id on the wire of `subject`, which holds that id in its low 32 bits.
fn wire_id(subject: EntityId) -> VarInt {
    VarInt(subject.0 as u32 as i32)
}
