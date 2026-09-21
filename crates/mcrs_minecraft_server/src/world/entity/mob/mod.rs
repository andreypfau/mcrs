use crate::world::aoi::{PlayerTrackerSet, TrackedBy, on_changed_transform};
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;
use bevy_app::{App, FixedPostUpdate, Plugin};
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::prelude::{
    Changed, Commands, Entity, Has, IntoScheduleConfigs, MessageWriter, On, Query, Res, Resource,
    SystemCondition, With,
};
use bevy_ecs::query::QueryData;
use bevy_ecs::schedule::{ScheduleConfigs, SystemSet};
use bevy_ecs::system::ScheduleSystem;
use bevy_math::DVec3;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::{ColumnPos, Direction, ResourceLocation, SectionPos};
use mcrs_minecraft_item::{ItemStack, Items, WireStack};
use mcrs_minecraft_level::aoi::{EntityTracker, PlayerObservers, TickInterval};
use mcrs_minecraft_level::entity::mob::{
    Baby, CatVariant, ChickenVariant, EntityInSection, EntityKind, EntityUuid, Equipment, Health,
    ItemFrame, MobFlags, RiddenBy, Riding, Villager, ZombieNautilusVariant,
};
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::entity::player::reposition::Reposition;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex};
use mcrs_minecraft_protocol::entity::{EquipmentSlot, MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_protocol::item::{ComponentPatch, RawStack};
use mcrs_minecraft_protocol::packets::game::clientbound::AttributeSnapshot;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{ProtoStack, VarInt};
use mcrs_minecraft_registry::{ChainLookup, RegistryLookup};
use mcrs_minecraft_world::entity::attribute::MAX_HEALTH;
use mcrs_minecraft_world::entity::minecraft as entity_types;
use mcrs_minecraft_world::entity::villager::VillagerData;
use mcrs_minecraft_worldgen_feature_place::entity::{
    Equipment as GeneratedEquipment, GeneratedEntity, GeneratedKind, ItemStack as GeneratedStack,
};
use serde::de::value::StrDeserializer;
use serde::de::{DeserializeOwned, IntoDeserializer};
use smallvec::{SmallVec, smallvec};

/// `clientTrackingRange` of the spawned kinds, in blocks: eight chunks, the
/// range of every monster; the guardian, shulker, frame and villager see ten.
// ponytail: one range for every kind; put the per-kind range on the entity type table if a
// player is meant to see a villager two chunks before a witch.
const TRACKING_RANGE_SQ: f64 = (8.0 * 16.0) * (8.0 * 16.0);

pub struct MobTracker;

#[derive(SystemSet, Clone, Default, Hash, PartialEq, Eq, Debug)]
pub struct MobTrackerSet;

#[derive(Resource, Default)]
pub struct MobTrackerCache;

impl EntityTracker for MobTracker {
    type Entity = EntityKind;
    type Cache = MobTrackerCache;
    type Set = MobTrackerSet;
    const CADENCE: TickInterval = TickInterval::Every;

    fn systems() -> ScheduleConfigs<ScheduleSystem> {
        update_mob_tracked_by.in_set(MobTrackerSet)
    }
}

pub struct MobTrackerPlugin;

impl Plugin for MobTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MobTrackerCache>();
        app.add_systems(
            FixedPostUpdate,
            MobTracker::systems()
                .after(PlayerTrackerSet)
                .run_if(on_changed_transform.or_else(on_changed_observers)),
        );
        app.add_observer(remove_untracked);
    }
}

fn on_changed_observers(query: Query<(), Changed<PlayerObservers>>) -> bool {
    !query.is_empty()
}

