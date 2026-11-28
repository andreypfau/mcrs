use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;
use mcrs_minecraft_inventory_client::RemoteMenu;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::packets::table::Row;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerClose;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundOpenScreen;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRespawn;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;
use mcrs_minecraft_protocol_game::text::RawText;
use mcrs_minecraft_protocol_game::text::Text;
use mcrs_minecraft_wire::Encode;
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

const OPEN_ID: i32 = 3;
const STALE_ID: i32 = 4;
const STONES: (Kind, u8) = (STONE, 10);
const DIRTS: (Kind, u8) = (DIRT, 3);

fn deliver<R: Row>(world: &mut World, packet: &impl Encode) {
    let connection = world.spawn_empty().id();
    world.trigger(Received::<R>::new(connection, body(packet).into()));
    world.flush();
}

fn open_screen(world: &mut World, container_id: i32) {
    let title = RawText::from_text(&Text::text("Chest")).expect("the title encodes");
    deliver::<ClientboundOpenScreen<'static>>(
        world,
        &ClientboundOpenScreen {
            container_id: VarInt(container_id),
            menu_type: MenuType::Generic9x1,
            title,
        },
    );
}

fn open_menu(world: &World, player: Entity) -> Entity {
    world
        .get::<Menus>(player)
        .and_then(Menus::open)
        .expect("a menu is open")
}

fn container_holder(world: &World, player: Entity) -> Entity {
    world
        .get::<MenuParts>(open_menu(world, player))
        .and_then(|parts| parts.holder(MenuLayout::GENERIC_CONTAINER))
        .expect("the open menu has a container part")
}

fn first_stack(world: &World, holder: Entity) -> Option<(Kind, u8)> {
    let stack = world.get::<Slots>(holder)?.get(0)?;
    let stack = world.entity(stack);
    Some((*stack.get::<Kind>()?, stack.get::<Count>()?.get()))
}

fn set_slot(world: &mut World, container_id: i32, state_id: i32, stack_of: (Kind, u8)) {
    let item = wire(world, Some(&stack(stack_of.0, stack_of.1)));
    deliver::<ClientboundContainerSetSlot>(
        world,
        &ClientboundContainerSetSlot {
            container_id: VarInt(container_id),
            state_seqno: VarInt(state_id),
            slot: 0,
            item,
        },
    );
}

#[derive(Clone, Copy, Debug)]
enum Inbound {
    Slot,
    Content,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Inventory,
    Container,
    Nothing,
}

struct RouteRow {
    inbound: Inbound,
    container_id: i32,
    target: Target,
}

const ROUTE_ROWS: &[RouteRow] = &[
    RouteRow {
        inbound: Inbound::Slot,
        container_id: 0,
        target: Target::Inventory,
    },
    RouteRow {
        inbound: Inbound::Slot,
        container_id: OPEN_ID,
        target: Target::Container,
    },
    RouteRow {
        inbound: Inbound::Slot,
        container_id: STALE_ID,
        target: Target::Nothing,
    },
    RouteRow {
        inbound: Inbound::Content,
        container_id: 0,
        target: Target::Inventory,
    },
    RouteRow {
        inbound: Inbound::Content,
        container_id: OPEN_ID,
        target: Target::Container,
    },
    RouteRow {
        inbound: Inbound::Content,
        container_id: STALE_ID,
        target: Target::Nothing,
    },
];

