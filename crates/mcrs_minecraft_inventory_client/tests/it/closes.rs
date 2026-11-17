use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_client::CloseMenu;
use mcrs_minecraft_inventory_client::MenuClick;
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
                world.write_message(CloseMenu { viewer: player });
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
        world.run_schedule(FixedUpdate);
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
        app.world_mut().write_message(MenuClick {
            viewer: player,
            click: Click {
                kind: ClickKind::PickUp,
                button: 0,
                clicked: Clicked::Entry(9),
            },
        });
        app.world_mut().run_schedule(FixedUpdate);
        app.update();
        let sent: Vec<i32> = server
            .each_before::<ServerboundContainerClose, ServerboundContainerClick>()
            .iter()
            .map(|packet| packet.container_id.0)
            .collect();
        assert_eq!(sent, row.sent, "{close:?}: sent container ids");
    }
}
