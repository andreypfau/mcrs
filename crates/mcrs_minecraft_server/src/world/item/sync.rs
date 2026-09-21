use crate::world::aoi::TrackedBy;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;
use crate::world::item::menu::{Menu, MenuLayout, MenuViewer};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::{
    DirtyStacks, DroppedItem, Items, SelectedHotbarSlot, SlotTable, WireStack, slots, stack_to_slot,
};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_protocol::entity::{MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_protocol::item::RawStack;
use mcrs_minecraft_registry::{ChainLookup, RegistryLookup};
use smallvec::SmallVec;

const DROPPED_ITEM_STACK_INDEX: u8 = 8;

#[derive(Resource, Default, Debug)]
pub struct MenuResync {
    pub held_slot: Vec<Entity>,
    pub menus_full: Vec<Entity>,
}

/// ponytail: every dirty cell is resent, including ones the client predicted
/// correctly; vanilla skips those by hashing the remote slot. Upgrade: keep a
/// hashed copy per menu cell and compare before sending.
pub fn sync_stack_slots(world: &mut World) {
    let mut dirty = std::mem::take(&mut *world.resource_mut::<DirtyStacks>());
    let resync = std::mem::take(&mut *world.resource_mut::<MenuResync>());
    if dirty.cells.is_empty()
        && dirty.roots.is_empty()
        && resync.held_slot.is_empty()
        && resync.menus_full.is_empty()
    {
        return;
    }
    dirty.cells.sort_unstable();
    dirty.cells.dedup();
    dirty.roots.sort_unstable();
    dirty.roots.dedup();

    let Some(items) = world.get_resource::<Items>().cloned() else {
        return;
    };
    let registry = world.resource::<RegistryAccess>().clone();
    let blocks = world.resource::<Blocks>().clone();
    let lookups: [&dyn RegistryLookup; 2] = [&registry, &*blocks.0];
    let lookup = ChainLookup(&lookups);
    let encode = |world: &World, holder: Entity, index: u16| -> RawStack {
        world
            .get::<SlotTable>(holder)
            .and_then(|table| table.get(index))
            .map(|stack| raw_stack(world, stack, &items, &lookup))
            .unwrap_or(RawStack::EMPTY)
    };
    let mut out = Vec::new();

    for player in resync.held_slot {
        if let Some(selected) = world.get::<SelectedHotbarSlot>(player) {
            out.push(to(world, player, PacketPayload::SetHeldSlot(selected.0)));
        }
    }

    for menu in resync.menus_full {
        let Some((layout, viewer)) = world
            .get::<MenuLayout>(menu)
            .zip(world.get::<MenuViewer>(menu))
            .map(|(layout, viewer)| (layout.0.clone(), viewer.0))
        else {
            continue;
        };
        let slots = layout
            .iter()
            .map(|&(holder, index)| encode(world, holder, index))
            .collect();
        let carried = encode(world, viewer, slots::CARRIED);
        let state_id = world.get_mut::<Menu>(menu).unwrap().next_state_id();
        let container_id = world.get::<Menu>(menu).unwrap().container_id;
        out.push(to(
            world,
            viewer,
            PacketPayload::ContainerSetContent {
                container_id,
                state_id,
                slots,
                carried,
            },
        ));
    }

    let mut menus = world.query::<(Entity, &MenuLayout, &MenuViewer)>();
    for (holder, index) in dirty.cells {
        if index == slots::CARRIED && world.get::<Player>(holder).is_some() {
            out.push(to(
                world,
                holder,
                PacketPayload::SetCursorItem(encode(world, holder, index)),
            ));
            continue;
        }
        let hits: Vec<(Entity, Entity, i16)> = menus
            .iter(world)
            .filter_map(|(menu, layout, viewer)| {
                let slot = layout.0.iter().position(|cell| *cell == (holder, index))?;
                Some((menu, viewer.0, slot as i16))
            })
            .collect();
        for (menu, viewer, slot) in hits {
            let item = encode(world, holder, index);
            let state_id = world.get_mut::<Menu>(menu).unwrap().next_state_id();
            let container_id = world.get::<Menu>(menu).unwrap().container_id;
            out.push(to(
                world,
                viewer,
                PacketPayload::ContainerSetSlot {
                    container_id,
                    state_id,
                    slot,
                    item,
                },
            ));
        }
    }

    for root in dirty.roots {
        if world.get::<DroppedItem>(root).is_none() {
            continue;
        }
        let raw = raw_stack(world, root, &items, &lookup);
        if world
            .get::<WireStack>(root)
            .is_some_and(|wire| wire.0 == raw)
        {
            continue;
        }
        let targets: SmallVec<[Entity; 8]> = world
            .get::<TrackedBy>(root)
            .map(|tracked| {
                tracked
                    .0
                    .iter()
                    .filter_map(|&player| world.get::<HostAnchor>(player).map(|anchor| anchor.0))
                    .collect()
            })
            .unwrap_or_default();
        world.entity_mut(root).insert(WireStack(raw.clone()));
        out.push(OutboundPlayerPacket {
            target: PacketTarget::PlayerSet(targets),
            priority: PacketPriority::Normal,
            data: PacketPayload::SetEntityData {
                entity_id: root.index_u32() as i32,
                metadata: Metadata(vec![MetadataEntry {
                    index: DROPPED_ITEM_STACK_INDEX,
                    value: MetaDataValue::Slot(raw),
                }]),
            },
            session: PlayerSession(0),
            epoch: 0,
        });
    }

    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write_batch(out);
}

fn raw_stack(world: &World, stack: Entity, items: &Items, lookup: &dyn RegistryLookup) -> RawStack {
    let slot = stack_to_slot(world, stack, items);
    RawStack::from_slot(&slot, lookup)
        .inspect_err(|error| tracing::warn!(%error, ?stack, "a stack could not be encoded"))
        .unwrap_or(RawStack::EMPTY)
}

pub(crate) fn to(world: &World, player: Entity, data: PacketPayload) -> OutboundPlayerPacket {
    let anchor = world
        .get::<HostAnchor>(player)
        .map_or(Entity::PLACEHOLDER, |anchor| anchor.0);
    OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(anchor),
        priority: PacketPriority::Normal,
        data,
        session: PlayerSession(0),
        epoch: 0,
    }
}