/// One entity per generated entry, linked to the section of its column that
/// holds its height; a passenger rides the entity spawned before it.
pub fn spawn_generated_entities(
    commands: &mut Commands,
    dim: InDimension,
    sections: &[(Entity, SectionPos)],
    registry: Option<&RegistryAccess>,
    items: Option<&Items>,
    entities: Vec<GeneratedEntity>,
) {
    for entity in entities {
        let section_y = (entity.pos[1].floor() as i32).div_euclid(16);
        let Some(&(section, _)) = sections.iter().find(|(_, pos)| pos.y == section_y) else {
            // ponytail: an entity of a section the column delivered without is dropped, as a
            // block entity is; keep them in the store per section if a late section must
            // carry them.
            tracing::debug!(pos = ?entity.pos, id = entity.kind.id(), "an entity outside the delivered sections");
            continue;
        };
        spawn_one(commands, dim, section, registry, items, entity, None);
    }
}

fn spawn_one(
    commands: &mut Commands,
    dim: InDimension,
    section: Entity,
    registry: Option<&RegistryAccess>,
    items: Option<&Items>,
    entity: GeneratedEntity,
    vehicle: Option<Entity>,
) {
    let [yaw, pitch] = entity.rotation;
    let mut spawned = commands.spawn((
        dim,
        Transform {
            translation: DVec3::from_array(entity.pos),
            rotation: Rotation::new(yaw, pitch),
        },
        EntityUuid(uuid(entity.uuid)),
        EntityInSection(section),
        TrackedBy::default(),
    ));
    if let Some(vehicle) = vehicle {
        spawned.insert(Riding(vehicle));
    }
    let left_handed = |left: bool| {
        if left {
            MobFlags::LEFT_HANDED
        } else {
            MobFlags::empty()
        }
    };
    match entity.kind {
        GeneratedKind::Witch { left_handed: left } => {
            spawned.insert((
                EntityKind(&entity_types::WITCH),
                Health::full(26.0),
                left_handed(left),
            ));
        }
        GeneratedKind::Cat {
            left_handed: left,
            variant,
            sound_variant,
        } => {
            spawned.insert((
                EntityKind(&entity_types::CAT),
                Health::full(10.0),
                left_handed(left),
            ));
            if let Some((variant, sound)) = registry_id(registry, "minecraft:cat_variant", &variant)
                .zip(registry_id(
                    registry,
                    "minecraft:cat_sound_variant",
                    &sound_variant,
                ))
            {
                spawned.insert(CatVariant { variant, sound });
            }
        }
        GeneratedKind::ElderGuardian { left_handed: left } => {
            spawned.insert((
                EntityKind(&entity_types::ELDER_GUARDIAN),
                Health::full(80.0),
                left_handed(left),
            ));
        }
        GeneratedKind::Drowned {
            left_handed: left,
            baby,
            equipment,
        } => {
            spawned.insert((
                EntityKind(&entity_types::DROWNED),
                Health::full(20.0),
                left_handed(left),
            ));
            if baby {
                spawned.insert(Baby);
            }
            if !equipment.is_empty() {
                spawned.insert(carried(items, equipment));
            }
        }
        GeneratedKind::Chicken {
            left_handed: left,
            variant,
            sound_variant,
            ..
        } => {
            spawned.insert((
                EntityKind(&entity_types::CHICKEN),
                Health::full(4.0),
                left_handed(left),
            ));
            if let Some((variant, sound)) =
                registry_id(registry, "minecraft:chicken_variant", &variant).zip(registry_id(
                    registry,
                    "minecraft:chicken_sound_variant",
                    &sound_variant,
                ))
            {
                spawned.insert(ChickenVariant { variant, sound });
            }
        }
        GeneratedKind::ZombieNautilus {
            left_handed: left,
            variant,
        } => {
            spawned.insert((
                EntityKind(&entity_types::ZOMBIE_NAUTILUS),
                Health::full(15.0),
                left_handed(left),
            ));
            if let Some(variant) =
                registry_id(registry, "minecraft:zombie_nautilus_variant", &variant)
            {
                spawned.insert(ZombieNautilusVariant(variant));
            }
        }
        GeneratedKind::Shulker { left_handed: left } => {
            spawned.insert((
                EntityKind(&entity_types::SHULKER),
                Health::full(30.0),
                left_handed(left),
            ));
        }
        GeneratedKind::ItemFrame { item, facing } => {
            spawned.insert((
                EntityKind(&entity_types::ITEM_FRAME),
                ItemFrame {
                    item: stack(items, item),
                    facing,
                },
            ));
        }
        GeneratedKind::Evoker { left_handed: left } => {
            spawned.insert((
                EntityKind(&entity_types::EVOKER),
                Health::full(24.0),
                left_handed(left),
            ));
        }
        GeneratedKind::Vindicator {
            left_handed: left,
            equipment,
        } => {
            spawned.insert((
                EntityKind(&entity_types::VINDICATOR),
                Health::full(24.0),
                left_handed(left),
            ));
            if !equipment.is_empty() {
                spawned.insert(carried(items, equipment));
            }
        }
        GeneratedKind::Allay { left_handed: left } => {
            spawned.insert((
                EntityKind(&entity_types::ALLAY),
                Health::full(20.0),
                left_handed(left),
            ));
        }
        GeneratedKind::Villager { data } => {
            spawned.insert((
                EntityKind(&entity_types::VILLAGER),
                Health::full(20.0),
                MobFlags::empty(),
                Villager(villager_data(&data)),
            ));
        }
        GeneratedKind::ZombieVillager { data } => {
            spawned.insert((
                EntityKind(&entity_types::ZOMBIE_VILLAGER),
                Health::full(20.0),
                MobFlags::empty(),
                Villager(villager_data(&data)),
            ));
        }
        GeneratedKind::ChestMinecart {
            loot_table,
            loot_table_seed,
        } => {
            spawned.insert((
                EntityKind(&entity_types::CHEST_MINECART),
                mcrs_minecraft_level::entity::mob::ContainerLoot {
                    table: loot_table,
                    seed: loot_table_seed,
                },
            ));
        }
    }
    let id = spawned.id();
    for passenger in entity.passengers {
        spawn_one(commands, dim, section, registry, items, passenger, Some(id));
    }
}

