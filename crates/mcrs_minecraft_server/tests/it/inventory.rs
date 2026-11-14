//! The inventory packets reach the inventory of a player through the packet path of the server,
//! and the server keeps that inventory across a leave and a dimension change.

use std::path::Path;
use std::time::{Duration, Instant};

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::persisted_registry::{PersistedRegistry, PersistedSnapshot};
use mcrs_minecraft_engine_item::{Dropped, ItemCommandsExt, Selected, Slots};
use mcrs_minecraft_engine_registries::{install_item_table, test_item_table};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_item_entity::{ItemEntity, Start};
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, ItemStack, Kind};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::Direction;
use mcrs_minecraft_protocol_game::item::{ContainerInput, DelimitedStack};
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundContainerClick, ServerboundContainerClose, ServerboundPlayerAction,
    ServerboundSetCarriedItem, ServerboundSetCreativeModeSlot,
};
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_save::{PlayerDat, read_player_dat};
use mcrs_minecraft_server::{
    VanillaEntityPhysicsPlugin, VanillaItemEntityPlugin, VanillaItemPlugin,
};
use mcrs_minecraft_vanilla_inventory::PlayerHolders;
use mcrs_minecraft_vanilla_item_entity::{Form, ItemEntityCommandsExt, Launch};
use mcrs_voxel::BlockPos;

use crate::commands::load_stone_column;
use crate::host_app::{self, Arrival, PlayServer, label_of};

/// On the stone of [`load_stone_column`].
const FEET: DVec3 = DVec3::new(8.5, 64.0, 8.5);
const OVERWORLD: (&str, &str) = ("minecraft:overworld", "minecraft:overworld");
const THE_END: (&str, &str) = ("minecraft:the_end", "minecraft:the_end");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Placed {
    slot: u16,
    item: Item,
    count: u8,
}

const fn at(slot: u16, item: Item, count: u8) -> Placed {
    Placed { slot, item, count }
}

#[derive(Clone, Copy, Debug)]
enum Sent {
    Click {
        slot: i16,
        button: u8,
        input: ContainerInput,
    },
    Close,
    Hand(PlayerAction),
    Select(u16),
    Creative {
        slot: i16,
        item: Item,
        count: u8,
    },
}

const fn pick_up(slot: i16, button: u8) -> Sent {
    Sent::Click {
        slot,
        button,
        input: ContainerInput::Pickup,
    }
}

fn start(dimensions: &[(&str, &str)]) -> PlayServer {
    let mut app = host_app::make_host_app();
    install_item_table(app.world_mut(), test_item_table()).unwrap();
    app.add_plugins((
        VanillaEntityPhysicsPlugin,
        VanillaItemEntityPlugin,
        VanillaItemPlugin,
    ));
    let mut server = PlayServer::start_in(app, dimensions, &[], 0);
    load_stone_column(&mut server);
    server
}

fn file(inventory: &[Placed]) -> PlayerDat {
    let entries = inventory.iter().map(|placed| {
        let mut stack = NbtCompound::new();
        stack.put_string("id", placed.item.as_static_str().to_owned());
        stack.put_int("count", i32::from(placed.count));
        stack.put_byte("Slot", i8::try_from(placed.slot).unwrap());
        NbtTag::Compound(stack)
    });
    let mut dat = PlayerDat::default();
    dat.root.put_list("Inventory", entries.collect());
    dat
}

/// Joins the player `uuid` whose file is `dat` through the join of the host, and returns its
/// host anchor after the ticks of the join.
fn join(server: &mut PlayServer, uuid: Uuid, dat: &PlayerDat) -> Entity {
    let registry = server.app.world().resource::<PersistedRegistry>();
    let file = PersistedSnapshot::from_dat(dat, registry, Path::new("player.dat")).unwrap();
    let arrival = Arrival {
        uuid,
        name: "holder",
        position: FEET,
        view_distance: 2,
    };
    let anchor = host_app::admit_with_file(&mut server.app, server.dim, &arrival, &file);
    server.ticks(anchor, 3);
    anchor
}