#[test]
fn a_menu_packet_applies_to_the_menu_of_its_container_id() {
    for row in ROUTE_ROWS {
        let (mut app, player) = client_with_player();
        let world = app.world_mut();
        open_screen(world, OPEN_ID);
        let inventory = world
            .get::<Menus>(player)
            .and_then(|menus| menus.iter().next())
            .expect("the inventory menu stays");
        let state_id = 9;
        match row.inbound {
            Inbound::Slot => set_slot(world, row.container_id, state_id, STONES),
            Inbound::Content => {
                let slot_data = vec![wire(world, Some(&stack(STONES.0, STONES.1)))];
                let carried_item = wire(world, Some(&stack(DIRTS.0, DIRTS.1)));
                deliver::<ClientboundContainerSetContent>(
                    world,
                    &ClientboundContainerSetContent {
                        container_id: VarInt(row.container_id),
                        state_seqno: VarInt(state_id),
                        slot_data,
                        carried_item,
                    },
                );
            }
        }
        let world = app.world();
        let case = (row.inbound, row.container_id);
        let container = world.get::<RemoteMenu>(open_menu(world, player)).copied();
        let inventory_state = world.get::<RemoteMenu>(inventory).map(RemoteMenu::state_id);
        let in_inventory = content(world, 0);
        let in_container = first_stack(world, container_holder(world, player));
        let (expected_inventory, expected_container) = match row.target {
            Target::Inventory => (Some(STONES), None),
            Target::Container => (None, Some(STONES)),
            Target::Nothing => (None, None),
        };
        assert_eq!(in_inventory, expected_inventory, "{case:?}: inventory");
        assert_eq!(in_container, expected_container, "{case:?}: container");
        assert_eq!(
            container.map(|remote| (remote.container_id(), remote.state_id())),
            Some((
                OPEN_ID,
                if row.target == Target::Container {
                    state_id
                } else {
                    0
                }
            )),
            "{case:?}: container menu"
        );
        assert_eq!(
            inventory_state,
            (row.target == Target::Inventory).then_some(state_id),
            "{case:?}: inventory state id"
        );
        if row.target == Target::Container
            && let Inbound::Content = row.inbound
        {
            assert_eq!(cursor(world), Some(DIRTS), "{case:?}: cursor");
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Leave {
    OpenAgain,
    ServerClose,
    ClientClose,
    Respawn,
}

struct LeaveRow {
    leave: Leave,
    open_after: Option<i32>,
    sent: &'static [i32],
}

const LEAVE_ROWS: &[LeaveRow] = &[
    LeaveRow {
        leave: Leave::OpenAgain,
        open_after: Some(STALE_ID),
        sent: &[],
    },
    LeaveRow {
        leave: Leave::ServerClose,
        open_after: None,
        sent: &[],
    },
    LeaveRow {
        leave: Leave::ClientClose,
        open_after: None,
        sent: &[OPEN_ID],
    },
    LeaveRow {
        leave: Leave::Respawn,
        open_after: None,
        sent: &[],
    },
];

#[test]
fn a_container_menu_takes_its_holder_and_the_cursor_with_it() {
    for row in LEAVE_ROWS {
        let (mut app, player) = client_with_player();
        let mut server = Server::join(&mut app);
        let world = app.world_mut();
        open_screen(world, OPEN_ID);
        set_slot(world, OPEN_ID, 1, STONES);
        let contents = wire(world, Some(&stack(DIRTS.0, DIRTS.1)));
        deliver::<ClientboundSetCursorItem>(world, &ClientboundSetCursorItem { contents });
        let holder = container_holder(world, player);
        let held = world
            .get::<Slots>(holder)
            .and_then(|slots| slots.get(0))
            .expect("the container holds the stack");
        match row.leave {
            Leave::OpenAgain => open_screen(world, STALE_ID),
            Leave::ServerClose => deliver::<ClientboundContainerClose>(
                world,
                &ClientboundContainerClose {
                    container_id: VarInt(OPEN_ID),
                },
            ),
            Leave::ClientClose => {
                world.write_message(MenuInput {
                    viewer: player,
                    action: MenuAction::Close,
                });
            }
            Leave::Respawn => {
                let connection = world.spawn_empty().id();
                world.trigger(Received::<ClientboundRespawn>::new(
                    connection,
                    Vec::new().into(),
                ));
                world.flush();
            }
        }
        app.update();
        let world = app.world();
        let leave = row.leave;
        assert!(world.get_entity(holder).is_err(), "{leave:?}: holder");
        assert!(world.get_entity(held).is_err(), "{leave:?}: stack");
        assert_eq!(cursor(world), None, "{leave:?}: cursor");
        let menu = open_menu(world, player);
        let remote = world.get::<RemoteMenu>(menu).map(RemoteMenu::container_id);
        match row.open_after {
            Some(container_id) => {
                assert_eq!(remote, Some(container_id), "{leave:?}: open menu");
                assert_eq!(
                    first_stack(world, container_holder(world, player)),
                    None,
                    "{leave:?}: new container"
                );
            }
            None => assert_eq!(
                world
                    .get::<Menus>(player)
                    .and_then(|menus| menus.iter().next()),
                Some(menu),
                "{leave:?}: the inventory menu is open"
            ),
        }
        app.world_mut().write_message(MenuInput {
            viewer: player,
            action: MenuAction::Click(Click {
                kind: ClickKind::PickUp,
                button: 0,
                clicked: Clicked::Entry(9),
            }),
        });
        app.update();
        let sent: Vec<i32> = server
            .each_before::<ServerboundContainerClose, ServerboundContainerClick>()
            .iter()
            .map(|packet| packet.container_id.0)
            .collect();
        assert_eq!(sent, row.sent, "{leave:?}: sent container ids");
    }
}
