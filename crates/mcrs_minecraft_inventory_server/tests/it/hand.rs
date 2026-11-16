use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::Direction;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;
use mcrs_voxel::BlockPos;

use crate::click::joined;
use crate::click::sent;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::dropped;
use crate::fixture::stacks;

fn act(world: &mut World, player: Entity, action: PlayerAction) {
    let packet = ServerboundPlayerAction {
        action,
        pos: BlockPos::new(0, 0, 0),
        direction: Direction::Down,
        sequence: VarInt(0),
    };
    let mut body = Vec::new();
    packet.encode(&mut body).expect("the packet encodes");
    world.trigger(Received::<ServerboundPlayerAction>::new(
        player,
        body.into(),
    ));
    world.flush();
    world.run_schedule(FixedUpdate);
}

/// INV-SEL-004, IEN-SPN-001: the drop keys take one item or the whole stack out of the selected
/// slot as an item entity, and the client gets the stack that the server holds.
#[test]
fn inv_sel_004_a_drop_key_throws_from_the_selected_slot() {
    struct Case {
        action: PlayerAction,
        hand: Vec<(usize, Kind, u8)>,
        dropped: Vec<(Kind, u8, i16)>,
        sent: Vec<&'static str>,
    }
    let cases = [
        Case {
            action: PlayerAction::DropItem,
            hand: vec![(0, STONE, 4)],
            dropped: vec![(STONE, 1, 40)],
            sent: vec!["slot 36 1x4 state 2"],
        },
        Case {
            action: PlayerAction::DropAllItems,
            hand: vec![],
            dropped: vec![(STONE, 5, 40)],
            sent: vec!["slot 36 empty state 2"],
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = joined(world);

        act(world, player, case.action);

        assert_eq!(stacks(world, player), case.hand, "{:?}", case.action);
        assert_eq!(dropped(world), case.dropped, "{:?}", case.action);
        assert_eq!(sent(world), case.sent, "{:?}", case.action);
    }
}

/// INV-SEL-005: the hand swap exchanges the selected slot and the off-hand slot.
#[test]
fn inv_sel_005_the_hand_swap_exchanges_the_hands() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);

    act(world, player, PlayerAction::SwapItemWithOffhand);

    assert_eq!(stacks(world, player), [(40, STONE, 5)]);
    assert_eq!(
        sent(world),
        ["slot 36 empty state 2", "slot 45 1x5 state 3"]
    );
}

/// INV-SEL-006: a hand swap with a rejection changes no slot, and the client gets both stacks
/// that the server holds.
#[test]
fn inv_sel_006_a_rejected_hand_swap_resends_both_hands() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let stone = ItemStack::new(STONE, Count::new(5).expect("a count above 0"));
    let mut hotbar_only = vec![None; 9];
    hotbar_only[0] = Some(stone);
    world.commands().entity(player).load_inventory(hotbar_only);
    world.flush();

    act(world, player, PlayerAction::SwapItemWithOffhand);

    assert_eq!(stacks(world, player), [(0, STONE, 5)]);
    assert_eq!(
        sent(world),
        ["slot 36 1x5 state 2", "slot 45 empty state 3"]
    );
}
