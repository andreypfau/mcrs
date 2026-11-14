use bevy::prelude::*;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_item::value;
use mcrs_minecraft_item::{Held, ItemStack, Items, ProtoStack, SlotTable, StackRevision, slots};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundContainerSetContent, ClientboundContainerSetSlot, ClientboundSetCursorItem,
    ClientboundSetPlayerInventory,
};
use mcrs_minecraft_protocol_game::item::{ItemStackValue, WireLookup};
use mcrs_minecraft_registries::{CurrentRegistries, FromWire, Wire};

use crate::player::Player;

/// The container id of the player inventory menu, which is open whenever no other menu is.
pub const INVENTORY_MENU: i32 = 0;

/// Mirrors the stacks the server sends for the player inventory menu and the carried item into
/// the [`SlotTable`] of the local player.
pub struct InventoryPacketsPlugin;

impl Plugin for InventoryPacketsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_set_content)
            .add_observer(on_set_slot)
            .add_observer(on_set_player_inventory)
            .add_observer(on_set_cursor_item);
    }
}

fn on_set_content(
    event: On<Received<ClientboundContainerSetContent>>,
    player: Single<Entity, With<Player>>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if packet.container_id.0 != INVENTORY_MENU {
        return;
    }
    let holder = *player;
    commands.queue(move |world: &mut World| {
        for (index, wire) in (0..).zip(packet.slot_data).take(slots::MENU_COUNT) {
            place(world, holder, index, wire);
        }
        place(world, holder, slots::CARRIED, packet.carried_item);
    });
}

fn on_set_slot(
    event: On<Received<ClientboundContainerSetSlot>>,
    player: Single<Entity, With<Player>>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok(index) = u16::try_from(packet.slot) else {
        return;
    };
    if packet.container_id.0 != INVENTORY_MENU || usize::from(index) >= slots::MENU_COUNT {
        return;
    }
    let holder = *player;
    commands.queue(move |world: &mut World| place(world, holder, index, packet.item));
}

fn on_set_player_inventory(
    event: On<Received<ClientboundSetPlayerInventory>>,
    player: Single<Entity, With<Player>>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Some(index) = u8::try_from(packet.slot.0)
        .ok()
        .and_then(slots::from_inventory_index)
    else {
        return;
    };
    let holder = *player;
    commands.queue(move |world: &mut World| place(world, holder, index, packet.contents));
}

fn on_set_cursor_item(
    event: On<Received<ClientboundSetCursorItem>>,
    player: Single<Entity, With<Player>>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let holder = *player;
    commands.queue(move |world: &mut World| {
        place(world, holder, slots::CARRIED, packet.contents);
    });
}

fn place(world: &mut World, holder: Entity, index: u16, wire: ProtoStack<Wire>) {
    let old = world
        .get::<SlotTable>(holder)
        .and_then(|table| table.get(index));
    if let Some(old) = old {
        world.despawn(old);
    }
    let Some(stack) = resolve(world, wire) else {
        return;
    };
    let value = match stack.to_value() {
        Ok(value) => value,
        Err(error) => {
            warn!(%error, "the server sent a stack of the inventory that is not valid");
            return;
        }
    };
    let spawned =
        world.try_resource_scope::<Items, _>(|world, items| spawn_value(world, &items, &value));
    if let Some(Some(stack)) = spawned {
        world.entity_mut(stack).insert(Held { holder, index });
    }
}

fn resolve(world: &World, wire: ProtoStack<Wire>) -> Option<ProtoStack> {
    let registries = world.get_resource::<CurrentRegistries>()?;
    match ProtoStack::from_wire(wire, &WireLookup::new(registries)) {
        Ok(stack) if !stack.is_empty() => Some(stack),
        Ok(_) => None,
        Err(error) => {
            warn!(%error, "the server sent a stack of the inventory that does not resolve");
            None
        }
    }
}

