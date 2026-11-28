use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRespawn;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetPlayerInventory;
use mcrs_minecraft_wire::VarInt;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::body;
use crate::fixture::client_with_player;
use crate::fixture::content;
use crate::fixture::cursor;
use crate::fixture::stack;
use crate::fixture::stacks;
use crate::fixture::state_id;
use crate::fixture::wire;

#[derive(Clone, Copy)]
enum Inbound {
    Content,
    Slot {
        index: i16,
        state_id: i32,
        stack: Option<(Kind, u8)>,
    },
    ForeignSlot,
    PlayerInventory {
        index: i32,
        stack: (Kind, u8),
    },
    Cursor {
        stack: (Kind, u8),
    },
    Respawn,
}

#[derive(Clone, Copy)]
enum Place {
    Entry(u16),
    Cursor,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Session {
    Items,
    NoItemTable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OldStack {
    Kept,
    Despawned,
}

struct Case {
    name: &'static str,
    session: Session,
    inbound: &'static [Inbound],
    expected: &'static [(Place, Option<(Kind, u8)>)],
    stacks: usize,
    state_id: Option<i32>,
    old_stack: OldStack,
}

const CONTENT_STATE_ID: i32 = 7;

fn owned(stack_of: (Kind, u8)) -> ItemStack {
    stack(stack_of.0, stack_of.1)
}

fn encode(world: &World, inbound: Inbound) -> Vec<u8> {
    match inbound {
        Inbound::Content => body(&ClientboundContainerSetContent {
            container_id: VarInt(0),
            state_seqno: VarInt(CONTENT_STATE_ID),
            slot_data: (1..=46)
                .map(|count| wire(world, Some(&stack(STONE, count))))
                .collect(),
            carried_item: wire(world, Some(&stack(DIRT, 5))),
        }),
        Inbound::Slot {
            index,
            state_id,
            stack,
        } => body(&ClientboundContainerSetSlot {
            container_id: VarInt(0),
            state_seqno: VarInt(state_id),
            slot: index,
            item: wire(world, stack.map(owned).as_ref()),
        }),
        Inbound::ForeignSlot => body(&ClientboundContainerSetSlot {
            container_id: VarInt(5),
            state_seqno: VarInt(3),
            slot: 36,
            item: wire(world, Some(&stack(NEW.0, NEW.1))),
        }),
        Inbound::PlayerInventory { index, stack } => body(&ClientboundSetPlayerInventory {
            slot: VarInt(index),
            contents: wire(world, Some(&owned(stack))),
        }),
        Inbound::Cursor { stack } => body(&ClientboundSetCursorItem {
            contents: wire(world, Some(&owned(stack))),
        }),
        Inbound::Respawn => Vec::new(),
    }
}

fn deliver_in_one_frame(world: &mut World, session: Session, inbound: &[Inbound]) {
    let bodies: Vec<Vec<u8>> = inbound
        .iter()
        .map(|packet| encode(world, *packet))
        .collect();
    if session == Session::NoItemTable {
        world.remove_resource::<ItemTable>();
    }
    let connection = world.spawn_empty().id();
    for (packet, body) in inbound.iter().zip(bodies) {
        let body = body.into();
        let mut commands = world.commands();
        match packet {
            Inbound::Content => {
                commands.trigger(Received::<ClientboundContainerSetContent>::new(
                    connection, body,
                ));
            }
            Inbound::Slot { .. } | Inbound::ForeignSlot => {
                commands.trigger(Received::<ClientboundContainerSetSlot>::new(
                    connection, body,
                ));
            }
            Inbound::PlayerInventory { .. } => {
                commands.trigger(Received::<ClientboundSetPlayerInventory>::new(
                    connection, body,
                ));
            }
            Inbound::Cursor { .. } => {
                commands.trigger(Received::<ClientboundSetCursorItem>::new(connection, body));
            }
            Inbound::Respawn => {
                commands.trigger(Received::<ClientboundRespawn>::new(connection, body));
            }
        }
    }
    world.flush();
}

const OLD: Option<(Kind, u8)> = Some((DIRT, 9));
const NEW: (Kind, u8) = (STONE, 4);

const CASES: &[Case] = &[
    Case {
        name: "a slot inside the menu takes the stack and the state id",
        session: Session::Items,
        inbound: &[Inbound::Slot {
            index: 36,
            state_id: 3,
            stack: Some(NEW),
        }],
        expected: &[(Place::Entry(36), Some(NEW))],
        stacks: 1,
        state_id: Some(3),
        old_stack: OldStack::Despawned,
    },
    Case {
        name: "an empty stack from the server clears the entry",
        session: Session::Items,
        inbound: &[Inbound::Slot {
            index: 36,
            state_id: 3,
            stack: None,
        }],
        expected: &[(Place::Entry(36), None)],
        stacks: 0,
        state_id: Some(3),
        old_stack: OldStack::Despawned,
    },
    Case {
        name: "a slot of another container changes nothing and sets no state id",
        session: Session::Items,
        inbound: &[Inbound::ForeignSlot],
        expected: &[(Place::Entry(36), OLD)],
        stacks: 1,
        state_id: None,
        old_stack: OldStack::Kept,
    },
    Case {
        name: "a slot outside the menu changes nothing and keeps the state id",
        session: Session::Items,
        inbound: &[
            Inbound::Slot {
                index: 46,
                state_id: 3,
                stack: Some(NEW),
            },
            Inbound::Slot {
                index: -1,
                state_id: 4,
                stack: Some(NEW),
            },
        ],
        expected: &[(Place::Entry(36), OLD)],
        stacks: 1,
        state_id: None,
        old_stack: OldStack::Kept,
    },
    Case {
        name: "a player inventory index below 9 is a hotbar entry",
        session: Session::Items,
        inbound: &[Inbound::PlayerInventory {
            index: 3,
            stack: NEW,
        }],
        expected: &[(Place::Entry(36), OLD), (Place::Entry(39), Some(NEW))],
        stacks: 2,
        state_id: None,
        old_stack: OldStack::Kept,
    },
    Case {
        name: "the player inventory index 40 is the off hand",
        session: Session::Items,
        inbound: &[Inbound::PlayerInventory {
            index: 40,
            stack: NEW,
        }],
        expected: &[(Place::Entry(45), Some(NEW))],
        stacks: 2,
        state_id: None,
        old_stack: OldStack::Kept,
    },
    Case {
        name: "a player inventory index outside 0 to 40 changes nothing",
        session: Session::Items,
        inbound: &[
            Inbound::PlayerInventory {
                index: 41,
                stack: NEW,
            },
            Inbound::PlayerInventory {
                index: -1,
                stack: NEW,
            },
        ],
        expected: &[(Place::Entry(36), OLD)],
        stacks: 1,
        state_id: None,
        old_stack: OldStack::Kept,
    },
    Case {
        name: "the cursor item takes the stack and keeps the state id",
        session: Session::Items,
        inbound: &[Inbound::Cursor { stack: NEW }],
        expected: &[(Place::Cursor, Some(NEW))],
        stacks: 2,
        state_id: None,
        old_stack: OldStack::Kept,
    },
    Case {
        name: "a slot after the content in one frame wins",
        session: Session::Items,
        inbound: &[
            Inbound::Content,
            Inbound::Slot {
                index: 36,
                state_id: 8,
                stack: Some(NEW),
            },
        ],
        expected: &[
            (Place::Entry(36), Some(NEW)),
            (Place::Entry(9), Some((STONE, 10))),
            (Place::Cursor, Some((DIRT, 5))),
        ],
        stacks: 47,
        state_id: Some(8),
        old_stack: OldStack::Despawned,
    },
    Case {
        name: "a respawn gives the inventory menu the state id 0 and keeps the stacks",
        session: Session::Items,
        inbound: &[Inbound::Content, Inbound::Respawn],
        expected: &[(Place::Entry(9), Some((STONE, 10)))],
        stacks: 47,
        state_id: None,
        old_stack: OldStack::Despawned,
    },
    Case {
        name: "a packet without the item table changes nothing",
        session: Session::NoItemTable,
        inbound: &[
            Inbound::Content,
            Inbound::Slot {
                index: 36,
                state_id: 3,
                stack: Some(NEW),
            },
        ],
        expected: &[(Place::Entry(36), OLD), (Place::Cursor, None)],
        stacks: 1,
        state_id: None,
        old_stack: OldStack::Kept,
    },
    Case {
        name: "a slot with the stack it holds keeps the stack entity",
        session: Session::Items,
        inbound: &[Inbound::Slot {
            index: 36,
            state_id: 3,
            stack: Some((DIRT, 9)),
        }],
        expected: &[(Place::Entry(36), OLD)],
        stacks: 1,
        state_id: Some(3),
        old_stack: OldStack::Kept,
    },
    Case {
        name: "a slot with a new count of the same stack keeps the stack entity",
        session: Session::Items,
        inbound: &[Inbound::Slot {
            index: 36,
            state_id: 3,
            stack: Some((DIRT, 2)),
        }],
        expected: &[(Place::Entry(36), Some((DIRT, 2)))],
        stacks: 1,
        state_id: Some(3),
        old_stack: OldStack::Kept,
    },
];

#[test]
fn inbound_packets_set_the_holders_and_the_state_id() {
    for case in CASES {
        let (mut app, player) = client_with_player();
        let world = app.world_mut();
        world
            .commands()
            .spawn_stack(stack(DIRT, 9))
            .put_at(player, 0);
        world.flush();
        let old = world
            .get::<Slots>(player)
            .and_then(|slots| slots.get(0))
            .expect("the old stack is in the first hotbar slot");

        deliver_in_one_frame(world, case.session, case.inbound);

        for (place, stack) in case.expected {
            let actual = match place {
                Place::Entry(entry) => content(world, *entry),
                Place::Cursor => cursor(world),
            };
            assert_eq!(actual, *stack, "{}", case.name);
        }
        assert_eq!(stacks(world), case.stacks, "{}", case.name);
        assert_eq!(state_id(world), case.state_id, "{}", case.name);
        let old_stack = if world.get_entity(old).is_ok() {
            OldStack::Kept
        } else {
            OldStack::Despawned
        };
        assert_eq!(old_stack, case.old_stack, "{}", case.name);
    }
}