fn uuid([a, b, c, d]: [i32; 4]) -> Uuid {
    let most = ((a as u32 as u64) << 32) | (b as u32 as u64);
    let least = ((c as u32 as u64) << 32) | (d as u32 as u64);
    Uuid::from_u64_pair(most, least)
}

fn registry_id(
    registry: Option<&RegistryAccess>,
    key: &str,
    location: &ResourceLocation,
) -> Option<u32> {
    let id = registry?
        .iter()
        .find(|snapshot| snapshot.registry_key() == key)?
        .iter_entries()
        .find(|entry| entry.location.as_str() == location.as_str())
        .map(|entry| entry.network_id);
    if id.is_none() {
        tracing::warn!(key, %location, "a spawned entity names a variant the registry lacks");
    }
    id
}

fn villager_data(data: &mcrs_minecraft_worldgen_feature::template::VillagerData) -> VillagerData {
    VillagerData {
        kind: registered(data.kind.as_str()),
        profession: registered(data.profession.as_str()),
        level: data.level,
    }
}

/// A villager type or profession by its id, the kind's default when the
/// template names one the registry lacks.
fn registered<T: DeserializeOwned + Default>(name: &str) -> T {
    let named: StrDeserializer<serde::de::value::Error> = name.into_deserializer();
    T::deserialize(named).unwrap_or_else(|_| {
        tracing::warn!(name, "a template villager names an id the registry lacks");
        T::default()
    })
}

/// A stack the corpus cannot name is carried as nothing.
fn stack(items: Option<&Items>, stack: GeneratedStack) -> Option<ItemStack> {
    let name = serde_json::to_value(stack.id).ok()?;
    let id = items?.id_of(name.as_str()?);
    if id.is_none() {
        tracing::warn!(item = %name, "a spawned entity carries an item the corpus lacks");
    }
    Some(ItemStack::new(id?, stack.count as u8))
}

