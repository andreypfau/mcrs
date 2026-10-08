use crate::world::aoi::{PlayerTrackerSet, TrackedBy, on_changed_transform};
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, to};
use crate::world::entity::player::HostAnchor;
use crate::world::item::item_lookups;
use crate::world::item::sync::WireStack;
use bevy_app::{App, FixedPostUpdate, Plugin};
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::prelude::{
    Changed, Commands, Entity, Has, IntoScheduleConfigs, MessageWriter, On, Query, Res,
    SystemCondition, With,
};
use bevy_ecs::query::QueryData;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::{ColumnPos, Direction, ResourceLocation, SectionPos};
use mcrs_minecraft_entity::villager::VillagerData;
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::entity::mob::{
    Baby, CatVariant, ChickenVariant, EntityInSection, EntityKind, EntityUuid, Equipment, Health,
    ItemFrame, MobFlags, RiddenBy, Riding, Villager, ZombieNautilusVariant,
};
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex};
use mcrs_minecraft_protocol::ByteAngle;
use mcrs_minecraft_protocol::LpVec3;
use mcrs_minecraft_protocol::entity::{EquipmentSlot, MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_protocol::item::{ComponentPatch, RawStack};
use mcrs_minecraft_protocol::packets::game::clientbound::AttributeSnapshot;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundRemoveEntities;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSetEntityData;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSetEquipment;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSetPassengers;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundUpdateAttributes;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{ProtoStack, RegistryId, VarInt};
use mcrs_minecraft_registry::{ChainLookup, Id, Registered, RegistryLookup, RegistrySet};
use mcrs_minecraft_worldgen_feature_place::entity::{
    Equipment as GeneratedEquipment, GeneratedEntity, GeneratedKind, ItemStack as GeneratedStack,
};
use smallvec::SmallVec;

/// `clientTrackingRange` of the spawned kinds, in blocks: eight chunks, the
/// range of every monster; the guardian, shulker, frame and villager see ten.
// chisle: one range for every kind; put the per-kind range on the entity type table if a
// player is meant to see a villager two chunks before a witch.
const TRACKING_RANGE_SQ: f64 = (8.0 * 16.0) * (8.0 * 16.0);

pub struct MobTrackerPlugin;

impl Plugin for MobTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            update_mob_tracked_by
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
    registry: Option<&RegistrySet>,
    entities: Vec<GeneratedEntity>,
) {
    for entity in entities {
        let section_y = (entity.pos[1].floor() as i32).div_euclid(16);
        let Some(&(section, _)) = sections.iter().find(|(_, pos)| pos.y == section_y) else {
            // chisle: an entity of a section the column delivered without is dropped, as a
            // block entity is; keep them in the store per section if a late section must
            // carry them.
            tracing::debug!(pos = ?entity.pos, id = entity.kind.kind().as_static_str(), "an entity outside the delivered sections");
            continue;
        };
        spawn_one(commands, dim, section, registry, entity, None);
    }
}