fn send(server: &mut PlayServer, anchor: Entity, sent: Sent) {
    match sent {
        Sent::Click {
            slot,
            button,
            input,
        } => server.queue(
            anchor,
            &ServerboundContainerClick {
                container_id: VarInt(0),
                state_seqno: VarInt(0),
                slot_index: slot,
                button,
                container_input: input,
                changed_slots: Vec::new().into(),
                carried_item: None,
            },
        ),
        Sent::Close => server.queue(
            anchor,
            &ServerboundContainerClose {
                container_id: VarInt(0),
            },
        ),
        Sent::Hand(action) => server.queue(
            anchor,
            &ServerboundPlayerAction {
                action,
                pos: BlockPos::new(0, 0, 0),
                direction: Direction::Down,
                sequence: VarInt(0),
            },
        ),
        Sent::Select(slot) => server.queue(anchor, &ServerboundSetCarriedItem { slot }),
        Sent::Creative { slot, item, count } => {
            let mut bytes = Vec::new();
            for part in [i32::from(count), i32::from(item.id().number()), 0, 0] {
                VarInt(part).encode(&mut bytes).unwrap();
            }
            let item = DelimitedStack::decode(&mut bytes.as_slice()).unwrap();
            server.queue(anchor, &ServerboundSetCreativeModeSlot { slot, item });
        }
    }
}

fn stacks(world: &World, holder: Entity) -> Vec<Placed> {
    let slots = world.get::<Slots>(holder).expect("a holder has slots");
    slots
        .iter()
        .zip(0..)
        .filter_map(|(stack, slot)| {
            let stack = world.entity(stack?);
            let kind = stack.get::<Kind>().expect("a stack has a kind");
            let count = stack.get::<Count>().expect("a stack has a count").get();
            Some(at(slot, Item::from_protocol_id(kind.0).unwrap(), count))
        })
        .collect()
}

fn cursor(world: &World, player: Entity) -> Vec<Placed> {
    let holders = world
        .get::<PlayerHolders>(player)
        .expect("a player has holders");
    stacks(world, holders.cursor)
}

/// The item and the count of each stack that lies in `world`.
fn dropped(world: &mut World) -> Vec<Placed> {
    let mut dropped: Vec<Placed> = world
        .query_filtered::<(&Kind, &Count), With<Dropped>>()
        .iter(world)
        .map(|(kind, count)| at(0, Item::from_protocol_id(kind.0).unwrap(), count.get()))
        .collect();
    dropped.sort_unstable_by_key(|placed| (placed.item, placed.count));
    dropped
}

struct Case {
    name: &'static str,
    file: &'static [Placed],
    sent: &'static [Sent],
    inventory: &'static [Placed],
    cursor: &'static [Placed],
    dropped: &'static [Placed],
}

/// One case for each serverbound inventory packet: a click (ITX-JAV-008 split), a close
/// (INV-CLS-001), the drop key (INV-SEL-004) and the hand swap (INV-SEL-005). The rules themselves
/// are tested in the inventory plugin; a case here fails when the server does not route its packet.
#[test]
fn each_inventory_packet_reaches_the_inventory_through_the_server() {
    use Item::Stone;
    const CASES: [Case; 4] = [
        Case {
            name: "split",
            file: &[at(0, Stone, 5)],
            sent: &[pick_up(36, 1)],
            inventory: &[at(0, Stone, 2)],
            cursor: &[at(0, Stone, 3)],
            dropped: &[],
        },
        Case {
            name: "close",
            file: &[at(0, Stone, 5), at(2, Stone, 5)],
            sent: &[pick_up(38, 0), Sent::Close],
            inventory: &[at(0, Stone, 10)],
            cursor: &[],
            dropped: &[],
        },
        Case {
            name: "drop key",
            file: &[at(0, Stone, 5)],
            sent: &[Sent::Hand(PlayerAction::DropItem)],
            inventory: &[at(0, Stone, 4)],
            cursor: &[],
            dropped: &[at(0, Stone, 1)],
        },
        Case {
            name: "hand swap",
            file: &[at(0, Stone, 5)],
            sent: &[Sent::Hand(PlayerAction::SwapItemWithOffhand)],
            inventory: &[at(40, Stone, 5)],
            cursor: &[],
            dropped: &[],
        },
    ];
    for case in CASES {
        let mut server = start(&[OVERWORLD]);
        let anchor = join(&mut server, Uuid::new_v4(), &file(case.file));
        for &sent in case.sent {
            send(&mut server, anchor, sent);
        }
        server.ticks(anchor, 2);

        let player = server.player(anchor);
        let world = server.world();
        assert_eq!(stacks(world, player), case.inventory, "{}", case.name);
        assert_eq!(cursor(world, player), case.cursor, "{}", case.name);
        assert_eq!(dropped(world), case.dropped, "{}", case.name);
    }
}

