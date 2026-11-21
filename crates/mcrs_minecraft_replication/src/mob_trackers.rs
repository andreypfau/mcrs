use std::collections::{BTreeMap, BTreeSet};

use bytes::{Bytes, BytesMut};
use mcrs_engine_muxer::{EntityId, ViewerId};
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundDamageEvent, ClientboundEntityEvent, ClientboundSwingAnimation,
    ClientboundTakeItemEntity,
};
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};
use rustc_hash::FxHashMap;
use uuid::Uuid;

use crate::attribute_state::AttributeState;
use crate::delivery::Delivery;
use crate::encode::record;
use crate::hold_set::HoldSet;
use crate::mob_bundle::{Entries, LivingEntries, spawn_extras};
use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use crate::mob_tracker::MobTracker;
use crate::player_input::Backlog;
use crate::spawn_look::SpawnLook;
use crate::subject_attributes::SubjectAttributes;
use crate::wire_id::wire_id;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::entity::{MetadataEntry, MobLooks};
use mcrs_minecraft_registries::Wire;

const BLOCKS_PER_COLUMN: u32 = 16;

const RESENDS_PER_TICK: usize = 1024;

#[derive(Debug, Default)]
pub(crate) struct MobTrackers {
    mobs: FxHashMap<EntityId, Mob>,
    scratch: BytesMut,
    signals: Vec<(EntityId, Bytes)>,
    swings: Vec<PendingMobSwing>,
    farewells: FxHashMap<EntityId, Vec<ViewerId>>,
    behind: BTreeSet<ViewerId>,
    missed: BTreeMap<ViewerId, BTreeSet<EntityId>>,
}

#[derive(Clone, Copy, Debug)]
struct PendingMobSwing {
    subject: EntityId,
    hand: Hand,
    animation: SwingAnimationKind,
    duration: i32,
}

#[derive(Debug)]
struct Mob {
    kind: &'static EntityTypeData,
    uuid: Uuid,
    looks: MobLooks,
    spawn_data: VarInt,
    tracker: MobTracker,
    attributes: SubjectAttributes,
}

impl MobTrackers {
    pub(crate) fn contains(&self, subject: EntityId) -> bool {
        self.mobs.contains_key(&subject)
    }

    pub(crate) fn spawn(&mut self, subject: EntityId, spawn: &MobSpawn, tick: u64) {
        let (looks, entries, spawn_data) = match &spawn.look {
            SpawnLook::Mob(looks) => (
                looks.clone(),
                Entries::Living(LivingEntries::of(&spawn.state)),
                VarInt(0),
            ),
            SpawnLook::Item(stack) => {
                (MobLooks::default(), Entries::Item(stack.clone()), VarInt(0))
            }
            SpawnLook::FallingBlock { state, start } => (
                MobLooks::default(),
                Entries::FallingBlock(*start),
                VarInt(i32::from(state.0)),
            ),
            SpawnLook::Custom { data, entries } => {
                (MobLooks::default(), Entries::custom(entries), VarInt(*data))
            }
        };
        self.mobs.insert(
            subject,
            Mob {
                kind: spawn.kind,
                uuid: spawn.uuid,
                looks,
                spawn_data,
                tracker: MobTracker::new(spawn.state, entries, spawn.kind, tick),
                attributes: SubjectAttributes::with_stored(
                    spawn.kind.attribute_bases(),
                    Vec::new(),
                ),
            },
        );
    }

    pub(crate) fn set_backlog(&mut self, viewer: ViewerId, backlog: Backlog) {
        match backlog {
            Backlog::Behind => self.behind.insert(viewer),
            Backlog::Clear => self.behind.remove(&viewer),
        };
    }

    pub(crate) fn signal(
        &mut self,
        subject: EntityId,
        signal: MobSignal,
        holds: &HoldSet,
    ) -> Result<()> {
        let entity_id = wire_id(subject);
        let event = |entity_status| {
            record(&ClientboundEntityEvent {
                entity_id: entity_id.0,
                entity_status,
            })
        };
        let packet = match signal {
            MobSignal::Swing {
                hand,
                animation,
                duration,
            } => {
                self.swings.push(PendingMobSwing {
                    subject,
                    hand,
                    animation,
                    duration,
                });
                return Ok(());
            }
            _ if holds.holders(subject).is_empty() => return Ok(()),
            MobSignal::Hurt { damage_type } => {
                let Ok(damage_type) = i32::try_from(damage_type) else {
                    tracing::warn!("the damage type {damage_type} of a hit has no id on the wire");
                    return Ok(());
                };
                record(&ClientboundDamageEvent {
                    entity_id,
                    damage_type: VarInt(damage_type),
                    cause_entity_id: VarInt(0),
                    direct_entity_id: VarInt(0),
                    source_position: None,
                })?
            }
            MobSignal::Died => event(ClientboundEntityEvent::DIED)?,
            MobSignal::Removed => event(ClientboundEntityEvent::REMOVED)?,
            MobSignal::Taken { collector, amount } => record(&ClientboundTakeItemEntity {
                item_id: entity_id,
                player_id: wire_id(collector),
                amount: VarInt(amount),
            })?,
        };
        self.signals.push((subject, packet));
        Ok(())
    }