fn spawn_one(
    commands: &mut Commands,
    dim: InDimension,
    section: Entity,
    registry: Option<&RegistrySet>,
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
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Witch.id()),
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
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Cat.id()),
                Health::full(10.0),
                left_handed(left),
            ));
            if let Some((variant, sound)) =
                registry_id(registry, &variant).zip(registry_id(registry, &sound_variant))
            {
                spawned.insert(CatVariant { variant, sound });
            }
        }
        GeneratedKind::ElderGuardian { left_handed: left } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::ElderGuardian.id()),
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
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Drowned.id()),
                Health::full(20.0),
                left_handed(left),
            ));
            if baby {
                spawned.insert(Baby);
            }
            if !equipment.is_empty() {
                spawned.insert(carried(equipment));
            }
        }
        GeneratedKind::Chicken {
            left_handed: left,
            variant,
            sound_variant,
            ..
        } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Chicken.id()),
                Health::full(4.0),
                left_handed(left),
            ));
            if let Some((variant, sound)) =
                registry_id(registry, &variant).zip(registry_id(registry, &sound_variant))
            {
                spawned.insert(ChickenVariant { variant, sound });
            }
        }
        GeneratedKind::ZombieNautilus {
            left_handed: left,
            variant,
        } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::ZombieNautilus.id()),
                Health::full(15.0),
                left_handed(left),
            ));
            if let Some(variant) = registry_id(registry, &variant) {
                spawned.insert(ZombieNautilusVariant(variant));
            }
        }
        GeneratedKind::Shulker { left_handed: left } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Shulker.id()),
                Health::full(30.0),
                left_handed(left),
            ));
        }
        GeneratedKind::ItemFrame { item, facing } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::ItemFrame.id()),
                ItemFrame {
                    item: Some(stack(item)),
                    facing,
                },
            ));
        }
        GeneratedKind::Evoker { left_handed: left } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Evoker.id()),
                Health::full(24.0),
                left_handed(left),
            ));
        }
        GeneratedKind::Vindicator {
            left_handed: left,
            equipment,
        } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Vindicator.id()),
                Health::full(24.0),
                left_handed(left),
            ));
            if !equipment.is_empty() {
                spawned.insert(carried(equipment));
            }
        }
        GeneratedKind::Allay { left_handed: left } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Allay.id()),
                Health::full(20.0),
                left_handed(left),
            ));
        }
        GeneratedKind::Villager { data } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::Villager.id()),
                Health::full(20.0),
                MobFlags::empty(),
                Villager(data),
            ));
        }
        GeneratedKind::ZombieVillager { data } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::ZombieVillager.id()),
                Health::full(20.0),
                MobFlags::empty(),
                Villager(data),
            ));
        }
        GeneratedKind::ChestMinecart {
            loot_table,
            loot_table_seed,
        } => {
            spawned.insert((
                EntityKind(mcrs_minecraft_entity::keys::EntityType::ChestMinecart.id()),
                mcrs_minecraft_level::entity::mob::ContainerLoot {
                    table: loot_table,
                    seed: loot_table_seed,
                },
            ));
        }
    }
    let id = spawned.id();
    for passenger in entity.passengers {
        spawn_one(commands, dim, section, registry, passenger, Some(id));
    }
}

fn uuid([a, b, c, d]: [i32; 4]) -> Uuid {
    let most = ((a as u32 as u64) << 32) | (b as u32 as u64);
    let least = ((c as u32 as u64) << 32) | (d as u32 as u64);
    Uuid::from_u64_pair(most, least)
}

fn registry_id<R: Registered>(
    registry: Option<&RegistrySet>,
    location: &ResourceLocation,
) -> Option<Id<R>> {
    let id = registry?.registry::<R>()?.by_name(location.as_str());
    if id.is_none() {
        tracing::warn!(registry = %R::REGISTRY, %location, "a spawned entity names a variant the registry lacks");
    }
    id
}

fn stack(stack: GeneratedStack) -> ItemStack {
    ItemStack::new(stack.id.id(), stack.count as u8)
}

