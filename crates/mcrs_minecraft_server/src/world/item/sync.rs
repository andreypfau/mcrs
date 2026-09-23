use crate::world::aoi::TrackedBy;
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Changed, Ref, With};
use bevy_ecs::world::World;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_inventory::{
    CurrentMenu, Menu, MenuLayout, MenuViewer, Remote, RemoteSlots, Slot, stack_in,
};
use mcrs_minecraft_item::{
    DroppedItem, Held, Items, SlotTable, StackRevision, WireStack, slots, stack_to_slot,
};
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_protocol::entity::{MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_protocol::item::{ProtoStack, RawStack};
use mcrs_minecraft_registry::ChainLookup;
use rustc_hash::FxHashSet;

const DROPPED_ITEM_STACK_INDEX: u8 = 8;

/// Holders whose slots may read differently since the last run: those whose
/// table changed, and the top-level holder of every stack whose revision
/// moved, so a damaged pickaxe inside a shulker marks the chest slot.
fn dirty_holders(world: &mut World) -> FxHashSet<Entity> {
    let mut dirty: FxHashSet<Entity> = world
        .query_filtered::<Entity, Changed<SlotTable>>()
        .iter(world)
        .collect();
    let revised: Vec<Entity> = world
        .query_filtered::<Entity, (Changed<StackRevision>, With<Held>)>()
        .iter(world)
        .collect();
    for stack in revised {
        let mut top = stack;
        while let Some(held) = world.get::<Held>(top) {
            top = held.holder;
        }
        dirty.insert(top);
    }
    dirty
}

/// Sends every open menu the slots its viewer does not hold yet, and every
/// tracked dropped item its stack when that changed.
pub fn sync_stack_slots(world: &mut World) {
    let items = world.resource::<Items>().clone();
    let registry = world.resource::<RegistryAccess>().clone();
    let blocks = world.resource::<Blocks>().clone();
    let lookup = ChainLookup(&[&registry, &*blocks.0]);
    let dirty = dirty_holders(world);
    let proto = |world: &World, slot: Slot| -> ProtoStack {
        stack_in(world, slot).map_or(ProtoStack::EMPTY, |stack| {
            stack_to_slot(world, stack, &items)
        })
    };
    let raw = |proto: &ProtoStack| -> RawStack {
        RawStack::from_stack(proto, &lookup)
            .inspect_err(|error| tracing::warn!(%error, "a stack could not be encoded"))
            .unwrap_or(RawStack::EMPTY)
    };
    let mut out = Vec::new();

    let mut menus = world.query::<(Entity, &MenuLayout, &MenuViewer, Ref<RemoteSlots>)>();
    let open: Vec<(Entity, Entity, Vec<Slot>)> = menus
        .iter(world)
        .filter(|(menu, layout, viewer, remote)| {
            world
                .get::<CurrentMenu>(viewer.0)
                .is_some_and(|current| current.0 == *menu)
                && (remote.full
                    || remote.is_changed()
                    || dirty.contains(&viewer.0)
                    || layout.0.iter().any(|slot| dirty.contains(&slot.holder)))
        })
        .map(|(menu, layout, viewer, _)| (menu, viewer.0, layout.0.clone()))
        .collect();
    for (menu, viewer, layout) in open {
        let stacks: Vec<ProtoStack> = layout.iter().map(|slot| proto(world, *slot)).collect();
        let carried = proto(world, Slot::new(viewer, slots::CARRIED));
        let container_id = world.get::<Menu>(menu).unwrap().container_id;
        let mut remote = world.get_mut::<RemoteSlots>(menu).unwrap();
        remote.slots.resize(stacks.len(), Remote::Unknown);
        if remote.full {
            let slots: Vec<RawStack> = stacks.iter().map(raw).collect();
            let carried = raw(&carried);
            remote.slots = slots.iter().cloned().map(Remote::Known).collect();
            remote.carried = Remote::Known(carried.clone());
            remote.full = false;
            let state_id = world.get_mut::<Menu>(menu).unwrap().next_state_id();
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
            continue;
        }
        let mut stale: Vec<(i16, RawStack)> = Vec::new();
        for (index, current) in stacks.iter().enumerate() {
            if let Some(known) = agree(&remote.slots[index], current, raw) {
                remote.slots[index] = known;
            } else {
                let encoded = raw(current);
                remote.slots[index] = Remote::Known(encoded.clone());
                stale.push((index as i16, encoded));
            }
        }
        let cursor = match agree(&remote.carried, &carried, raw) {
            Some(known) => {
                remote.carried = known;
                None
            }
            None => {
                let encoded = raw(&carried);
                remote.carried = Remote::Known(encoded.clone());
                Some(encoded)
            }
        };
        for (slot, item) in stale {
            let state_id = world.get_mut::<Menu>(menu).unwrap().next_state_id();
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
        if let Some(item) = cursor {
            out.push(to(world, viewer, PacketPayload::SetCursorItem(item)));
        }
    }

    let revised: Vec<Entity> = world
        .query_filtered::<Entity, (Changed<StackRevision>, With<DroppedItem>)>()
        .iter(world)
        .collect();
    for root in revised {
        let encoded = raw(&stack_to_slot(world, root, &items));
        if world
            .get::<WireStack>(root)
            .is_some_and(|wire| wire.0 == encoded)
        {
            continue;
        }
        let targets = TrackedBy::anchors(world, root);
        world.entity_mut(root).insert(WireStack(encoded.clone()));
        out.push(OutboundPlayerPacket {
            target: PacketTarget::PlayerSet(targets),
            priority: PacketPriority::Normal,
            data: PacketPayload::SetEntityData {
                entity_id: root.index_u32() as i32,
                metadata: Metadata(vec![MetadataEntry {
                    index: DROPPED_ITEM_STACK_INDEX,
                    value: MetaDataValue::Slot(encoded),
                }]),
            },
            session: PlayerSession(0),
            epoch: 0,
        });
    }

    if !out.is_empty() {
        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .write_batch(out);
    }
}

/// The remote entry to keep when the client already holds `current`: a known
/// copy that encodes the same, or a claim that hashes the same and is now
/// pinned to the encoding.
fn agree(
    remote: &Remote,
    current: &ProtoStack,
    raw: impl Fn(&ProtoStack) -> RawStack,
) -> Option<Remote> {
    match remote {
        Remote::Unknown => None,
        Remote::Known(known) => (*known == raw(current)).then(|| remote.clone()),
        Remote::Claimed(hashed) => {
            let matches = hashed
                .as_ref()
                .map_or(current.is_empty(), |hashed| hashed.matches(current));
            matches.then(|| Remote::Known(raw(current)))
        }
    }
}

pub(crate) fn to(world: &World, player: Entity, data: PacketPayload) -> OutboundPlayerPacket {
    let anchor = world
        .get::<HostAnchor>(player)
        .map_or(Entity::PLACEHOLDER, |anchor| anchor.0);
    crate::world::bus::to(anchor, data)
}
