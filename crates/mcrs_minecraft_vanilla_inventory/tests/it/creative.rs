use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;

use crate::click::joined;
use crate::click::sent;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::dropped;
use crate::fixture::stacks;

/// Sends `set_creative_mode_slot` for `slot` with `count` items of `kind`, or the empty stack for
/// the count 0, without running the tick.
fn send(world: &mut World, player: Entity, slot: i16, kind: Kind, count: u8) {
    let mut body = slot.to_be_bytes().to_vec();
    let id = u8::try_from(kind.0).expect("a test item id fits one VarInt byte");
    if count == 0 {
        body.push(0);
    } else {
        body.extend([count, id, 0, 0]);
    }
    world.trigger(Received::<ServerboundSetCreativeModeSlot>::new(
        player,
        body.into(),
    ));
    world.flush();
}

/// INV-CRE-003, INV-CRE-004, INV-CRE-006, IEN-SPN-007: a packet that passes the checks sets the
/// entry past every filter and slot limit, or throws its stack, and sends nothing. A packet that
/// fails a check changes no slot, and the client gets the stack of its entry that the server holds.
#[test]
fn inv_cre_004_the_creative_packet_sets_an_entry_after_its_checks() {
    struct Case {
        name: &'static str,
        mode: GameMode,
        slot: i16,
        count: u8,
        inventory: Vec<(usize, Kind, u8)>,
        dropped: Vec<(Kind, u8, i16)>,
        sent: Vec<&'static str>,
    }
    let cases = [
        Case {
            name: "set of the hotbar",
            mode: GameMode::Creative,
            slot: 36,
            count: 3,
            inventory: vec![(0, STONE, 3)],
            dropped: vec![],
            sent: vec![],
        },
        Case {
            name: "set of the empty stack",
            mode: GameMode::Creative,
            slot: 36,
            count: 0,
            inventory: vec![],
            dropped: vec![],
            sent: vec![],
        },
        Case {
            name: "set of an armour slot past its filter and limit",
            mode: GameMode::Creative,
            slot: 8,
            count: 64,
            inventory: vec![(0, STONE, 5), (36, STONE, 64)],
            dropped: vec![],
            sent: vec![],
        },
        Case {
            name: "creative drop",
            mode: GameMode::Creative,
            slot: -1,
            count: 3,
            inventory: vec![(0, STONE, 5)],
            dropped: vec![(STONE, 3, 40)],
            sent: vec![],
        },
        Case {
            name: "not in creative mode",
            mode: GameMode::Survival,
            slot: 36,
            count: 3,
            inventory: vec![(0, STONE, 5)],
            dropped: vec![],
            sent: vec!["slot 36 1x5 state 2"],
        },
        Case {
            name: "count above the maximum stack size",
            mode: GameMode::Creative,
            slot: 36,
            count: 65,
            inventory: vec![(0, STONE, 5)],
            dropped: vec![],
            sent: vec!["slot 36 1x5 state 2"],
        },
        Case {
            name: "the craft result",
            mode: GameMode::Creative,
            slot: 0,
            count: 3,
            inventory: vec![(0, STONE, 5)],
            dropped: vec![],
            sent: vec!["slot 0 empty state 2"],
        },
        Case {
            name: "past the last entry",
            mode: GameMode::Creative,
            slot: 46,
            count: 3,
            inventory: vec![(0, STONE, 5)],
            dropped: vec![],
            sent: vec![],
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = joined(world);
        world.entity_mut(player).insert(PlayerGameMode(case.mode));

        send(world, player, case.slot, STONE, case.count);
        world.run_schedule(FixedUpdate);

        assert_eq!(stacks(world, player), case.inventory, "{}", case.name);
        assert_eq!(dropped(world), case.dropped, "{}", case.name);
        assert_eq!(sent(world), case.sent, "{}", case.name);
    }
}

/// IEN-SPN-007: a burst throws 74 creative drops; the counter then falls by one each tick.
#[test]
fn ien_spn_007_the_creative_drop_counter_limits_a_burst() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    world
        .entity_mut(player)
        .insert(PlayerGameMode(GameMode::Creative));

    for _ in 0..75 {
        send(world, player, -1, STONE, 1);
    }
    world.run_schedule(FixedUpdate);
    assert_eq!(dropped(world).len(), 74);

    world.resource_mut::<BirthClock>().0 += 1;
    send(world, player, -1, STONE, 1);
    world.run_schedule(FixedUpdate);
    assert_eq!(
        dropped(world).len(),
        75,
        "the counter is 1479 one tick later"
    );
}
