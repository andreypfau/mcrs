use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerClose;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;
use mcrs_minecraft_wire::VarInt;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::body;
use crate::fixture::client_with_player;
use crate::fixture::content;
use crate::fixture::cursor;
use crate::fixture::stack;
use crate::fixture::wire;
use crate::server::Server;

#[derive(Clone, Copy, Debug)]
enum Close {
    Client,
    Server { container_id: i32 },
}

struct Row {
    close: Close,
    sent: &'static [i32],
}

const STONES: (Kind, u8) = (STONE, 10);
const DIRTS: (Kind, u8) = (DIRT, 3);
const CRAFT_RESULT_ENTRY: u16 = 0;
const CRAFT_GRID_ENTRY: u16 = 1;
const PICK_UP: Click = Click {
    kind: ClickKind::PickUp,
    button: 0,
    clicked: Clicked::Entry(9),
};

const ROWS: &[Row] = &[
    Row {
        close: Close::Client,
        sent: &[0],
    },
    Row {
        close: Close::Server { container_id: 0 },
        sent: &[],
    },
    Row {
        close: Close::Server { container_id: 5 },
        sent: &[],
    },
];

fn set_up(world: &mut World, connection: Entity) {
    for (entry, (kind, count)) in [(CRAFT_RESULT_ENTRY, STONES), (CRAFT_GRID_ENTRY, DIRTS)] {
        let item = wire(world, Some(&stack(kind, count)));
        let body = body(&ClientboundContainerSetSlot {
            container_id: VarInt(0),
            state_seqno: VarInt(5),
            slot: i16::try_from(entry).expect("an entry fits a slot index"),
            item,
        });
        world.trigger(Received::<ClientboundContainerSetSlot>::new(
            connection,
            body.into(),
        ));
    }
    let contents = wire(world, Some(&stack(STONES.0, STONES.1)));
    let body = body(&ClientboundSetCursorItem { contents });
    world.trigger(Received::<ClientboundSetCursorItem>::new(
        connection,
        body.into(),
    ));
    world.flush();
}

#[test]
fn a_close_empties_the_craft_result_and_keeps_the_cursor_and_the_grid() {
    for row in ROWS {
        let (mut app, player) = client_with_player();
        let mut server = Server::join(&mut app);
        let world = app.world_mut();
        let connection = world.spawn_empty().id();
        set_up(world, connection);
        match row.close {
            Close::Client => {
                world.write_message(MenuInput {
                    viewer: player,
                    action: MenuAction::Close,
                });
            }
            Close::Server { container_id } => {
                let body = body(&ClientboundContainerClose {
                    container_id: VarInt(container_id),
                });
                world.trigger(Received::<ClientboundContainerClose>::new(
                    connection,
                    body.into(),
                ));
                world.flush();
            }
        }
        app.update();
        let world = app.world();
        let close = row.close;
        assert_eq!(
            content(world, CRAFT_RESULT_ENTRY),
            None,
            "{close:?}: result"
        );
        assert_eq!(
            content(world, CRAFT_GRID_ENTRY),
            Some(DIRTS),
            "{close:?}: grid"
        );
        assert_eq!(cursor(world), Some(STONES), "{close:?}: cursor");
        app.world_mut().write_message(MenuInput {
            viewer: player,
            action: MenuAction::Click(PICK_UP),
        });
        app.update();
        let sent: Vec<i32> = server
            .each_before::<ServerboundContainerClose, ServerboundContainerClick>()
            .iter()
            .map(|packet| packet.container_id.0)
            .collect();
        assert_eq!(sent, row.sent, "{close:?}: sent container ids");
    }
}

struct OrderRow {
    actions: [MenuAction; 2],
    clicks_before_close: usize,
}

const ORDER_ROWS: &[OrderRow] = &[
    OrderRow {
        actions: [MenuAction::Click(PICK_UP), MenuAction::Close],
        clicks_before_close: 1,
    },
    OrderRow {
        actions: [MenuAction::Close, MenuAction::Click(PICK_UP)],
        clicks_before_close: 0,
    },
];

#[test]
fn a_click_and_a_close_of_one_frame_are_sent_in_the_order_they_are_written() {
    for row in ORDER_ROWS {
        let (mut app, player) = client_with_player();
        let mut server = Server::join(&mut app);
        let world = app.world_mut();
        let connection = world.spawn_empty().id();
        set_up(world, connection);
        for action in row.actions.clone() {
            world.write_message(MenuInput {
                viewer: player,
                action,
            });
        }
        app.update();
        let world = app.world();
        let actions = row.actions.clone();
        assert_eq!(content(world, 9), Some(STONES), "{actions:?}: placed");
        assert_eq!(cursor(world), None, "{actions:?}: cursor");
        assert_eq!(
            content(world, CRAFT_RESULT_ENTRY),
            None,
            "{actions:?}: result"
        );
        let clicks = server.each_before::<ServerboundContainerClick, ServerboundContainerClose>();
        assert_eq!(
            clicks.len(),
            row.clicks_before_close,
            "{actions:?}: clicks before the close"
        );
    }
}
