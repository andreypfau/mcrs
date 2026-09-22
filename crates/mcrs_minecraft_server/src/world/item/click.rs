use crate::world::entity::item::launch_thrown_items;
use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::entity::player::player_action::{PlayerAction, PlayerActionKind};
use crate::world::item::chest::close_container_menu;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageCursor, Messages};
use bevy_ecs::prelude::{Local, MessageWriter, On, Query};
use bevy_ecs::system::Command;
use bevy_ecs::world::World;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_inventory::{
    ContainerClickRequest, CurrentMenu, Menu, MenuContainer, MenuSnapshot, Op, Planner, Slot,
    Transaction, player_menu_layout, stack_in,
};
use mcrs_minecraft_item::dropped::THROWN_PICKUP_DELAY;
use mcrs_minecraft_item::{ItemEntry, Items, SlotTable, item_of, slots};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::item::{ItemStackValue, MaxStackSize, RawDelimitedStack};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundContainerClick, ServerboundContainerClose, ServerboundSetCreativeModeSlot,
};
use mcrs_minecraft_registry::ChainLookup;

#[derive(Message, Debug)]
pub struct CreativeSlotRequest {
    pub player: Entity,
    pub slot: i16,
    pub item: RawDelimitedStack,
}

#[derive(Message, Debug)]
pub struct CloseContainerRequest {
    pub player: Entity,
    pub container_id: i32,
}

pub fn decode_container_click(
    event: On<ReceivedPacketEvent>,
    modes: Query<&PlayerGameMode>,
    mut out: MessageWriter<ContainerClickRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundContainerClick>() else {
        return;
    };
    out.write(ContainerClickRequest {
        player: event.entity,
        game_mode: modes
            .get(event.entity)
            .map_or(GameMode::Survival, |mode| mode.0),
        container_id: pkt.container_id.0,
        state_id: pkt.state_seqno.0,
        slot: pkt.slot_index,
        button: pkt.button,
        input: pkt.container_input,
        changed: pkt.changed_slots.0,
        carried: pkt.carried_item,
    });
}

pub fn decode_creative_slot(
    event: On<ReceivedPacketEvent>,
    mut out: MessageWriter<CreativeSlotRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundSetCreativeModeSlot>() else {
        return;
    };
    out.write(CreativeSlotRequest {
        player: event.entity,
        slot: pkt.slot,
        item: pkt.item,
    });
}

pub fn decode_container_close(
    event: On<ReceivedPacketEvent>,
    mut out: MessageWriter<CloseContainerRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundContainerClose>() else {
        return;
    };
    out.write(CloseContainerRequest {
        player: event.entity,
        container_id: pkt.container_id.0,
    });
}

fn snapshot(world: &World, player: Entity) -> MenuSnapshot {
    MenuSnapshot::new(
        world,
        world.resource::<Items>(),
        player,
        player_menu_layout(player),
    )
}

/// Applies a player's planned ops at once and puts what they threw into the world.
pub fn commit(world: &mut World, ops: Vec<Op>) {
    if ops.is_empty() {
        return;
    }
    Transaction(ops).apply(world);
    launch_thrown_items(world);
}

/// Hands the cursor and crafting stacks back to the inventory, dropping what
/// does not fit.
pub fn return_carried(world: &mut World, player: Entity) {
    let mut snapshot = snapshot(world, player);
    let mut planner = Planner::new(&mut snapshot);
    planner.close();
    commit(world, planner.ops);
}

