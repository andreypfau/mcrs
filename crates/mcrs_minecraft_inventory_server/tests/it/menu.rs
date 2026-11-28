use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine_inventory_menu::EntryPressed;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Encode;

use crate::click::HOTBAR_0;
use crate::click::Packet;
use crate::click::claim;
use crate::click::click;
use crate::click::joined;
use crate::click::sent;
use crate::click::summary;
use crate::close::close;
use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::holders;
use crate::fixture::put;
use crate::fixture::stacks;

fn open_chest(world: &mut World, player: Entity, chest: Entity, buttons: MenuButtons) {
    let parts = MenuParts::new([chest, player]).expect("two parts");
    world.commands().entity(player).open_menu(
        MenuLayout::of(MenuType::Generic9x3).expect("a generic menu"),
        parts,
        buttons,
    );
    world.flush();
}

#[test]
fn a_click_on_a_button_entry_presses_it_and_moves_nothing() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let chest = world.spawn(Slots::new(27)).id();
    put(world, chest, 13, STONE, 3);
    open_chest(world, player, chest, MenuButtons::default().with(13));
    assert_eq!(sent(world), ["open 1", "content state 1"]);
    let menu = world
        .get::<Menus>(player)
        .and_then(Menus::open)
        .expect("the chest is open");

    click(
        world,
        player,
        Packet::pick_up(1, 13)
            .in_container(1)
            .claiming(vec![(13, None)], claim(STONE, 3)),
    );
    world.run_schedule(FixedUpdate);
    click(
        world,
        player,
        Packet::pick_up_all(2, 13)
            .in_container(1)
            .claiming(vec![(13, None)], claim(STONE, 3)),
    );
    world.run_schedule(FixedUpdate);

    let pressed: Vec<EntryPressed> = world
        .resource_mut::<Messages<EntryPressed>>()
        .drain()
        .collect();
    assert_eq!(
        pressed,
        [EntryPressed {
            viewer: player,
            menu,
            entry: 13,
            button: 0,
        }]
    );
    assert_eq!(stacks(world, chest), [(13, STONE, 3)]);
    assert_eq!(stacks(world, player), [(0, STONE, 5)]);
    assert_eq!(stacks(world, holders(world, player).cursor), []);
    assert_eq!(
        sent(world),
        [
            "slot 13 1x3 state 2",
            "cursor empty",
            "slot 13 1x3 state 3",
            "cursor empty"
        ]
    );
}

#[test]
fn inv_syn_007_a_close_hands_the_shared_client_views_to_the_inventory_menu() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let cursor = holders(world, player).cursor;
    put(world, cursor, 0, STONE, 2);
    world.run_schedule(FixedUpdate);
    assert_eq!(sent(world), ["cursor 1x2"]);
    let chest = world.spawn(Slots::new(27)).id();
    open_chest(world, player, chest, MenuButtons::default());
    assert_eq!(sent(world), ["open 1", "content state 1"]);
    put(world, player, 1, DIRT, 4);
    world.run_schedule(FixedUpdate);
    assert_eq!(sent(world), ["slot 55 2x4 state 2"]);

    close(world, player);

    assert_eq!(stacks(world, player), [(0, STONE, 7), (1, DIRT, 4)]);
    assert_eq!(
        sent(world),
        ["slot 36 1x7 state 2"],
        "P[1] matches the view of the chest, the emptied cursor matches the empty view"
    );

    open_chest(world, player, chest, MenuButtons::default());
    assert_eq!(sent(world), ["open 2", "content state 1"]);
    world.commands().entity(player).close_menu();
    world.flush();
    assert_eq!(sent(world), ["close 2"]);
}