fn carried(items: Option<&Items>, equipment: GeneratedEquipment) -> Equipment {
    Equipment {
        mainhand: equipment
            .mainhand
            .and_then(|stack| self::stack(items, stack)),
        offhand: equipment
            .offhand
            .and_then(|stack| self::stack(items, stack)),
    }
}

/// Indices of the synched entity data the spawned kinds send, in the order
/// their class chain defines them.
const HEALTH: u8 = 9;
const MOB_FLAGS: u8 = 15;
const BABY: u8 = 16;
const CAT_VARIANT: u8 = 20;
const CAT_SOUND_VARIANT: u8 = 24;
const CHICKEN_VARIANT: u8 = 18;
const CHICKEN_SOUND_VARIANT: u8 = 19;
const ZOMBIE_NAUTILUS_VARIANT: u8 = 21;
const VILLAGER_DATA: u8 = 19;
const ZOMBIE_VILLAGER_DATA: u8 = 20;
const HANGING_DIRECTION: u8 = 8;
const FRAME_ITEM: u8 = 9;
const DROPPED_ITEM_STACK: u8 = 8;

#[derive(QueryData)]
pub struct Pairing {
    entity: Entity,
    kind: &'static EntityKind,
    uuid: &'static EntityUuid,
    transform: &'static Transform,
    health: Option<&'static Health>,
    flags: Option<&'static MobFlags>,
    baby: Has<Baby>,
    cat: Option<&'static CatVariant>,
    chicken: Option<&'static ChickenVariant>,
    nautilus: Option<&'static ZombieNautilusVariant>,
    villager: Option<&'static Villager>,
    frame: Option<&'static ItemFrame>,
    equipment: Option<&'static Equipment>,
    ridden_by: Option<&'static RiddenBy>,
    riding: Option<&'static Riding>,
    wire: Option<&'static WireStack>,
}

fn wire_id(entity: Entity) -> i32 {
    entity.index_u32() as i32
}

/// A stack the registries cannot encode is dropped from the packet rather
/// than sent malformed.
fn wire_stack(stack: ItemStack, lookup: &dyn RegistryLookup) -> Option<RawStack> {
    let slot = ProtoStack::new(
        stack.item(),
        i32::from(stack.count()),
        ComponentPatch::default(),
    );
    RawStack::from_stack(&slot, lookup)
        .inspect_err(|error| tracing::warn!(%error, "a mob's stack could not be encoded"))
        .ok()
}

impl PairingItem<'_, '_> {
    /// What a player is told when it starts seeing the entity: the add packet
    /// with the data that differs from the kind's defaults, its attributes,
    /// what it holds and who rides whom.
    fn packets(
        &self,
        reposition: &Reposition,
        vehicles: &Query<&RiddenBy>,
        lookup: &dyn RegistryLookup,
    ) -> Vec<PacketPayload> {
        let id = wire_id(self.entity);
        let mut out = vec![PacketPayload::PlayerEnteredView {
            entity_id: id,
            uuid: self.uuid.0,
            kind: self.kind.protocol_id as i32,
            position: reposition.convert_dvec3(self.transform.translation),
            yaw: self.transform.rotation.yaw(),
            pitch: self.transform.rotation.pitch(),
            data: self.frame.map_or(0, |frame| frame.facing.id() as i32),
        }];
        let metadata = self.entity_data(lookup);
        if !metadata.is_empty() {
            out.push(PacketPayload::SetEntityData {
                entity_id: id,
                metadata: Metadata(metadata),
            });
        }
        if let Some(health) = self.health {
            out.push(PacketPayload::UpdateAttributes {
                entity_id: id,
                attributes: vec![AttributeSnapshot {
                    attribute: VarInt(MAX_HEALTH.protocol_id as i32),
                    base: f64::from(health.max),
                    modifiers: Vec::new(),
                }],
            });
        }
        if let Some(equipment) = self.equipment {
            let slots: Vec<(EquipmentSlot, RawStack)> = [
                (EquipmentSlot::MainHand, equipment.mainhand),
                (EquipmentSlot::OffHand, equipment.offhand),
            ]
            .into_iter()
            .filter_map(|(slot, stack)| Some((slot, wire_stack(stack?, lookup)?)))
            .collect();
            if !slots.is_empty() {
                out.push(PacketPayload::SetEquipment {
                    entity_id: id,
                    slots,
                });
            }
        }
        if let Some(ridden_by) = self.ridden_by.filter(|riders| !riders.is_empty()) {
            out.push(PacketPayload::SetPassengers {
                vehicle: id,
                passengers: ridden_by.iter().map(|rider| wire_id(*rider)).collect(),
            });
        }
        if let Some(vehicle) = self.riding
            && let Ok(riders) = vehicles.get(vehicle.0)
        {
            out.push(PacketPayload::SetPassengers {
                vehicle: wire_id(vehicle.0),
                passengers: riders.iter().map(|rider| wire_id(*rider)).collect(),
            });
        }
        out
    }

