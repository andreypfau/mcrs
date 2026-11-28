use crate::delivery::Delivery;
use crate::delta_base::DeltaBase;
use crate::encode::record;
use crate::hold_set::HoldSet;
use crate::list_packets::{Profile, game_mode_packet};
use crate::mob_report::MobReport;
use crate::mob_signal::MobSignal;
use crate::mob_trackers::MobTrackers;
use crate::motion::Motion;
use crate::player_data::PlayerData;
use crate::player_entries::PlayerEntries;
use crate::player_input::PlayerInput;
use crate::player_list::{Dispatch, Member, PlayerList, ProfileNeed};
use crate::player_spawn::PlayerSpawn;
use crate::replication_data::ReplicationData;
use crate::subject_attributes::SubjectAttributes;
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
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundRemoveEntities, ClientboundSetEntityData,
    ClientboundSwingAnimation,
};
use mcrs_minecraft_protocol_game::entity::Metadata;
use mcrs_minecraft_protocol_game::{Hand, LpVec3, SwingAnimationKind};
use std::collections::BTreeMap;
use std::mem;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct EntityCodec {
    list: PlayerList,
    subjects: BTreeMap<EntityId, Subject>,
    owners: BTreeMap<ViewerId, EntityId>,
    holds: HoldSet,
    latency: BTreeMap<ViewerId, i32>,
    tick: u64,
    generation: u64,
    delimiter: Option<Bytes>,
    joined: Vec<Message>,
    delivery: Delivery,
    dispatches: Vec<Dispatch>,
    mobs: MobTrackers,
    attributed: Vec<EntityId>,
    posed: Vec<EntityId>,
    swings: Vec<PendingSwing>,
    signals: Vec<PendingSignal>,
    hidden: BTreeMap<EntityId, Vec<ViewerId>>,
    hiding: Vec<EntityId>,
}

#[derive(Clone, Copy, Debug)]
struct PendingSignal {
    subject: EntityId,
    signal: MobSignal,
}

#[derive(Clone, Copy, Debug)]
struct PendingSwing {
    subject: EntityId,
    hand: Hand,
    animation: SwingAnimationKind,
    duration: i32,
}