/// Spawns a stack equal to `value`, with each stack its contents hold as a held child.
fn spawn_value(world: &mut World, items: &Items, value: &ItemStackValue) -> Option<Entity> {
    let entry = match value::named_entry(items, value) {
        Ok(entry) => entry,
        Err(error) => {
            warn!(%error, "the server sent a stack of an item the client does not know");
            return None;
        }
    };
    let Ok(count) = u8::try_from(value.count.0) else {
        warn!(
            count = value.count.0,
            "the server sent a stack count out of range"
        );
        return None;
    };
    let effective = entry.prototype.apply(&value.components);
    let mut entity = world.spawn((
        ItemStack {
            item: entry.id,
            count,
        },
        StackRevision::default(),
    ));
    let mut children = Vec::new();
    for kind in DataComponentType::ALL.iter().copied() {
        let Some(given) = effective.get_value(kind) else {
            continue;
        };
        if value::is_child_kind(kind) {
            children = value::child_targets(given).unwrap_or_default();
        } else if let Err(error) = (value::ops(kind).insert)(&mut entity, given) {
            warn!(%error, "a component of an inventory stack does not apply");
        }
    }
    let stack = entity.id();
    if !children.is_empty() {
        world
            .entity_mut(stack)
            .insert(SlotTable::fixed(children.len()));
    }
    for (index, child) in (0..).zip(children) {
        if let Some(child) = child.and_then(|child| spawn_value(world, items, child)) {
            world.entity_mut(child).insert(Held {
                holder: stack,
                index,
            });
        }
    }
    Some(stack)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use mcrs_minecraft_engine_registries::test_corpus;
    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use mcrs_minecraft_item::{ItemStack, ProtoStack, SlotTable, slots};
    use mcrs_minecraft_network::event::Received;
    use mcrs_minecraft_protocol::packets::table::Row;
    use mcrs_minecraft_protocol::{Encode, VarInt};
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundContainerSetContent, ClientboundContainerSetSlot, ClientboundSetCursorItem,
        ClientboundSetPlayerInventory,
    };
    use mcrs_minecraft_registries::{CurrentRegistries, Wire, WireId};

    use super::InventoryPacketsPlugin;
    use crate::player::Player;

    fn stack(path: &str, count: i32) -> ProtoStack<Wire> {
        let id = test_corpus().1.id_of(path).unwrap();
        ProtoStack {
            id: WireId::from(id),
            count,
            ..ProtoStack::default()
        }
    }

    fn send<P: Row + Encode>(app: &mut App, player: Entity, packet: &P) {
        let mut bytes = Vec::new();
        packet.encode(&mut bytes).unwrap();
        app.world_mut()
            .trigger(Received::<P>::new(player, bytes.into()));
        app.world_mut().flush();
    }

    fn held(app: &App, player: Entity, index: u16) -> Option<(String, u8)> {
        let world = app.world();
        let stack = world.get::<SlotTable>(player)?.get(index)?;
        let stack = world.get::<ItemStack>(stack)?;
        let items = &test_corpus().1;
        Some((items.get(stack.item)?.identifier.to_string(), stack.count))
    }

    #[test]
    fn the_player_slots_follow_the_inventory_packets() {
        let mut app = App::new();
        app.insert_resource(test_corpus().1.clone())
            .insert_resource(CurrentRegistries::new(test_registries().clone()))
            .add_plugins(InventoryPacketsPlugin);
        let player = app
            .world_mut()
            .spawn((Player, SlotTable::fixed(slots::COUNT)))
            .id();
        let mut content = vec![ProtoStack::default(); slots::MENU_COUNT];
        content[usize::from(slots::OFFHAND)] = stack("minecraft:stick", 3);
        send(
            &mut app,
            player,
            &ClientboundContainerSetContent {
                container_id: VarInt(0),
                state_seqno: VarInt(1),
                slot_data: content,
                carried_item: stack("minecraft:stone", 64),
            },
        );
        send(
            &mut app,
            player,
            &ClientboundSetPlayerInventory {
                slot: VarInt(2),
                contents: stack("minecraft:apple", 5),
            },
        );
        send(
            &mut app,
            player,
            &ClientboundContainerSetSlot {
                container_id: VarInt(0),
                state_seqno: VarInt(2),
                slot: i16::try_from(slots::OFFHAND).unwrap(),
                item: stack("minecraft:stick", 7),
            },
        );
        send(
            &mut app,
            player,
            &ClientboundSetCursorItem {
                contents: ProtoStack::default(),
            },
        );
        let stick = Some(("minecraft:stick".to_owned(), 7));
        let apple = Some(("minecraft:apple".to_owned(), 5));
        assert_eq!(held(&app, player, slots::OFFHAND), stick);
        assert_eq!(held(&app, player, slots::held(2)), apple);
        assert_eq!(held(&app, player, slots::CARRIED), None);
        assert_eq!(
            app.world_mut()
                .query::<&ItemStack>()
                .iter(app.world())
                .count(),
            2
        );
    }
}
