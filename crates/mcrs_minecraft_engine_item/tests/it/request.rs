use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::InSlot;
use mcrs_minecraft_engine_item::Reason;
use mcrs_minecraft_engine_item::Rejected;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Rarity;

use crate::fixture::LORED;
use crate::fixture::PLAIN;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::spawn;
use crate::fixture::value;

const ID: RequestId = RequestId(7);

fn rare(items: u8) -> ItemStack {
    ItemStack::new(PLAIN, count(items)).set(Rarity::Rare)
}

fn place(world: &mut World, value: ItemStack, holder: Entity, index: u16) -> Entity {
    let stack = spawn(world, value);
    world.commands().entity(stack).put_at(holder, index);
    world.flush();
    stack
}

fn request(world: &mut World, holder: Entity, request: Request) -> Vec<RequestResult> {
    world.commands().entity(holder).request(request);
    world.flush();
    world
        .resource_mut::<Messages<RequestResult>>()
        .drain()
        .collect()
}

fn slots(world: &World, holder: Entity) -> Vec<Option<Entity>> {
    world.get::<Slots>(holder).unwrap().iter().collect()
}

fn items(world: &World, stack: Entity) -> Option<u8> {
    world.get::<Count>(stack).map(|count| count.get())
}

#[derive(Debug, PartialEq)]
struct Stack {
    entity: Entity,
    items: u8,
    slot: Option<InSlot>,
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    stacks: Vec<Stack>,
    slots: Vec<Vec<Option<Entity>>>,
}

fn snapshot(world: &mut World) -> Snapshot {
    let mut stacks: Vec<Stack> = world
        .query::<(Entity, &Count, Option<&InSlot>)>()
        .iter(world)
        .map(|(entity, count, slot)| Stack {
            entity,
            items: count.get(),
            slot: slot.copied(),
        })
        .collect();
    stacks.sort_by_key(|stack| stack.entity);
    let mut holders: Vec<(Entity, Vec<Option<Entity>>)> = world
        .query::<(Entity, &Slots)>()
        .iter(world)
        .map(|(holder, slots)| (holder, slots.iter().collect()))
        .collect();
    holders.sort_by_key(|holder| holder.0);
    let slots = holders.into_iter().map(|holder| holder.1).collect();
    Snapshot { stacks, slots }
}

struct Layout {
    player: Entity,
    bare: Entity,
}

impl Layout {
    fn slot(&self, index: u16) -> SlotRef {
        SlotRef {
            holder: self.player,
            index,
        }
    }
}

#[derive(Clone, Copy)]
enum Requester {
    Player,
    Bare,
}

struct Case {
    name: &'static str,
    request: fn(&Layout) -> Request,
    requester: Requester,
    rejected: Rejected,
}

fn rejected(op: usize, reason: Reason) -> Rejected {
    Rejected { op, reason }
}