    fn entity_data(&self, lookup: &dyn RegistryLookup) -> Vec<MetadataEntry<'static>> {
        let mut data = Vec::new();
        let mut put = |index: u8, value: MetaDataValue<'static>| {
            data.push(MetadataEntry { index, value });
        };
        if let Some(frame) = self.frame {
            if frame.facing != Direction::South {
                put(HANGING_DIRECTION, MetaDataValue::Direction(frame.facing));
            }
            if let Some(item) = frame.item.and_then(|item| wire_stack(item, lookup)) {
                put(FRAME_ITEM, MetaDataValue::Slot(item));
            }
        }
        if let Some(wire) = self.wire {
            put(DROPPED_ITEM_STACK, MetaDataValue::Slot(wire.0.clone()));
        }
        if let Some(health) = self.health
            && health.current != 1.0
        {
            put(HEALTH, MetaDataValue::Float(health.current));
        }
        if let Some(flags) = self.flags
            && !flags.is_empty()
        {
            put(MOB_FLAGS, MetaDataValue::Byte(flags.bits() as i8));
        }
        if self.baby {
            put(BABY, MetaDataValue::Boolean(true));
        }
        if let Some(cat) = self.cat {
            put(
                CAT_VARIANT,
                MetaDataValue::CatVariant(VarInt(cat.variant as i32)),
            );
            put(
                CAT_SOUND_VARIANT,
                MetaDataValue::CatSoundVariant(VarInt(cat.sound as i32)),
            );
        }
        if let Some(chicken) = self.chicken {
            put(
                CHICKEN_VARIANT,
                MetaDataValue::ChickenVariant(VarInt(chicken.variant as i32)),
            );
            put(
                CHICKEN_SOUND_VARIANT,
                MetaDataValue::ChickenSoundVariant(VarInt(chicken.sound as i32)),
            );
        }
        if let Some(nautilus) = self.nautilus {
            put(
                ZOMBIE_NAUTILUS_VARIANT,
                MetaDataValue::ZombieNautilusVariant(VarInt(nautilus.0 as i32)),
            );
        }
        if let Some(villager) = self.villager {
            let zombie = *self.kind.0 == entity_types::ZOMBIE_VILLAGER;
            if zombie || villager.0 != VillagerData::default() {
                put(
                    if zombie {
                        ZOMBIE_VILLAGER_DATA
                    } else {
                        VILLAGER_DATA
                    },
                    MetaDataValue::VillagerData(mcrs_minecraft_protocol::entity::VillagerData {
                        kind: VarInt(villager.kind.protocol_id() as i32),
                        profession: VarInt(villager.profession.protocol_id() as i32),
                        level: VarInt(villager.level),
                    }),
                );
            }
        }
        data
    }
}

fn to(anchor: Entity, data: PacketPayload) -> OutboundPlayerPacket {
    OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(anchor),
        priority: PacketPriority::Normal,
        data,
        session: PlayerSession(0),
        epoch: 0,
    }
}

fn remove(entity: Entity) -> PacketPayload {
    PacketPayload::PlayerLeftView {
        entity_ids: smallvec![wire_id(entity)],
    }
}