/// ponytail: no drop spam throttle; a per-player counter (20 per drop, -1 per
/// tick, allowed below 1480) is the upgrade.
pub fn handle_creative_slots(world: &mut World) {
    let requests: Vec<CreativeSlotRequest> = world
        .resource_mut::<Messages<CreativeSlotRequest>>()
        .drain()
        .collect();
    if requests.is_empty() {
        return;
    }
    let items = world.resource::<Items>().clone();
    let registry = world.resource::<RegistryAccess>().clone();
    let blocks = world.resource::<Blocks>().clone();
    let lookup = ChainLookup(&[&registry, &*blocks.0]);
    for req in requests {
        if !world
            .get::<PlayerGameMode>(req.player)
            .is_some_and(|mode| mode.0 == GameMode::Creative)
        {
            continue;
        }
        let resolved = req.item.resolve(&lookup).and_then(|slot| {
            (!slot.is_empty())
                .then(|| slot.to_value(&lookup))
                .transpose()
        });
        let value = match resolved {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, player = ?req.player, "a creative stack was rejected");
                continue;
            }
        };
        let valid_slot = (1..=slots::OFFHAND as i16).contains(&req.slot);
        if !valid_slot && req.slot >= 0 {
            continue;
        }
        let slot = Slot::new(req.player, req.slot as u16);
        let existing = stack_in(world, slot).filter(|_| valid_slot);
        let Some(value) = value else {
            if let Some(existing) = existing {
                commit(world, vec![Op::Despawn { stack: existing }]);
            }
            continue;
        };
        let Some(entry) = items
            .id_of(value.item.as_str())
            .and_then(|id| items.get(id))
        else {
            tracing::warn!(item = %value.item, player = ?req.player, "a creative stack names no item");
            continue;
        };
        if value.count.0 > i32::from(max_stack_size_of(&value, entry)) {
            continue;
        }
        if let Some(existing) =
            existing.filter(|existing| item_of(world, *existing) == Some(entry.id))
        {
            commit(
                world,
                vec![Op::Apply {
                    stack: existing,
                    value,
                }],
            );
            continue;
        }
        if !valid_slot {
            let entity = world.spawn_empty().id();
            commit(
                world,
                vec![Op::SpawnDropped {
                    entity,
                    value,
                    pickup_delay: THROWN_PICKUP_DELAY,
                    thrower: Some(req.player),
                }],
            );
            continue;
        }
        let mut ops = Vec::new();
        if let Some(existing) = existing {
            ops.push(Op::Despawn { stack: existing });
        }
        ops.push(Op::Spawn { value, to: slot });
        commit(world, ops);
    }
}

fn max_stack_size_of(value: &ItemStackValue, entry: &ItemEntry) -> u8 {
    value
        .components
        .get::<MaxStackSize>()
        .or_else(|| entry.prototype.get::<MaxStackSize>())
        .map_or(1, |max| max.0.0 as u8)
}

pub fn close_menus(world: &mut World) {
    let requests: Vec<CloseContainerRequest> = world
        .resource_mut::<Messages<CloseContainerRequest>>()
        .drain()
        .collect();
    for req in requests {
        let Some(menu) = world
            .get::<CurrentMenu>(req.player)
            .map(|current| current.0)
        else {
            continue;
        };
        if world
            .get::<Menu>(menu)
            .is_none_or(|menu| i32::from(menu.container_id) != req.container_id)
        {
            continue;
        }
        if world.get::<MenuContainer>(menu).is_some() {
            close_container_menu(world, req.player, menu, false);
            continue;
        }
        world.get_mut::<Menu>(menu).unwrap().drag = None;
        return_carried(world, req.player);
    }
}

pub fn handle_drop_actions(world: &mut World, mut cursor: Local<MessageCursor<PlayerAction>>) {
    let actions: Vec<(Entity, PlayerActionKind, u8)> = cursor
        .read(world.resource::<Messages<PlayerAction>>())
        .filter(|action| {
            matches!(
                action.kind,
                PlayerActionKind::DropItem
                    | PlayerActionKind::DropAllItems
                    | PlayerActionKind::SwapItemWithOffhand
            )
        })
        .map(|action| {
            (
                action.player,
                action.kind.clone(),
                action.selected_hotbar_slot,
            )
        })
        .collect();
    for (player, kind, selected) in actions {
        if world
            .get::<PlayerGameMode>(player)
            .is_some_and(|mode| mode.0 == GameMode::Spectator)
            || world.get::<SlotTable>(player).is_none()
        {
            continue;
        }
        let mut snapshot = snapshot(world, player);
        snapshot.selected = selected;
        let mut planner = Planner::new(&mut snapshot);
        match kind {
            PlayerActionKind::DropItem => planner.drop_held(false),
            PlayerActionKind::DropAllItems => planner.drop_held(true),
            PlayerActionKind::SwapItemWithOffhand => planner.swap_offhand(),
            _ => {}
        }
        commit(world, planner.ops);
    }
}