#[test]
fn a_rejected_request_changes_nothing_and_reports_its_reason() {
    let cases = [
        Case {
            name: "more than 128 operations",
            request: |at| {
                (0..129).fold(Request::new(ID), |request, _| {
                    request.swap(at.slot(0), at.slot(3))
                })
            },
            requester: Requester::Player,
            rejected: rejected(128, Reason::TooManyOps),
        },
        Case {
            name: "an index past the end, after a valid operation",
            request: |at| {
                Request::new(ID)
                    .split(at.slot(0), at.slot(3), 1)
                    .split(at.slot(0), at.slot(4), 1)
            },
            requester: Requester::Player,
            rejected: rejected(1, Reason::NoSuchSlot),
        },
        Case {
            name: "a holder without slots",
            request: |at| {
                Request::new(ID).merge(
                    SlotRef {
                        holder: at.bare,
                        index: 0,
                    },
                    at.slot(3),
                )
            },
            requester: Requester::Bare,
            rejected: rejected(0, Reason::NoSuchSlot),
        },
        Case {
            name: "the same slot",
            request: |at| Request::new(ID).merge(at.slot(0), at.slot(0)),
            requester: Requester::Player,
            rejected: rejected(0, Reason::SameSlot),
        },
        Case {
            name: "a split of zero",
            request: |at| Request::new(ID).split(at.slot(0), at.slot(3), 0),
            requester: Requester::Player,
            rejected: rejected(0, Reason::BadCount),
        },
        Case {
            name: "an empty source",
            request: |at| Request::new(ID).merge(at.slot(3), at.slot(0)),
            requester: Requester::Player,
            rejected: rejected(0, Reason::EmptySource),
        },
        Case {
            name: "more items than the source holds",
            request: |at| Request::new(ID).split(at.slot(0), at.slot(3), 11),
            requester: Requester::Player,
            rejected: rejected(0, Reason::NotEnough),
        },
        Case {
            name: "a different stack in the target, after a valid operation",
            request: |at| {
                Request::new(ID)
                    .split(at.slot(0), at.slot(3), 2)
                    .merge(at.slot(3), at.slot(2))
            },
            requester: Requester::Player,
            rejected: rejected(1, Reason::NotEqual),
        },
        Case {
            name: "a count past the maximum stack size",
            request: |at| Request::new(ID).split(at.slot(0), at.slot(1), 5),
            requester: Requester::Player,
            rejected: rejected(0, Reason::NoRoom),
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = world.spawn(Slots::new(4)).id();
        let bare = world.spawn_empty().id();
        place(world, rare(10), player, 0);
        place(world, rare(60), player, 1);
        place(world, ItemStack::new(LORED, count(5)), player, 2);
        let layout = Layout { player, bare };
        let holder = match case.requester {
            Requester::Player => player,
            Requester::Bare => bare,
        };
        let before = snapshot(world);

        let results = request(world, holder, (case.request)(&layout));

        assert_eq!(snapshot(world), before, "{}", case.name);
        let expected = RequestResult {
            holder,
            id: ID,
            outcome: Err(case.rejected),
        };
        assert_eq!(results, [expected], "{}", case.name);
    }
}

#[derive(Resource, Default)]
struct Observed(Vec<RequestResult>);

#[test]
fn an_applied_request_moves_merges_splits_and_swaps_and_keeps_the_entities() {
    let mut app = app();
    app.init_resource::<Observed>().add_observer(
        |result: On<RequestResult>, mut observed: ResMut<Observed>| observed.0.push(*result),
    );
    let world = app.world_mut();
    let player = world.spawn(Slots::new(5)).id();
    let cursor = world.spawn(Slots::new(1)).id();
    let source = place(world, rare(10), player, 0);
    let full = place(world, rare(60), player, 1);
    let lored = place(world, ItemStack::new(LORED, count(5)), player, 2);
    let merged = place(world, rare(3), player, 4);
    let at = |index| SlotRef {
        holder: player,
        index,
    };
    let cursor_slot = SlotRef {
        holder: cursor,
        index: 0,
    };
    let stacks_before = world.query::<&Kind>().iter(world).count();

    let results = request(
        world,
        player,
        Request::new(ID)
            .split(at(0), at(3), 4)
            .merge(at(0), at(1))
            .swap(at(2), cursor_slot)
            .merge(at(4), at(0)),
    );

    let expected = RequestResult {
        holder: player,
        id: ID,
        outcome: Ok(()),
    };
    assert_eq!(results, [expected]);
    assert_eq!(world.resource::<Observed>().0, [expected]);
    let [Some(kept), Some(filled), None, Some(split), None] = slots(world, player)[..] else {
        panic!("slots of the player: {:?}", slots(world, player));
    };
    assert_eq!(
        (kept, filled),
        (source, full),
        "a merge keeps the target entity"
    );
    assert_eq!(
        slots(world, cursor),
        [Some(lored)],
        "a swap moves the entity"
    );
    assert!(
        world.get_entity(merged).is_err(),
        "an emptied source despawns"
    );
    assert_eq!(items(world, source), Some(5));
    assert_eq!(items(world, full), Some(64));
    assert_eq!(items(world, lored), Some(5));
    assert_eq!(
        value(world, split),
        Some(rare(4)),
        "a split copies the components"
    );
    assert_eq!(
        world.query::<&Kind>().iter(world).count(),
        stacks_before,
        "one stack despawned and one split spawned"
    );
}

#[test]
fn a_request_on_a_despawned_holder_does_nothing() {
    let mut app = app();
    let world = app.world_mut();
    let player = world.spawn(Slots::new(1)).id();
    world.despawn(player);
    let at = SlotRef {
        holder: player,
        index: 0,
    };

    let results = request(world, player, Request::new(ID).merge(at, at));

    assert_eq!(results, []);
}

// INV-CRE-001
#[test]
fn a_set_puts_an_unplaced_stack_copies_a_placed_one_with_its_count_and_ends_the_stacks_it_replaces()
{
    let mut app = app();
    let world = app.world_mut();
    let player = world.spawn(Slots::new(4)).id();
    let replaced = place(world, rare(10), player, 0);
    let held = place(world, ItemStack::new(LORED, count(5)), player, 2);
    let emptied = place(world, rare(3), player, 3);
    let unplaced = spawn(world, rare(40));
    let at = |index| SlotRef {
        holder: player,
        index,
    };

    let results = request(
        world,
        player,
        Request::new(ID)
            .set(at(0), Some(unplaced), 12)
            .set(at(1), Some(held), 64)
            .set(at(3), None, 0),
    );

    assert_eq!(
        results,
        [RequestResult {
            holder: player,
            id: ID,
            outcome: Ok(()),
        }]
    );
    let [Some(first), Some(copy), Some(kept), None] = slots(world, player)[..] else {
        panic!("slots of the player: {:?}", slots(world, player));
    };
    assert_eq!((first, kept), (unplaced, held));
    assert_ne!(copy, held, "a stack in a slot is copied");
    assert_eq!(value(world, copy), Some(ItemStack::new(LORED, count(64))));
    assert_eq!(value(world, held), Some(ItemStack::new(LORED, count(5))));
    assert_eq!(value(world, first), Some(rare(12)));
    assert!(world.get_entity(replaced).is_err(), "a replaced stack ends");
    assert!(world.get_entity(emptied).is_err(), "an emptied stack ends");
    assert_eq!(world.query::<&Kind>().iter(world).count(), 3);
}