#[test]
fn inv_mnu_009_010_open_screen_carries_the_title_of_the_opener_or_the_empty_text() {
    let cases: [(Option<MenuTitle>, &[u8]); 2] = [
        (
            Some(MenuTitle(Text::text("Shop"))),
            &[0x08, 0x00, 0x04, b'S', b'h', b'o', b'p'],
        ),
        (None, &[0x08, 0x00, 0x00]),
    ];
    for (title, expected) in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = joined(world);
        let chest = world.spawn(Slots::new(27)).id();
        let parts = MenuParts::new([chest, player]).expect("two parts");
        let mut viewer = world.commands();
        let mut viewer = viewer.entity(player);
        match title {
            Some(title) => viewer.open_titled_menu(
                MenuLayout::of(MenuType::Generic9x3).expect("a generic menu"),
                parts,
                MenuButtons::default(),
                title,
            ),
            None => viewer.open_menu(
                MenuLayout::of(MenuType::Generic9x3).expect("a generic menu"),
                parts,
                MenuButtons::default(),
            ),
        };
        world.flush();

        let packets: Vec<OutboundPlayerPacket> = world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect();
        let mut sent_title = Vec::new();
        let mut sent_type = Vec::new();
        for packet in &packets {
            if let PacketPayload::OpenScreen(screen) = &packet.data {
                screen
                    .menu_type
                    .encode(&mut sent_type)
                    .expect("a menu type encodes");
                screen
                    .title
                    .encode(&mut sent_title)
                    .expect("a title encodes");
            }
        }
        assert_eq!(sent_title, expected);
        assert_eq!(sent_type, [2]);
    }
}

fn screens(world: &mut World) -> Vec<String> {
    let packets: Vec<OutboundPlayerPacket> = world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();
    packets
        .iter()
        .filter_map(|packet| match &packet.data {
            PacketPayload::ContainerClose(close) => Some(format!("close {}", close.container_id.0)),
            PacketPayload::OpenScreen(screen) => Some(format!("open {}", screen.container_id.0)),
            PacketPayload::ContainerSetContent(content) => Some(format!(
                "content {} state {} cursor {}",
                content.container_id.0,
                content.state_seqno.0,
                summary(&content.carried_item)
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn inv_mnu_007_011_a_menu_over_a_chest_closes_it_first_and_one_over_the_inventory_keeps_the_cursor()
{
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let cursor = holders(world, player).cursor;
    put(world, cursor, 0, STONE, 2);
    world.run_schedule(FixedUpdate);
    assert_eq!(sent(world), ["cursor 1x2"]);
    let chest = world.spawn(Slots::new(27)).id();

    open_chest(world, player, chest, MenuButtons::default());
    assert_eq!(
        screens(world),
        ["open 1", "content 1 state 1 cursor 1x2"],
        "over the inventory menu the cursor stack stays on the cursor"
    );

    open_chest(world, player, chest, MenuButtons::default());
    assert_eq!(
        screens(world),
        ["close 1", "open 2", "content 2 state 1 cursor empty"],
        "the close of the chest empties the cursor before the new menu exists"
    );
    assert_eq!(stacks(world, player), [(0, STONE, 7)]);
}

#[test]
fn itx_jav_014_a_drag_open_in_the_previous_menu_does_not_reach_the_new_menu() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let chest = world.spawn(Slots::new(27)).id();
    put(world, chest, 13, STONE, 3);
    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
    );
    click(
        world,
        player,
        Packet::drag_start(1).claiming(vec![], claim(STONE, 5)),
    );
    world.run_schedule(FixedUpdate);
    open_chest(world, player, chest, MenuButtons::default());
    let _ = sent(world);

    click(
        world,
        player,
        Packet::pick_up(1, 13)
            .in_container(1)
            .claiming(vec![(13, claim(STONE, 8))], None),
    );
    world.run_schedule(FixedUpdate);

    assert_eq!(stacks(world, chest), [(13, STONE, 8)]);
    assert_eq!(stacks(world, holders(world, player).cursor), []);
}

#[test]
fn itx_jav_014_a_drag_open_under_a_closed_menu_does_not_reach_the_menu_below() {
    let mut app = app();
    let world = app.world_mut();
    let player = joined(world);
    let chest = world.spawn(Slots::new(27)).id();
    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
    );
    click(
        world,
        player,
        Packet::drag_start(1).claiming(vec![], claim(STONE, 5)),
    );
    world.run_schedule(FixedUpdate);
    open_chest(world, player, chest, MenuButtons::default());
    world.commands().entity(player).close_menu();
    world.flush();
    world.run_schedule(FixedUpdate);
    let _ = sent(world);

    click(
        world,
        player,
        Packet::pick_up(1, HOTBAR_0).claiming(vec![(HOTBAR_0, None)], claim(STONE, 5)),
    );
    world.run_schedule(FixedUpdate);

    assert_eq!(stacks(world, player), []);
    assert_eq!(
        stacks(world, holders(world, player).cursor),
        [(0, STONE, 5)]
    );
}