/// INV-SEL-002 and INV-INS-001: the selection that arrives in a tick holds for the pickup of that
/// tick, so the picked stack joins the alike stack of the new selected slot.
#[test]
fn inv_sel_002_a_pickup_in_the_tick_of_a_selection_fills_the_new_selected_slot() {
    let mut server = start(&[OVERWORLD]);
    let anchor = join(
        &mut server,
        Uuid::new_v4(),
        &file(&[at(0, Item::Stone, 10), at(4, Item::Stone, 10)]),
    );
    send(&mut server, anchor, Sent::Select(4));
    let world = server.world();
    world
        .commands()
        .spawn_stack(ItemStack::new(
            Kind(Item::Stone.id().number()),
            Count::new(3).unwrap(),
        ))
        .launch(Launch {
            source: BirthSource::Block(BlockPos::new(8, 64, 8)),
            form: Form::Fixed(Start {
                position: FEET,
                velocity: DVec3::ZERO,
            }),
            values: ItemEntity::default(),
        });
    world.flush();
    server.ticks(anchor, 1);

    let player = server.player(anchor);
    let world = server.world();
    assert_eq!(
        stacks(world, player),
        [at(0, Item::Stone, 10), at(4, Item::Stone, 13)]
    );
    assert_eq!(world.get::<Selected>(player), Some(&Selected(4)));
}

/// INV-SAV-001 and INV-SAV-003: the leave writes the slots and the selected slot that the packets
/// gave, and a join with that file holds them again.
#[test]
fn inv_sav_003_a_rejoin_holds_the_inventory_of_the_leave() {
    let mut server = start(&[OVERWORLD]);
    let uuid = Uuid::new_v4();
    let anchor = join(
        &mut server,
        uuid,
        &file(&[at(0, Item::Stone, 5), at(20, Item::Dirt, 7)]),
    );
    send(&mut server, anchor, Sent::Select(4));
    send(
        &mut server,
        anchor,
        Sent::Click {
            slot: 20,
            button: 4,
            input: ContainerInput::Swap,
        },
    );
    server.ticks(anchor, 2);

    let save_dir = std::env::temp_dir().join(format!("mcrs-inventory-rejoin-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&save_dir).unwrap();
    server.world().insert_resource(WorldSave(save_dir.clone()));
    let session = server.session_of(anchor).unwrap();
    let dim = server.dim;
    server.submit_to(dim, ToDim::Despawn(InboundPlayerDespawn { session }));
    let deadline = Instant::now() + Duration::from_secs(30);
    let dat = loop {
        server.ticks_all(1);
        if let Some(dat) = read_player_dat(&save_dir, uuid).unwrap() {
            break dat;
        }
        assert!(
            Instant::now() < deadline,
            "the leave never wrote the player file"
        );
    };
    let _ = std::fs::remove_dir_all(save_dir);

    let anchor = join(&mut server, uuid, &dat);
    let player = server.player(anchor);
    let world = server.world();
    assert_eq!(
        stacks(world, player),
        [at(0, Item::Stone, 5), at(4, Item::Dirt, 7)]
    );
    assert_eq!(world.get::<Selected>(player), Some(&Selected(4)));
}

/// INV-CLS-003 at a dimension change: the cursor stack finds no room in the full inventory, so it
/// lies in the dimension left as an item entity, and the player arrives with an empty cursor. The
/// creative packet refills the slot that the click emptied, and the inventory crosses with it.
#[test]
fn inv_cls_003_a_dimension_change_throws_the_cursor_that_finds_no_room() {
    let mut server = start(&[OVERWORLD, THE_END]);
    let full: Vec<Placed> = (0..36).map(|slot| at(slot, Item::Dirt, 64)).collect();
    let mut dat = file(&full);
    dat.root.put_int("playerGameType", 1);
    let anchor = join(&mut server, Uuid::new_v4(), &dat);
    send(&mut server, anchor, pick_up(36, 0));
    send(
        &mut server,
        anchor,
        Sent::Creative {
            slot: 36,
            item: Item::Stone,
            count: 10,
        },
    );
    server.ticks(anchor, 2);

    let overworld = server.dim;
    let end = label_of(&server.app, THE_END.0);
    let player = server.player(anchor);
    let world = server.world();
    assert_eq!(cursor(world, player), [at(0, Item::Dirt, 64)]);
    let transform = *world.get::<Transform>(player).unwrap();
    world
        .resource_mut::<Messages<ChangeDimensionRequest>>()
        .write(ChangeDimensionRequest {
            player,
            to: ResourceKeyBuf::from_location(ResourceLocationBuf::read(THE_END.0).unwrap()),
            transform,
        });
    server.ticks_all(3);

    let arrived = server
        .player_in(end, anchor)
        .expect("the player is in the End");
    let world = server.world_in(end);
    assert_eq!(cursor(world, arrived), []);
    let mut crossed = full;
    crossed[0] = at(0, Item::Stone, 10);
    assert_eq!(stacks(world, arrived), crossed);
    assert_eq!(dropped(server.world_in(overworld)), [at(0, Item::Dirt, 64)]);
}
