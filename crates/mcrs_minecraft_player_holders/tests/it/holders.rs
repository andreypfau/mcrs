use std::sync::LazyLock;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Equippable;
use mcrs_minecraft_item_stack::EquippableSlot;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_player_holders::PlayerHoldersPlugin;
use mcrs_minecraft_registries::CurrentRegistries;

const STONE: Kind = Kind(1);
const FEET: u16 = 36;
const HEAD: u16 = 39;

static TABLE: LazyLock<ItemTable> =
    LazyLock::new(|| ItemTable::leak(vec![vec![Builtin::MaxStackSize(MaxStackSize(64))]; 2]));

struct PlaceRow {
    name: &'static str,
    holder: Entity,
    index: u16,
    wear: Option<EquippableSlot>,
    expected: Filter,
}

fn place_answer(world: &mut World, row: &PlaceRow, player: Entity) -> Filter {
    let holder = row.holder;
    let mut stack = ItemStack::new(STONE, Count::new(1).expect("a count above 0"));
    if let Some(slot) = row.wear {
        stack = stack.set(Equippable::new(slot));
    }
    let stack = world.commands().spawn_stack(stack).id();
    world.flush();
    let place = world
        .get::<SlotPolicy>(holder)
        .expect("a holder has a slot policy")
        .access(row.index)
        .place;
    let query = GateQuery {
        slot: SlotRef {
            holder,
            index: row.index,
        },
        actor: player,
        stack,
    };
    place.answer(world, &query)
}

#[test]
fn inv_slt_001_002_003_a_new_player_holds_four_holders_and_the_slot_policy_gates_places() {
    let mut app = App::new();
    app.insert_resource(CurrentRegistries::new(test_registries().clone()))
        .insert_resource(*TABLE)
        .add_plugins((ItemPlugin::default(), PlayerHoldersPlugin));
    let world = app.world_mut();
    let player = world.spawn(Player).id();
    world.flush();
    let holders = *world
        .get::<PlayerHolders>(player)
        .expect("a new player has holders");

    let sizes = [
        player,
        holders.cursor,
        holders.craft_grid,
        holders.craft_result,
    ]
    .map(|holder| world.get::<Slots>(holder).map(Slots::len));
    assert_eq!(
        sizes,
        [Some(41), Some(1), Some(4), Some(1)],
        "the inventory, the cursor, the craft grid and the craft result have their sizes"
    );

    let rows = [
        PlaceRow {
            name: "stone into the craft result",
            holder: holders.craft_result,
            index: 0,
            wear: None,
            expected: Filter::Refuses,
        },
        PlaceRow {
            name: "stone into the head slot",
            holder: player,
            index: HEAD,
            wear: None,
            expected: Filter::Refuses,
        },
        PlaceRow {
            name: "a feet stack into the head slot",
            holder: player,
            index: HEAD,
            wear: Some(EquippableSlot::Feet),
            expected: Filter::Refuses,
        },
        PlaceRow {
            name: "a feet stack into the feet slot",
            holder: player,
            index: FEET,
            wear: Some(EquippableSlot::Feet),
            expected: Filter::Permits,
        },
    ];
    for row in rows {
        assert_eq!(
            place_answer(world, &row, player),
            row.expected,
            "{}",
            row.name
        );
    }
}