/// A player sees a mob when it holds the mob's column and stands within the
/// tracking range of it, horizontally, as the reference decides it.
// ponytail: every mob is re-evaluated whenever any transform or observer set changes; index
// mobs per column when their count makes that a cost.
#[allow(clippy::type_complexity)]
pub fn update_mob_tracked_by(
    mut mobs: Query<(&InDimension, &mut TrackedBy, Pairing), With<EntityKind>>,
    vehicles: Query<&RiddenBy>,
    registry: Res<RegistryAccess>,
    blocks: Option<Res<Blocks>>,
    observers: Query<&PlayerObservers, With<Column>>,
    column_indices: Query<&ColumnIndex>,
    players: Query<(&Transform, &HostAnchor, &Reposition), With<Player>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let registry: &dyn RegistryLookup = &*registry;
    let mut lookups: Vec<&dyn RegistryLookup> = Vec::with_capacity(2);
    lookups.push(registry);
    if let Some(blocks) = &blocks {
        lookups.push(&*blocks.0);
    }
    let lookup = ChainLookup(&lookups);
    for (in_dim, mut tracked_by, pairing) in mobs.iter_mut() {
        let at = pairing.transform.translation;
        let seen_by = column_indices
            .get(in_dim.0)
            .ok()
            .and_then(|index| index.0.get(&ColumnPos::from(at)))
            .and_then(|slot| observers.get(slot.entity).ok());
        let mut now: SmallVec<[Entity; 32]> = SmallVec::new();
        for &player in seen_by.into_iter().flat_map(|seen_by| seen_by.0.iter()) {
            let Ok((transform, _, _)) = players.get(player) else {
                continue;
            };
            let delta = transform.translation - at;
            if delta.x * delta.x + delta.z * delta.z <= TRACKING_RANGE_SQ && !now.contains(&player)
            {
                now.push(player);
            }
        }
        for &player in &now {
            if tracked_by.0.contains(&player) {
                continue;
            }
            let Ok((_, anchor, reposition)) = players.get(player) else {
                continue;
            };
            for payload in pairing.packets(reposition, &vehicles, &lookup) {
                packets.write(to(anchor.0, payload));
            }
        }
        for &player in tracked_by.0.iter() {
            if !now.contains(&player)
                && let Ok((_, anchor, _)) = players.get(player)
            {
                packets.write(to(anchor.0, remove(pairing.entity)));
            }
        }
        if tracked_by.0 != now {
            tracked_by.0 = now;
        }
    }
}

