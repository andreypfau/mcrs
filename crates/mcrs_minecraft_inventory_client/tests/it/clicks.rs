use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetPlayerInventory;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
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

type Held = Option<(Kind, u8)>;

struct Row {
    name: &'static str,
    inventory: &'static [(i32, (Kind, u8))],
    cursor: Held,
    state_id: Option<i32>,
    kind: ClickKind,
    button: u8,
    entry: u16,
    entries: &'static [(u16, Held)],
    changed: &'static [(u16, Held)],
    carried: Held,
}

const STONES: (Kind, u8) = (STONE, 10);
const DIRTS: (Kind, u8) = (DIRT, 3);

const ROWS: &[Row] = &[
    Row {
        name: "a left click takes the whole stack",
        inventory: &[(0, STONES)],
        cursor: None,
        state_id: Some(5),
        kind: ClickKind::PickUp,
        button: 0,
        entry: 36,
        entries: &[(36, None)],
        changed: &[(36, None)],
        carried: Some(STONES),
    },
    Row {
        name: "a right click takes half of the stack",
        inventory: &[(0, STONES)],
        cursor: None,
        state_id: Some(5),
        kind: ClickKind::PickUp,
        button: 1,
        entry: 36,
        entries: &[(36, Some((STONE, 5)))],
        changed: &[(36, Some((STONE, 5)))],
        carried: Some((STONE, 5)),
    },
    Row {
        name: "a shift click moves the stack to the first free main entry",
        inventory: &[(0, STONES), (9, DIRTS)],
        cursor: None,
        state_id: Some(5),
        kind: ClickKind::QuickMove,
        button: 0,
        entry: 36,
        entries: &[(9, Some(DIRTS)), (10, Some(STONES)), (36, None)],
        changed: &[(10, Some(STONES)), (36, None)],
        carried: None,
    },
    Row {
        name: "a hotbar key swaps two different stacks",
        inventory: &[(0, STONES), (9, DIRTS)],
        cursor: None,
        state_id: Some(5),
        kind: ClickKind::Swap,
        button: 0,
        entry: 9,
        entries: &[(9, Some(STONES)), (36, Some(DIRTS))],
        changed: &[(9, Some(STONES)), (36, Some(DIRTS))],
        carried: None,
    },
    Row {
        name: "a swap of two equal stacks lists no entry",
        inventory: &[(0, STONES), (9, STONES)],
        cursor: None,
        state_id: Some(5),
        kind: ClickKind::Swap,
        button: 0,
        entry: 9,
        entries: &[(9, Some(STONES)), (36, Some(STONES))],
        changed: &[],
        carried: None,
    },
    Row {
        name: "a refused click is sent with no change and the state id 0",
        inventory: &[],
        cursor: Some(STONES),
        state_id: None,
        kind: ClickKind::PickUp,
        button: 0,
        entry: 5,
        entries: &[(5, None)],
        changed: &[],
        carried: Some(STONES),
    },
];

fn set_up(world: &mut World, row: &Row) {
    let connection = world.spawn_empty().id();
    for &(slot, (kind, count)) in row.inventory {
        let contents = wire(world, Some(&stack(kind, count)));
        let body = body(&ClientboundSetPlayerInventory {
            slot: VarInt(slot),
            contents,
        });
        world.trigger(Received::<ClientboundSetPlayerInventory>::new(
            connection,
            body.into(),
        ));
    }
    if let Some((kind, count)) = row.cursor {
        let contents = wire(world, Some(&stack(kind, count)));
        let body = body(&ClientboundSetCursorItem { contents });
        world.trigger(Received::<ClientboundSetCursorItem>::new(
            connection,
            body.into(),
        ));
    }
    if let Some(state_id) = row.state_id {
        let item = wire(world, None);
        let body = body(&ClientboundContainerSetSlot {
            container_id: VarInt(0),
            state_seqno: VarInt(state_id),
            slot: 20,
            item,
        });
        world.trigger(Received::<ClientboundContainerSetSlot>::new(
            connection,
            body.into(),
        ));
    }
    world.flush();
}

fn held(hashed: Option<&HashedStack>) -> Held {
    let hashed = hashed?;
    let kind = hashed.kind;
    let count = u8::try_from(hashed.count).expect("a hashed count fits a stack");
    Some((kind, count))
}

#[test]
fn a_click_is_predicted_and_sent_with_its_claims() {
    for row in ROWS {
        let (mut app, player) = client_with_player();
        let mut server = Server::join(&mut app);
        set_up(app.world_mut(), row);
        let click = Click {
            kind: row.kind,
            button: row.button,
            clicked: Clicked::Entry(row.entry),
        };
        app.world_mut().write_message(MenuInput {
            viewer: player,
            action: MenuAction::Click(click),
        });
        app.update();
        let world = app.world();
        for &(entry, expected) in row.entries {
            assert_eq!(
                content(world, entry),
                expected,
                "{}: entry {entry}",
                row.name
            );
        }
        assert_eq!(cursor(world), row.carried, "{}: cursor", row.name);
        let packet = server.next::<ServerboundContainerClick>();
        assert_eq!(packet.container_id.0, 0, "{}", row.name);
        assert_eq!(
            packet.state_seqno.0,
            row.state_id.unwrap_or(0),
            "{}",
            row.name
        );
        assert_eq!(packet.click, click, "{}", row.name);
        let mut changed: Vec<(u16, Held)> = (packet.changed_slots.0.iter())
            .map(|(entry, hashed)| (*entry, held(hashed.as_ref())))
            .collect();
        changed.sort_unstable_by_key(|(entry, _)| *entry);
        assert_eq!(changed, row.changed, "{}: changed slots", row.name);
        assert_eq!(
            held(packet.carried_item.as_ref()),
            row.carried,
            "{}: carried item",
            row.name
        );
    }
}