fn carried(equipment: GeneratedEquipment) -> Equipment {
    Equipment {
        mainhand: equipment.mainhand.map(stack),
        offhand: equipment.offhand.map(stack),
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

fn wire_id(entity: Entity) -> VarInt {
    VarInt(entity.index_u32() as i32)
}

/// A stack the registries cannot encode is dropped from the packet rather
/// than sent malformed.
fn wire_stack(stack: ItemStack, lookup: &dyn RegistryLookup) -> Option<RawStack> {
    let slot = ProtoStack::new(
        stack.item,
        i32::from(stack.count),
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
        vehicles: &Query<&RiddenBy>,
        lookup: &dyn RegistryLookup,
    ) -> Vec<PacketPayload> {
        let id = wire_id(self.entity);
        let yaw = ByteAngle::from_degrees(self.transform.rotation.yaw());
        let mut out = vec![PacketPayload::PlayerEnteredView(ClientboundAddEntity {
            id,
            uuid: self.uuid.0,
            kind: RegistryId::from(self.kind.0),
            pos: self.transform.translation,
            movement: LpVec3(DVec3::ZERO),
            yaw,
            pitch: ByteAngle::from_degrees(self.transform.rotation.pitch()),
            head_yaw: yaw,
            data: VarInt(self.frame.map_or(0, |frame| frame.facing.id() as i32)),
        })];
        let metadata = self.entity_data(lookup);
        if !metadata.is_empty() {
            out.push(PacketPayload::SetEntityData(ClientboundSetEntityData {
                entity_id: id,
                metadata: Metadata(metadata),
            }));
        }
        if let Some(health) = self.health {
            out.push(PacketPayload::UpdateAttributes(
                ClientboundUpdateAttributes {
                    entity_id: id,
                    attributes: vec![AttributeSnapshot {
                        attribute: RegistryId::from(
                            mcrs_minecraft_entity::keys::Attribute::MaxHealth.id(),
                        ),
                        base: f64::from(health.max),
                        modifiers: Vec::new(),
                    }],
                },
            ));
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
                out.push(PacketPayload::SetEquipment(ClientboundSetEquipment {
                    entity_id: id,
                    slots,
                }));
            }
        }
        if let Some(ridden_by) = self.ridden_by.filter(|riders| !riders.is_empty()) {
            out.push(PacketPayload::SetPassengers(ClientboundSetPassengers {
                vehicle: id,
                passengers: ridden_by.iter().map(|rider| wire_id(*rider)).collect(),
            }));
        }
        if let Some(vehicle) = self.riding
            && let Ok(riders) = vehicles.get(vehicle.0)
        {
            out.push(PacketPayload::SetPassengers(ClientboundSetPassengers {
                vehicle: wire_id(vehicle.0),
                passengers: riders.iter().map(|rider| wire_id(*rider)).collect(),
            }));
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
            put(CAT_VARIANT, MetaDataValue::CatVariant(cat.variant));
            put(CAT_SOUND_VARIANT, MetaDataValue::CatSoundVariant(cat.sound));
        }
        if let Some(chicken) = self.chicken {
            put(
                CHICKEN_VARIANT,
                MetaDataValue::ChickenVariant(chicken.variant),
            );
            put(
                CHICKEN_SOUND_VARIANT,
                MetaDataValue::ChickenSoundVariant(chicken.sound),
            );
        }
        if let Some(nautilus) = self.nautilus {
            put(
                ZOMBIE_NAUTILUS_VARIANT,
                MetaDataValue::ZombieNautilusVariant(nautilus.0),
            );
        }
        if let Some(villager) = self.villager {
            let zombie =
                self.kind.0 == mcrs_minecraft_entity::keys::EntityType::ZombieVillager.id();
            if zombie || villager.0 != VillagerData::default() {
                put(
                    if zombie {
                        ZOMBIE_VILLAGER_DATA
                    } else {
                        VILLAGER_DATA
                    },
                    MetaDataValue::VillagerData(villager.0),
                );
            }
        }
        data
    }
}

fn remove(entity: Entity) -> PacketPayload {
    PacketPayload::PlayerLeftView(ClientboundRemoveEntities {
        entity_ids: vec![wire_id(entity)],
    })
}

/// A player sees a mob when it holds the mob's column and stands within the
/// tracking range of it, horizontally, as the reference decides it.
// chisle: every mob is re-evaluated whenever any transform or observer set changes; index
// mobs per column when their count makes that a cost.
#[allow(clippy::type_complexity)]
pub fn update_mob_tracked_by(
    mut mobs: Query<(&InDimension, &mut TrackedBy, Pairing), With<EntityKind>>,
    vehicles: Query<&RiddenBy>,
    set: Res<RegistrySet>,
    blocks: Res<Blocks>,
    observers: Query<&PlayerObservers, With<Column>>,
    column_indices: Query<&ColumnIndex>,
    players: Query<(&Transform, &HostAnchor), With<Player>>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let lookups = item_lookups(&set, &blocks.0);
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
            let Ok((transform, _)) = players.get(player) else {
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
            let Ok((_, anchor)) = players.get(player) else {
                continue;
            };
            for payload in pairing.packets(&vehicles, &lookup) {
                packets.write(to(anchor.0, payload));
            }
        }
        for &player in tracked_by.0.iter() {
            if !now.contains(&player)
                && let Ok((_, anchor)) = players.get(player)
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
    use crate::world::bus::PacketTarget;
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
            [(to, PacketPayload::PlayerLeftView(ClientboundRemoveEntities { entity_ids }))]
                if *to == anchor && entity_ids.as_slice() == [wire_id(mob)]
        )
    }

    #[test]
    fn a_cat_holds_the_ids_its_set_numbers_its_variants_by() {
        use mcrs_minecraft_entity::variant::{CatSoundVariant, CatVariant as CatVariantValue};

        let set = mcrs_minecraft_world::registries::test_registries();
        let variants = set.registry::<CatVariantValue>().unwrap();
        let sounds = set.registry::<CatSoundVariant>().unwrap();
        let name = |text: &str| ResourceLocation::read(text).unwrap();
        let cat = |variant: &str| GeneratedEntity {
            pos: [0.5, 1.0, 0.5],
            rotation: [0.0, 0.0],
            uuid: [1, 2, 3, 4],
            kind: GeneratedKind::Cat {
                left_handed: false,
                variant: name(variant),
                sound_variant: name("minecraft:classic"),
            },
            passengers: Vec::new(),
        };

        let mut app = App::new();
        let dim = app.world_mut().spawn_empty().id();
        let section = app.world_mut().spawn_empty().id();
        let mut commands = app.world_mut().commands();
        spawn_generated_entities(
            &mut commands,
            InDimension(dim),
            &[(section, SectionPos::new(0, 0, 0))],
            Some(set),
            vec![cat("minecraft:red"), cat("minecraft:not_a_cat")],
        );
        app.world_mut().flush();

        let held: Vec<CatVariant> = app
            .world_mut()
            .query::<&CatVariant>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(
            held,
            [CatVariant {
                variant: variants.require_by_name("minecraft:red").unwrap(),
                sound: sounds.require_by_name("minecraft:classic").unwrap(),
            }],
            "a variant the set lacks leaves the cat with the default"
        );
    }

    #[test]
    fn a_witch_is_paired_with_the_player_holding_its_column_and_removed_with_its_section() {
        let mut app = App::new();
        app.add_schedule(Schedule::new(FixedPostUpdate));
        app.add_message::<OutboundPlayerPacket>();
        let set = crate::world::entity::report_registries();
        app.insert_resource(set.clone());
        app.insert_resource(mcrs_minecraft_world::item::test_corpus().0.clone());
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
        let PacketPayload::PlayerEnteredView(ClientboundAddEntity {
            id,
            kind,
            pos,
            data,
            ..
        }) = &sent[0].1
        else {
            panic!("{sent:?}")
        };
        assert_eq!(*id, wire_id(mob));
        let witch_id = set
            .registry::<mcrs_minecraft_entity::keys::EntityType>()
            .unwrap()
            .by_name("minecraft:witch")
            .unwrap();
        assert_eq!(*kind, RegistryId::from(witch_id));
        assert_eq!(*pos, DVec3::new(8.5, 65.0, 8.5));
        assert_eq!(*data, VarInt(0));
        let PacketPayload::SetEntityData(ClientboundSetEntityData { metadata, .. }) = &sent[1].1
        else {
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
        assert!(matches!(sent[2].1, PacketPayload::UpdateAttributes(_)));
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