    pub(crate) fn farewell(
        &self,
        viewer: ViewerId,
        subjects: &[EntityId],
        chunks: &mut Vec<Bytes>,
    ) {
        for (subject, packet) in &self.signals {
            let held = self
                .farewells
                .get(subject)
                .is_some_and(|holders| holders.contains(&viewer));
            if held && subjects.contains(subject) {
                chunks.push(packet.clone());
            }
        }
    }

    pub(crate) fn record(&mut self, subject: EntityId, state: MobState) {
        if let Some(mob) = self.mobs.get_mut(&subject) {
            mob.tracker.record(state);
        }
    }

    pub(crate) fn record_stack(&mut self, subject: EntityId, stack: &ProtoStack<Wire>) {
        if let Some(mob) = self.mobs.get_mut(&subject) {
            mob.tracker.record_stack(stack.clone());
        }
    }

    pub(crate) fn record_entries(&mut self, subject: EntityId, entries: &[MetadataEntry<'static>]) {
        if let Some(mob) = self.mobs.get_mut(&subject) {
            mob.tracker.record_entries(entries);
        }
    }

    pub(crate) fn record_attribute(&mut self, subject: EntityId, attribute: &AttributeState) {
        if let Some(mob) = self.mobs.get_mut(&subject) {
            mob.attributes.record(attribute);
        }
    }

    pub(crate) fn despawn(&mut self, subject: EntityId, holds: &mut HoldSet) {
        if self.mobs.remove(&subject).is_none() {
            return;
        }
        let holders = holds.remove_subject(subject);
        for viewer in &holders {
            if let Some(missed) = self.missed.get_mut(viewer) {
                missed.remove(&subject);
            }
        }
        self.farewells.insert(subject, holders);
    }

    pub(crate) fn tracking_range(&self, subject: EntityId) -> Option<u32> {
        let mob = self.mobs.get(&subject)?;
        Some(mob.kind.tracking_range.saturating_mul(BLOCKS_PER_COLUMN))
    }

    pub(crate) fn position(&self, subject: EntityId) -> Option<[f64; 3]> {
        let mob = self.mobs.get(&subject)?;
        Some(mob.tracker.latest().position.to_array())
    }

    pub(crate) fn gain(
        &mut self,
        viewer: ViewerId,
        subject: EntityId,
        tick: u64,
        delimiter: &Bytes,
        holds: &mut HoldSet,
        chunks: &mut Vec<Bytes>,
    ) -> Result<()> {
        let Some(mob) = self.mobs.get_mut(&subject) else {
            return Ok(());
        };
        if holds.holders(subject).is_empty() && mob.tracker.is_stale(tick) {
            mob.tracker.restart();
        }
        let id = wire_id(subject);
        let add = record(&mob.tracker.add_entity(id, mob.uuid, mob.spawn_data))?;
        let extras = spawn_extras(id, &mob.looks, mob.tracker.sent_entries(), &mob.attributes)?;
        chunks.extend([delimiter.clone(), add, extras, delimiter.clone()]);
        holds.insert(viewer, subject);
        if let Some(missed) = self.missed.get_mut(&viewer) {
            missed.remove(&subject);
        }
        Ok(())
    }

    pub(crate) fn lose(&mut self, viewer: ViewerId, subject: EntityId) -> bool {
        if !self.mobs.contains_key(&subject) {
            return false;
        }
        if let Some(missed) = self.missed.get_mut(&viewer) {
            missed.remove(&subject);
        }
        true
    }

    pub(crate) fn forget(&mut self, viewer: ViewerId, holds: &mut HoldSet) {
        holds.release_where(viewer, |subject| self.mobs.contains_key(&subject));
        self.behind.remove(&viewer);
        self.missed.remove(&viewer);
    }

    pub(crate) fn finish_tick(&mut self, tick: u64, holds: &HoldSet, delivery: &mut Delivery) {
        let Self {
            mobs,
            scratch,
            signals,
            swings,
            farewells,
            behind,
            missed,
        } = self;
        for (subject, packet) in signals.drain(..) {
            for &viewer in holds.holders(subject) {
                delivery.owe(viewer, packet.clone());
            }
        }
        send_mob_swings(mobs, swings, holds, delivery);
        farewells.clear();
        for (subject, holders) in holds.held() {
            let Some(mob) = mobs.get_mut(&subject) else {
                continue;
            };
            scratch.clear();
            if let Err(error) =
                mob.tracker
                    .tick(wire_id(subject), tick, &mut mob.attributes, scratch)
            {
                tracing::warn!("the update of a mob did not encode: {error:#}");
                continue;
            }
            if scratch.is_empty() {
                continue;
            }
            let update = scratch.split().freeze();
            for &viewer in holders {
                let misses = behind.contains(&viewer)
                    || missed
                        .get(&viewer)
                        .is_some_and(|missed| missed.contains(&subject));
                if misses {
                    missed.entry(viewer).or_default().insert(subject);
                } else {
                    delivery.owe(viewer, update.clone());
                }
            }
        }
        resend_missed(mobs, behind, missed, scratch, delivery);
    }
}

fn send_mob_swings(
    mobs: &FxHashMap<EntityId, Mob>,
    swings: &mut Vec<PendingMobSwing>,
    holds: &HoldSet,
    delivery: &mut Delivery,
) {
    for swing in swings.drain(..) {
        if !mobs.contains_key(&swing.subject) {
            continue;
        }
        let observers = holds.holders(swing.subject);
        if observers.is_empty() {
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
                tracing::warn!("a mob swing did not encode: {error:#}");
                continue;
            }
        };
        for &viewer in observers {
            delivery.owe(viewer, packet.clone());
        }
    }
}