#[derive(Debug)]
struct Subject {
    viewer: ViewerId,
    uuid: Uuid,
    username: String,
    properties: Vec<Property>,
    motion: Motion,
    game_mode: GameMode,
    attributes: SubjectAttributes,
    entries: PlayerEntries,
    sent_entries: PlayerEntries,
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
                attributes: SubjectAttributes::with_stored(
                    mcrs_minecraft_entity::attribute_bases(EntityType::Player),
                    spawn.attributes.clone(),
                ),
                entries: spawn.entries,
                sent_entries: spawn.entries,
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
        if self.hidden.get(&subject).is_some_and(Vec::is_empty) {
            self.hidden.remove(&subject);
        }
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
                EntityEvent::Data(ReplicationData::Mob(MobReport::Stack(stack))) => {
                    self.mobs.record_stack(subject, stack);
                }
                EntityEvent::Data(ReplicationData::Mob(MobReport::Entries(entries))) => {
                    self.mobs.record_entries(subject, entries);
                }
                EntityEvent::Data(ReplicationData::Mob(MobReport::Attribute(attribute))) => {
                    self.mobs.record_attribute(subject, attribute);
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
                EntityEvent::Data(ReplicationData::Player(PlayerData::Attribute(attribute))) => {
                    if let Some(state) = self.subjects.get_mut(&subject) {
                        state.attributes.record(attribute);
                        self.attributed.push(subject);
                    }
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Entries(entries))) => {
                    if let Some(state) = self.subjects.get_mut(&subject) {
                        state.entries = *entries;
                        self.posed.push(subject);
                    }
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Left(packet))) => {
                    for &viewer in self.holds.holders(subject) {
                        self.delivery.owe(viewer, packet.clone());
                    }
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Swing {
                    hand,
                    animation,
                    duration,
                })) => {
                    self.swings.push(PendingSwing {
                        subject,
                        hand: *hand,
                        animation: *animation,
                        duration: *duration,
                    });
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Signal(signal))) => {
                    self.signals.push(PendingSignal {
                        subject,
                        signal: *signal,
                    });
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Hide)) => {
                    self.hide(subject);
                }
                EntityEvent::Data(ReplicationData::Player(PlayerData::Show)) => {
                    self.show(subject)?;
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

    fn send_signals(&mut self) {
        let Self {
            subjects,
            holds,
            delivery,
            signals,
            ..
        } = self;
        for PendingSignal { subject, signal } in signals.drain(..) {
            let Some(owner) = subjects.get(&subject).map(|state| state.viewer) else {
                continue;
            };
            let to_owner = matches!(signal, MobSignal::Hurt { .. });
            let observers = holds.holders(subject);
            if !to_owner && !observers.iter().any(|&viewer| viewer != owner) {
                continue;
            }
            let packet = match signal.encode(subject) {
                Ok(Some(packet)) => packet,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!("a player signal did not encode: {error:#}");
                    continue;
                }
            };
            for &viewer in observers {
                if viewer != owner {
                    delivery.owe(viewer, packet.clone());
                }
            }
            if to_owner {
                delivery.owe(owner, packet);
            }
        }
    }

    fn hide(&mut self, subject: EntityId) {
        if self.subjects.contains_key(&subject) && !self.hidden.contains_key(&subject) {
            self.hidden.insert(subject, Vec::new());
            self.hiding.push(subject);
        }
    }

    fn show(&mut self, subject: EntityId) -> Result<()> {
        let Some(watchers) = self.hidden.remove(&subject) else {
            return Ok(());
        };
        if let Some(state) = self.subjects.get_mut(&subject) {
            state.motion.refresh();
        }
        let mut chunks = Vec::new();
        for viewer in watchers {
            self.spawn_chunks(viewer, subject, Tier::NEAREST, &mut chunks)?;
            for chunk in chunks.drain(..) {
                self.delivery.owe(viewer, chunk);
            }
        }
        Ok(())
    }

    fn send_hides(&mut self) {
        let Self {
            hidden,
            hiding,
            holds,
            owners,
            list,
            delivery,
            tick,
            ..
        } = self;
        for subject in hiding.drain(..) {
            let Some(watchers) = hidden.get_mut(&subject) else {
                continue;
            };
            let removed = holds.remove_subject(subject);
            if removed.is_empty() {
                continue;
            }
            watchers.extend_from_slice(&removed);
            let removal = match record(&ClientboundRemoveEntities {
                entity_ids: vec![wire_id(subject)],
            }) {
                Ok(removal) => removal,
                Err(error) => {
                    tracing::warn!("the removal of a hidden player did not encode: {error:#}");
                    continue;
                }
            };
            for &viewer in &removed {
                if let Some(&own) = owners.get(&viewer) {
                    list.release(own, subject, *tick);
                }
                delivery.owe(viewer, removal.clone());
            }
        }
    }

    fn send_swings(&mut self) {
        let Self {
            subjects,
            holds,
            delivery,
            swings,
            ..
        } = self;
        for swing in swings.drain(..) {
            let Some(owner) = subjects.get(&swing.subject).map(|state| state.viewer) else {
                continue;
            };
            let observers = holds.holders(swing.subject);
            if !observers.iter().any(|&viewer| viewer != owner) {
                continue;
            }
            let packet = match record(&ClientboundSwingAnimation {
                entity_id: wire_id(swing.subject),
                hand: swing.hand,
                animation: swing.animation,
                duration: VarInt(swing.duration),
            }) {
                Ok(packet) => packet,
                Err(error) => {
                    tracing::warn!("a player swing did not encode: {error:#}");
                    continue;
                }
            };
            for &viewer in observers {
                if viewer != owner {
                    delivery.owe(viewer, packet.clone());
                }
            }
        }
    }

    fn write_blob(
        &mut self,
        subjects: &[EntityId],
        mut write: impl FnMut(&mut Motion, VarInt, &mut BytesMut) -> Result<()>,
    ) -> Result<TileBlob> {
        let mut bytes = BytesMut::new();
        let mut ranges = Vec::new();
        for &subject in subjects {
            if self.hidden.contains_key(&subject) {
                continue;
            }
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

    fn is_hidden_from(&self, viewer: ViewerId, subject: EntityId) -> bool {
        self.hidden.contains_key(&subject) && !self.holds.contains(viewer, subject)
    }

    fn release_all(&mut self, viewer: ViewerId, subjects: &[EntityId]) -> Option<Bytes> {
        let own = self.owners.get(&viewer).copied();
        let entity_ids: Vec<VarInt> = subjects
            .iter()
            .filter(|&&subject| !self.is_hidden_from(viewer, subject))
            .map(|&subject| wire_id(subject))
            .collect();
        for &subject in subjects {
            let hidden = self.is_hidden_from(viewer, subject);
            self.holds.remove(viewer, subject);
            if hidden {
                if let Some(watchers) = self.hidden.get_mut(&subject) {
                    watchers.retain(|&watcher| watcher != viewer);
                    if watchers.is_empty() && !self.subjects.contains_key(&subject) {
                        self.hidden.remove(&subject);
                    }
                }
                continue;
            }
            if self.mobs.lose(viewer, subject) {
                continue;
            }
            if let Some(own) = own {
                self.list.release(own, subject, self.tick);
            }
        }
        if entity_ids.is_empty() {
            return None;
        }
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
        chunks.push(delimiter.clone());
        chunks.extend(profile);
        chunks.push(add_player);
        let entries = state.entries.changed_from(&PlayerEntries::default());
        if !entries.is_empty() {
            chunks.push(record(&ClientboundSetEntityData {
                entity_id: wire_id(subject),
                metadata: Metadata(entries),
            })?);
        }
        let mut attributes = BytesMut::new();
        state
            .attributes
            .write_spawn(wire_id(subject), &mut attributes)?;
        if !attributes.is_empty() {
            chunks.push(attributes.freeze());
        }
        chunks.push(delimiter);
        self.holds.insert(viewer, subject);
        Ok(())
    }

    fn send_attributes(&mut self) {
        let Self {
            attributed,
            subjects,
            holds,
            delivery,
            ..
        } = self;
        for subject in attributed.drain(..) {
            let Some(state) = subjects.get_mut(&subject) else {
                continue;
            };
            let mut bytes = BytesMut::new();
            if let Err(error) = state.attributes.write_unsent(wire_id(subject), &mut bytes) {
                tracing::warn!("the attributes of a player did not encode: {error:#}");
                continue;
            }
            state.attributes.mark_sent();
            if bytes.is_empty() {
                continue;
            }
            let packet = bytes.freeze();
            for &viewer in holds.holders(subject) {
                delivery.owe(viewer, packet.clone());
            }
        }
    }

    fn send_entries(&mut self) {
        let Self {
            posed,
            subjects,
            holds,
            delivery,
            ..
        } = self;
        for subject in posed.drain(..) {
            let Some(state) = subjects.get_mut(&subject) else {
                continue;
            };
            let changed = state.entries.changed_from(&state.sent_entries);
            state.sent_entries = state.entries;
            if changed.is_empty() {
                continue;
            }
            let packet = match record(&ClientboundSetEntityData {
                entity_id: wire_id(subject),
                metadata: Metadata(changed),
            }) {
                Ok(packet) => packet,
                Err(error) => {
                    tracing::warn!("the pose entries of a player did not encode: {error:#}");
                    continue;
                }
            };
            for &viewer in holds.holders(subject) {
                delivery.owe(viewer, packet.clone());
            }
        }
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
            Hold::Gain { subject, .. } if self.hidden.contains_key(&subject) => {
                if let Some(watchers) = self.hidden.get_mut(&subject) {
                    watchers.push(viewer);
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
                let subjects = &self.subjects;
                self.hidden.retain(|subject, watchers| {
                    watchers.retain(|&watcher| watcher != viewer);
                    !watchers.is_empty() || subjects.contains_key(subject)
                });
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
        self.send_signals();
        self.send_attributes();
        self.send_entries();
        self.send_swings();
        self.send_hides();
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

#[cfg(test)]
mod tests {
    use mcrs_engine_muxer::TileKey;
    use mcrs_minecraft_entity::EntityTypeData;
    use mcrs_minecraft_protocol_game::ProtoStack;
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundSetEntityData, ClientboundTakeItemEntity,
    };
    use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry};
    use mcrs_minecraft_registries::Wire;
    use mcrs_minecraft_wire::Decode;

    use super::*;
    use crate::mob_signal::MobSignal;
    use crate::mob_spawn::MobSpawn;
    use crate::mob_state::MobState;
    use crate::spawn_look::SpawnLook;

    const ITEM: EntityId = EntityId(7);
    const PLAYER: EntityId = EntityId(3);
    const VIEWER: ViewerId = ViewerId(1);

    fn stack(count: u8) -> ProtoStack<Wire> {
        ProtoStack::decode(&mut &[count, 1, 0, 0][..]).unwrap()
    }

    fn apply(codec: &mut EntityCodec, tick: u64, events: Vec<EntityEvent<ReplicationData>>) {
        let delta = TileDelta {
            tile: TileKey::new(0, 0, 0),
            events: events
                .into_iter()
                .map(|event| SubjectEvent {
                    subject: ITEM,
                    event,
                })
                .collect(),
        };
        codec.apply(tick, &delta);
    }

    #[test]
    fn ien_net_001_004_item_spawns_with_stack_and_pickup_precedes_removal() {
        let mut codec = EntityCodec::default();
        let state = MobState {
            position: DVec3::new(0.5, -60.0, 0.5),
            on_ground: true,
            ..MobState::default()
        };
        apply(
            &mut codec,
            0,
            vec![EntityEvent::Spawn(
                MobReport::Spawn(MobSpawn {
                    kind: &EntityTypeData::ITEM,
                    uuid: Uuid::nil(),
                    state,
                    look: SpawnLook::Item(stack(3)),
                })
                .into(),
            )],
        );

        let mut spawn = Vec::new();
        codec.hold(
            VIEWER,
            Hold::Gain {
                subject: ITEM,
                tier: Tier::NEAREST,
            },
            &mut spawn,
        );
        let id = wire_id(ITEM);
        let add = record(&ClientboundAddEntity {
            id,
            uuid: Uuid::nil(),
            kind: EntityType::Item,
            pos: state.position,
            movement: LpVec3(DVec3::ZERO),
            pitch: Default::default(),
            yaw: Default::default(),
            head_yaw: Default::default(),
            data: VarInt(0),
        })
        .unwrap();
        let data = record(&ClientboundSetEntityData {
            entity_id: id,
            metadata: Metadata(vec![MetadataEntry {
                index: 8,
                value: MetaDataValue::Slot(stack(3)),
            }]),
        })
        .unwrap();
        assert_eq!(
            spawn[1..3],
            [add, data],
            "the bundle holds add_entity of the type item, then the entity data of the stack"
        );
        assert_eq!(codec.tracking_range(ITEM), Some(6 * 16), "IEN-NET-002");

        apply(
            &mut codec,
            1,
            vec![
                EntityEvent::Data(
                    MobReport::Signal(MobSignal::Taken {
                        collector: PLAYER,
                        amount: 3,
                    })
                    .into(),
                ),
                EntityEvent::Despawn,
            ],
        );
        let mut removal = Vec::new();
        codec.hold(VIEWER, Hold::Lose { subjects: &[ITEM] }, &mut removal);
        let take = record(&ClientboundTakeItemEntity {
            item_id: id,
            player_id: wire_id(PLAYER),
            amount: VarInt(3),
        })
        .unwrap();
        let remove = record(&ClientboundRemoveEntities {
            entity_ids: vec![id],
        })
        .unwrap();
        assert_eq!(removal, [take, remove], "take_item_entity goes first");
    }

    const OBSERVER_A: ViewerId = ViewerId(10);
    const OBSERVER_B: ViewerId = ViewerId(11);
    const LATE_VIEWER: ViewerId = ViewerId(12);
    const SUBJECT_VIEWER: ViewerId = ViewerId(13);

    fn setup_swing_codec() -> EntityCodec {
        use crate::delta_base::Pose;

        let mut codec = EntityCodec::default();
        let spawn = PlayerSpawn {
            viewer: SUBJECT_VIEWER,
            uuid: Uuid::from_u128(99),
            username: "Alice".to_owned(),
            properties: Vec::new(),
            pose: Pose {
                position: DVec3::new(0.5, 64.0, 0.5),
                look: Default::default(),
            },
            game_mode: GameMode::Survival,
            attributes: Vec::new(),
            entries: PlayerEntries::default(),
        };
        codec.apply(
            0,
            &TileDelta {
                tile: TileKey::new(0, 0, 0),
                events: vec![SubjectEvent {
                    subject: PLAYER,
                    event: EntityEvent::Spawn(PlayerData::Spawn(spawn).into()),
                }],
            },
        );
        let mut out = Vec::new();
        for viewer in [OBSERVER_A, OBSERVER_B, SUBJECT_VIEWER] {
            codec.hold(
                viewer,
                Hold::Gain {
                    subject: PLAYER,
                    tier: Tier::NEAREST,
                },
                &mut out,
            );
        }
        let mut drained = Vec::new();
        codec.finish_tick(0, &mut drained);
        codec
    }

    fn finish_by_viewer(codec: &mut EntityCodec, tick: u64) -> BTreeMap<ViewerId, Vec<Bytes>> {
        let mut finished = Vec::new();
        codec.finish_tick(tick, &mut finished);
        let mut by_viewer: BTreeMap<ViewerId, Vec<Bytes>> = BTreeMap::new();
        for ViewerBytes { viewer, chunks } in finished {
            by_viewer.entry(viewer).or_default().extend(chunks);
        }
        by_viewer
    }

    fn swing_event(
        hand: Hand,
        animation: SwingAnimationKind,
        duration: i32,
    ) -> SubjectEvent<ReplicationData> {
        SubjectEvent {
            subject: PLAYER,
            event: EntityEvent::Data(
                PlayerData::Swing {
                    hand,
                    animation,
                    duration,
                }
                .into(),
            ),
        }
    }

    #[test]
    fn player_swings_share_one_encoded_buffer_across_observers_only_and_never_replay() {
        let mut codec = setup_swing_codec();
        codec.apply(
            1,
            &TileDelta {
                tile: TileKey::new(0, 0, 0),
                events: vec![swing_event(Hand::Off, SwingAnimationKind::Stab, 9)],
            },
        );
        let packets_by_viewer = finish_by_viewer(&mut codec, 1);
        let expected = record(&ClientboundSwingAnimation {
            entity_id: wire_id(PLAYER),
            hand: Hand::Off,
            animation: SwingAnimationKind::Stab,
            duration: VarInt(9),
        })
        .unwrap();

        let observer_a = packets_by_viewer.get(&OBSERVER_A).expect("observer A");
        let observer_b = packets_by_viewer.get(&OBSERVER_B).expect("observer B");
        assert_eq!(observer_a.as_slice(), std::slice::from_ref(&expected));
        assert_eq!(observer_b.as_slice(), std::slice::from_ref(&expected));
        assert_eq!(
            observer_a[0].as_ptr(),
            observer_b[0].as_ptr(),
            "SWG-PKT-001: observers share one encoded Bytes buffer"
        );
        assert!(
            !packets_by_viewer.contains_key(&SUBJECT_VIEWER),
            "SWG-DST-001: observer-only swing excludes actor"
        );

        let mut late_spawn = Vec::new();
        codec.hold(
            LATE_VIEWER,
            Hold::Gain {
                subject: PLAYER,
                tier: Tier::NEAREST,
            },
            &mut late_spawn,
        );
        assert!(
            !late_spawn.contains(&expected),
            "SWG-DST-002: transient swing never enters spawn backlog"
        );
    }

    #[test]
    fn player_swings_skip_lost_observers_and_same_tick_despawn() {
        let mut codec = setup_swing_codec();
        let expected = record(&ClientboundSwingAnimation {
            entity_id: wire_id(PLAYER),
            hand: Hand::Off,
            animation: SwingAnimationKind::Stab,
            duration: VarInt(9),
        })
        .unwrap();

        codec.apply(
            1,
            &TileDelta {
                tile: TileKey::new(0, 0, 0),
                events: vec![swing_event(Hand::Off, SwingAnimationKind::Stab, 9)],
            },
        );
        let mut lost = Vec::new();
        codec.hold(
            OBSERVER_A,
            Hold::Lose {
                subjects: &[PLAYER],
            },
            &mut lost,
        );
        let by_viewer = finish_by_viewer(&mut codec, 1);
        assert!(
            !by_viewer.contains_key(&OBSERVER_A),
            "same-tick Hold::Lose must not receive SwingAnimation after RemoveEntities"
        );
        assert_eq!(
            by_viewer.get(&OBSERVER_B).map(Vec::as_slice),
            Some(std::slice::from_ref(&expected)),
            "remaining observer still receives the swing"
        );

        codec.apply(
            2,
            &TileDelta {
                tile: TileKey::new(0, 0, 0),
                events: vec![
                    swing_event(Hand::Main, SwingAnimationKind::Whack, 6),
                    SubjectEvent {
                        subject: PLAYER,
                        event: EntityEvent::Despawn,
                    },
                ],
            },
        );
        let whack = record(&ClientboundSwingAnimation {
            entity_id: wire_id(PLAYER),
            hand: Hand::Main,
            animation: SwingAnimationKind::Whack,
            duration: VarInt(6),
        })
        .unwrap();
        let after_despawn = finish_by_viewer(&mut codec, 2);
        assert!(
            after_despawn
                .values()
                .all(|chunks| chunks.iter().all(|chunk| chunk != &whack)),
            "same-tick Despawn must not emit a queued SwingAnimation"
        );
    }

    #[test]
    fn a_viewer_lost_in_the_tick_of_a_hide_gets_the_removal_once() {
        let mut codec = setup_swing_codec();
        codec.apply(
            1,
            &TileDelta {
                tile: TileKey::new(0, 0, 0),
                events: vec![SubjectEvent {
                    subject: PLAYER,
                    event: EntityEvent::Data(PlayerData::Hide.into()),
                }],
            },
        );
        let mut lost = Vec::new();
        codec.hold(
            OBSERVER_A,
            Hold::Lose {
                subjects: &[PLAYER],
            },
            &mut lost,
        );
        let removal = record(&ClientboundRemoveEntities {
            entity_ids: vec![wire_id(PLAYER)],
        })
        .unwrap();
        let finished = finish_by_viewer(&mut codec, 1);
        assert_eq!(
            (lost, finished.get(&OBSERVER_A)),
            (vec![removal.clone()], None),
            "a viewer that held the player when it lost it gets one removal"
        );
        assert_eq!(
            finished.get(&OBSERVER_B).map(Vec::as_slice),
            Some(std::slice::from_ref(&removal)),
            "a viewer that still holds the player gets the removal of the hide"
        );
    }

    #[test]
    fn a_viewer_gained_in_the_tick_of_a_hide_sees_the_player_when_it_shows() {
        let mut codec = setup_swing_codec();
        let player_event = |data: PlayerData| TileDelta {
            tile: TileKey::new(0, 0, 0),
            events: vec![SubjectEvent {
                subject: PLAYER,
                event: EntityEvent::Data(data.into()),
            }],
        };
        codec.apply(1, &player_event(PlayerData::Hide));
        let mut gained = Vec::new();
        codec.hold(
            LATE_VIEWER,
            Hold::Gain {
                subject: PLAYER,
                tier: Tier::NEAREST,
            },
            &mut gained,
        );
        let hidden = finish_by_viewer(&mut codec, 1);
        assert!(
            gained.is_empty() && !hidden.contains_key(&LATE_VIEWER),
            "a viewer gained while the player is hidden gets neither a spawn nor a removal"
        );

        codec.apply(2, &player_event(PlayerData::Show));
        let shown = finish_by_viewer(&mut codec, 2);
        let spawn_of = |viewer| shown.get(&viewer).filter(|chunks| !chunks.is_empty());
        assert!(
            spawn_of(OBSERVER_A).is_some(),
            "a viewer that held the player before the hide gets the spawn"
        );
        assert_eq!(
            spawn_of(LATE_VIEWER),
            spawn_of(OBSERVER_A),
            "a viewer gained in the tick of the hide gets the same spawn"
        );
    }
}
