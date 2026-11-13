use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;

use crate::click::HOTBAR_0;
use crate::click::Packet;
use crate::click::claim;
use crate::click::click;
use crate::click::joined;
use crate::click::sent;
use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::dropped;
use crate::fixture::holders;
use crate::fixture::put;
use crate::fixture::stacks;

fn close(world: &mut World, player: Entity) {
    let packet = ServerboundContainerClose {
        container_id: VarInt(7),
    };
    let mut body = Vec::new();
    packet.encode(&mut body).expect("the packet encodes");
    world.trigger(Received::<ServerboundContainerClose>::new(
        player,
        body.into(),
    ));
    world.flush();
    world.run_schedule(FixedUpdate);
}

fn stack(kind: Kind, count: u8) -> Option<ItemStack> {
    Some(ItemStack::new(
        kind,
        Count::new(count).expect("a count above 0"),
    ))
}

/// INV-CLS-001, INV-CLS-002: a close of any container id moves the cursor stack and then the
/// craft grid into the player inventory by the insert rule. The server answers with no packet;
/// the comparison sends the entries that changed.
#[test]
fn inv_cls_001_a_close_folds_the_cursor_and_the_craft_grid_into_the_inventory() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let holders = holders(world, player);
    put(world, holders.cursor, 0, STONE, 64);
    put(world, holders.craft_grid, 1, DIRT, 2);

    close(world, player);

    assert_eq!(
        stacks(world, player),
        [(0, STONE, 64), (1, STONE, 5), (2, DIRT, 2)]
    );
    assert_eq!(stacks(world, holders.cursor), []);
    assert_eq!(stacks(world, holders.craft_grid), []);
    assert_eq!(dropped(world), []);
    assert_eq!(
        sent(world),
        [
            "slot 36 1x64 state 2",
            "slot 37 1x5 state 3",
            "slot 38 2x2 state 4"
        ]
    );
}

/// INV-CLS-001, IEN-SPN-001: the items that find no room become one item entity for each source
/// stack, with the pickup delay 40.
#[test]
fn inv_cls_001_a_close_drops_what_finds_no_room() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let holders = holders(world, player);
    let mut inventory = vec![stack(DIRT, 64); 36];
    inventory[0] = stack(STONE, 60);
    inventory.resize(41, None);
    world.commands().entity(player).load_inventory(inventory);
    world.flush();
    put(world, holders.cursor, 0, STONE, 10);
    put(world, holders.craft_grid, 0, DIRT, 1);

    close(world, player);

    assert_eq!(stacks(world, player).first(), Some(&(0, STONE, 64)));
    assert_eq!(stacks(world, holders.cursor), []);
    assert_eq!(stacks(world, holders.craft_grid), []);
    assert_eq!(dropped(world), [(STONE, 6, 40), (DIRT, 1, 40)]);
}

/// ITX-MOD-005, INV-CLS-001: a close that follows a pick up in one tick reads the cursor stack
/// that the pick up left, and folds it back into the inventory.
#[test]
fn itx_mod_005_a_close_after_a_click_in_one_tick_folds_the_picked_stack() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);

    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
    );
    close(world, player);

    assert_eq!(stacks(world, player), [(0, STONE, 5)]);
    assert_eq!(stacks(world, holders(world, player).cursor), []);
    assert_eq!(dropped(world), []);
}