fn remove_untracked(
    removed: On<Remove, TrackedBy>,
    mobs: Query<&TrackedBy, With<EntityKind>>,
    players: Query<&HostAnchor, With<Player>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let mob = removed.event().entity;
    let Ok(tracked_by) = mobs.get(mob) else {
        return;
    };
    for anchor in tracked_by
        .0
        .iter()
        .filter_map(|player| players.get(*player).ok())
    {
        packets.write(to(anchor.0, remove(mob)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::message::Messages;
    use bevy_ecs::schedule::Schedule;
    use mcrs_minecraft_level::world::storage::column::ColumnSlot;

    fn drain(app: &mut App) -> Vec<(Entity, PacketPayload)> {
        app.world_mut()
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .map(|packet| match packet.target {
                PacketTarget::SinglePlayer(anchor) => (anchor, packet.data),
                other => panic!("a mob packet addressed {other:?}"),
            })
            .collect()
    }

    fn left_view(sent: &[(Entity, PacketPayload)], anchor: Entity, mob: Entity) -> bool {
        matches!(
            sent,
            [(to, PacketPayload::PlayerLeftView { entity_ids })]
                if *to == anchor && entity_ids.as_slice() == [wire_id(mob)]
        )
    }

    #[test]
    fn a_witch_is_paired_with_the_player_holding_its_column_and_removed_with_its_section() {
        let mut app = App::new();
        app.add_schedule(Schedule::new(FixedPostUpdate));
        app.add_message::<OutboundPlayerPacket>();
        app.insert_resource(RegistryAccess::default());
        app.add_plugins(MobTrackerPlugin);

        let dim = app.world_mut().spawn(ColumnIndex::default()).id();
        let section = app
            .world_mut()
            .spawn((SectionPos::new(0, 4, 0), InDimension(dim)))
            .id();
        let anchor = app.world_mut().spawn_empty().id();
        let player = app
            .world_mut()
            .spawn((
                Player,
                Transform::from_xyz(40.0, 70.0, 8.0),
                Reposition::default(),
                InDimension(dim),
                HostAnchor(anchor),
            ))
            .id();
        let mut observers = PlayerObservers::default();
        observers.0.push(player);
        let column = app.world_mut().spawn((Column, observers)).id();
        app.world_mut()
            .get_mut::<ColumnIndex>(dim)
            .unwrap()
            .0
            .insert(
                ColumnPos::new(0, 0),
                ColumnSlot {
                    entity: column,
                    section_count: 1,
                },
            );

        let witch = GeneratedEntity {
            pos: [8.5, 65.0, 8.5],
            rotation: [0.0, 0.0],
            uuid: [1, 2, 3, 4],
            kind: GeneratedKind::Witch { left_handed: true },
            passengers: Vec::new(),
        };
        let mut commands = app.world_mut().commands();
        spawn_generated_entities(
            &mut commands,
            InDimension(dim),
            &[(section, SectionPos::new(0, 4, 0))],
            None,
            None,
            vec![witch],
        );
        app.world_mut().flush();
        let mob = app
            .world_mut()
            .query_filtered::<Entity, With<EntityKind>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<EntityInSection>(mob).map(|link| link.0),
            Some(section)
        );

        app.world_mut().run_schedule(FixedPostUpdate);
        let sent = drain(&mut app);
        assert!(sent.iter().all(|(to, _)| *to == anchor), "{sent:?}");
        assert_eq!(sent.len(), 3, "{sent:?}");
        let PacketPayload::PlayerEnteredView {
            entity_id,
            kind,
            position,
            data,
            ..
        } = &sent[0].1
        else {
            panic!("{sent:?}")
        };
        assert_eq!(*entity_id, wire_id(mob));
        assert_eq!(*kind, entity_types::WITCH.protocol_id as i32);
        assert_eq!(*position, DVec3::new(8.5, 65.0, 8.5));
        assert_eq!(*data, 0);
        let PacketPayload::SetEntityData { metadata, .. } = &sent[1].1 else {
            panic!("{sent:?}")
        };
        assert_eq!(
            metadata.0,
            vec![
                MetadataEntry {
                    index: HEALTH,
                    value: MetaDataValue::Float(26.0)
                },
                MetadataEntry {
                    index: MOB_FLAGS,
                    value: MetaDataValue::Byte(2)
                },
            ]
        );
        assert!(matches!(sent[2].1, PacketPayload::UpdateAttributes { .. }));
        assert_eq!(
            app.world()
                .get::<TrackedBy>(mob)
                .map(|tracked| tracked.0.to_vec()),
            Some(vec![player])
        );

        app.world_mut().run_schedule(FixedPostUpdate);
        assert!(drain(&mut app).is_empty(), "a still player is told once");

        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation
            .x = 200.0;
        app.world_mut().run_schedule(FixedPostUpdate);
        let sent = drain(&mut app);
        assert!(left_view(&sent, anchor, mob), "{sent:?}");

        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation
            .x = 40.0;
        app.world_mut().run_schedule(FixedPostUpdate);
        assert_eq!(drain(&mut app).len(), 3, "walking back re-adds the witch");

        app.world_mut().despawn(section);
        assert!(
            app.world().get_entity(mob).is_err(),
            "the section takes its mob"
        );
        let sent = drain(&mut app);
        assert!(left_view(&sent, anchor, mob), "{sent:?}");
    }
}