fn resend_missed(
    mobs: &FxHashMap<EntityId, Mob>,
    behind: &BTreeSet<ViewerId>,
    missed: &mut BTreeMap<ViewerId, BTreeSet<EntityId>>,
    scratch: &mut BytesMut,
    delivery: &mut Delivery,
) {
    for (viewer, subjects) in missed.iter_mut() {
        if behind.contains(viewer) {
            continue;
        }
        scratch.clear();
        for _ in 0..RESENDS_PER_TICK {
            let Some(subject) = subjects.pop_first() else {
                break;
            };
            let Some(mob) = mobs.get(&subject) else {
                continue;
            };
            if let Err(error) = mob
                .tracker
                .resend(wire_id(subject), &mob.attributes, scratch)
            {
                tracing::warn!("the state of a mob did not encode: {error:#}");
            }
        }
        if !scratch.is_empty() {
            delivery.owe(*viewer, scratch.split().freeze());
        }
    }
    missed.retain(|_, subjects| !subjects.is_empty());
}

#[cfg(test)]
mod tests {
    use glam::DVec3;
    use mcrs_engine_muxer::ViewerBytes;
    use mcrs_minecraft_core::Look;
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
    use mcrs_minecraft_entity_attribute::{
        AttributeModifier, AttributeSet, InstanceModifier, ModifierKind, ModifierOperation,
    };
    use mcrs_minecraft_protocol_game::block::BlockStateNumber;
    use mcrs_minecraft_protocol_game::clientbound::{
        AttributeSnapshot, ClientboundAddEntity, ClientboundEntityPositionSync,
        ClientboundSetEntityData, ClientboundUpdateAttributes, PositionPath,
    };
    use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry};
    use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3};
    use mcrs_minecraft_resource::ResourceLocation;
    use mcrs_voxel::BlockPos;

    use super::*;

    const MOB: EntityId = EntityId(8);
    const KEEPS_UP: ViewerId = ViewerId(1);
    const BEHIND: ViewerId = ViewerId(2);

    fn at(x: f64) -> MobState {
        MobState {
            position: DVec3::new(x, -60.0, 0.5),
            on_ground: true,
            health: 10.0,
            ..MobState::default()
        }
    }

    fn sync_at(x: f64) -> Bytes {
        record(&ClientboundEntityPositionSync {
            entity_id: wire_id(MOB),
            position: PositionPath::Linear(at(x).position),
            look: Look {
                yaw: 0.0,
                pitch: 0.0,
            },
            on_ground: true,
        })
        .unwrap()
    }

    #[derive(Default)]
    struct World {
        mobs: MobTrackers,
        holds: HoldSet,
    }

    impl World {
        fn spawned(state: MobState) -> Self {
            let mut world = Self::default();
            let spawn = MobSpawn {
                kind: &EntityTypeData::PIG,
                uuid: Uuid::nil(),
                state,
                look: SpawnLook::Mob(MobLooks::default()),
            };
            world.mobs.spawn(MOB, &spawn, 0);
            world
        }

        fn gain(&mut self, viewer: ViewerId, tick: u64) -> Vec<Bytes> {
            let mut chunks = Vec::new();
            let delimiter = Bytes::new();
            self.mobs
                .gain(viewer, MOB, tick, &delimiter, &mut self.holds, &mut chunks)
                .unwrap();
            chunks
        }

        fn tick(&mut self, tick: u64) -> BTreeMap<ViewerId, Vec<Bytes>> {
            let mut delivery = Delivery::default();
            self.mobs.finish_tick(tick, &self.holds, &mut delivery);
            let mut out = Vec::new();
            delivery.drain(&mut out);
            out.into_iter()
                .map(|ViewerBytes { viewer, chunks }| (viewer, chunks))
                .collect()
        }
    }

    #[test]
    fn the_spawn_bundle_holds_only_the_instances_that_differ_from_the_type() {
        let mut world = World::spawned(at(0.5));
        let data = record(&ClientboundSetEntityData {
            entity_id: wire_id(MOB),
            metadata: Metadata(vec![MetadataEntry {
                index: 9,
                value: MetaDataValue::Float(10.0),
            }]),
        })
        .unwrap();
        assert_eq!(
            world.gain(KEEPS_UP, 0)[2],
            data,
            "a mob of the type sends no update_attributes"
        );

        let bases = EntityTypeData::PIG.attribute_bases().unwrap();
        let mut set = AttributeSet::new();
        set.add_modifier(
            bases,
            Attribute::MovementSpeed,
            InstanceModifier::new(
                "test:slow".parse().unwrap(),
                -0.5,
                ModifierOperation::AddMultipliedTotal,
                ModifierKind::Permanent,
            ),
        )
        .unwrap();
        world.mobs.record_attribute(
            MOB,
            &AttributeState {
                attribute: Attribute::MovementSpeed,
                instance: set.instances().next().cloned(),
            },
        );
        let attributes = record(&ClientboundUpdateAttributes {
            entity_id: wire_id(MOB),
            attributes: vec![AttributeSnapshot {
                attribute: Attribute::MovementSpeed.into(),
                base: 0.25,
                modifiers: vec![AttributeModifier {
                    id: ResourceLocation::new_static("test:slow").unwrap(),
                    amount: -0.5,
                    operation: ModifierOperation::AddMultipliedTotal,
                }],
            }],
        })
        .unwrap();
        assert_eq!(
            world.gain(BEHIND, 0)[2],
            Bytes::from([data, attributes].concat()),
            "the base of the type and the modifier"
        );
    }

    #[test]
    fn fbk_spn_002_the_spawn_of_a_falling_block_carries_its_state_and_start() {
        let mut world = World::default();
        let state = at(0.5);
        let start = BlockPos::new(0, -59, 0);
        let spawn = MobSpawn {
            kind: &EntityTypeData::FALLING_BLOCK,
            uuid: Uuid::nil(),
            state,
            look: SpawnLook::FallingBlock {
                state: BlockStateNumber(118),
                start,
            },
        };
        world.mobs.spawn(MOB, &spawn, 0);

        let chunks = world.gain(KEEPS_UP, 0);

        let add = record(&ClientboundAddEntity {
            id: wire_id(MOB),
            uuid: Uuid::nil(),
            kind: EntityType::FallingBlock,
            pos: state.position,
            movement: LpVec3(DVec3::ZERO),
            pitch: ByteAngle::default(),
            yaw: ByteAngle::default(),
            head_yaw: ByteAngle::default(),
            data: VarInt(118),
        })
        .unwrap();
        let data = record(&ClientboundSetEntityData {
            entity_id: wire_id(MOB),
            metadata: Metadata(vec![MetadataEntry {
                index: 8,
                value: MetaDataValue::BlockPos(start),
            }]),
        })
        .unwrap();
        assert_eq!(
            chunks[1..3],
            [add, data],
            "add_entity with the state id, then only the start position"
        );
    }

    #[test]
    fn a_custom_kind_spawns_with_its_data_and_sends_a_changed_entry_once() {
        let entry = |index, value| MetadataEntry {
            index,
            value: MetaDataValue::VarInt(VarInt(value)),
        };
        let mut world = World::default();
        let state = at(0.5);
        let spawn = MobSpawn {
            kind: &EntityTypeData::ITEM,
            uuid: Uuid::nil(),
            state,
            look: SpawnLook::Custom {
                data: 7,
                entries: vec![entry(9, 2), entry(8, 1)],
            },
        };
        world.mobs.spawn(MOB, &spawn, 0);

        let chunks = world.gain(KEEPS_UP, 0);
        world.tick(0);

        let add = record(&ClientboundAddEntity {
            id: wire_id(MOB),
            uuid: Uuid::nil(),
            kind: EntityType::Item,
            pos: state.position,
            movement: LpVec3(DVec3::ZERO),
            pitch: ByteAngle::default(),
            yaw: ByteAngle::default(),
            head_yaw: ByteAngle::default(),
            data: VarInt(7),
        })
        .unwrap();
        let data = |entries| {
            record(&ClientboundSetEntityData {
                entity_id: wire_id(MOB),
                metadata: Metadata(entries),
            })
            .unwrap()
        };
        assert_eq!(
            chunks[1..3],
            [add, data(vec![entry(8, 1), entry(9, 2)])],
            "add_entity with the data value, then the entries in the order of the indices"
        );

        world.mobs.record_entries(MOB, &[entry(8, 1), entry(9, 5)]);
        assert_eq!(
            world.tick(1).get(&KEEPS_UP),
            Some(&vec![data(vec![entry(9, 5)])]),
            "only the entry that changed"
        );
        assert!(world.tick(2).is_empty(), "the entry goes once");
    }

    #[test]
    fn lagging_viewer_misses_moves_then_gets_the_held_state() {
        let mut world = World::spawned(at(0.5));
        for viewer in [KEEPS_UP, BEHIND] {
            world.gain(viewer, 0);
        }
        world.tick(0);

        world.mobs.set_backlog(BEHIND, Backlog::Behind);
        let mut moves = 0;
        for (step, now) in (1..=6).enumerate() {
            world.mobs.record(MOB, at(0.5 + 0.2 * (step + 1) as f64));
            let owed = world.tick(now);
            assert!(!owed.contains_key(&BEHIND), "no move while behind");
            moves += owed.get(&KEEPS_UP).map_or(0, Vec::len);
        }
        assert!(moves > 0, "the other viewer keeps getting the moves");

        world.mobs.set_backlog(BEHIND, Backlog::Clear);
        let owed = world.tick(7);
        let resent = owed.get(&BEHIND).and_then(|chunks| chunks.first()).unwrap();
        assert!(
            resent.starts_with(&sync_at(0.5 + 0.2 * 6.0)),
            "the latest state that the other viewer holds, as an absolute position"
        );
        assert!(!world.tick(8).contains_key(&BEHIND), "sent once");
    }

    #[test]
    fn a_mob_that_leaves_while_a_viewer_is_behind_is_not_resent() {
        let mut world = World::spawned(at(0.5));
        world.gain(BEHIND, 0);
        world.tick(0);
        world.mobs.set_backlog(BEHIND, Backlog::Behind);
        world.mobs.record(MOB, at(2.5));
        world.tick(3);

        world.mobs.despawn(MOB, &mut world.holds);
        world.mobs.set_backlog(BEHIND, Backlog::Clear);

        assert!(
            world.holds.holders(MOB).is_empty(),
            "no client holds a mob that left"
        );
        assert!(
            !world.tick(4).contains_key(&BEHIND),
            "the state of a mob that left is not resent"
        );
    }

    #[test]
    fn the_first_viewer_after_a_gap_gets_the_latest_state_in_the_spawn() {
        let mut world = World::spawned(at(0.5));
        world.gain(KEEPS_UP, 0);
        world.tick(0);
        world.holds.remove(KEEPS_UP, MOB);
        world.mobs.lose(KEEPS_UP, MOB);
        for now in 1..10_u32 {
            world.mobs.record(MOB, at(0.5 + f64::from(now)));
            world.tick(u64::from(now));
        }

        world.gain(BEHIND, 10);

        let add = world.mobs.mobs[&MOB]
            .tracker
            .add_entity(wire_id(MOB), Uuid::nil(), VarInt(0));
        assert_eq!(
            add.pos,
            at(9.5).position,
            "the spawn shows the last state, not the one that no client received"
        );
    }
}
