use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Reason;
use mcrs_minecraft_engine_item::Refusal;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_inventory_minigame::ArenaRulesPlugin;
use mcrs_minecraft_inventory_minigame::EnterGame;
use mcrs_minecraft_inventory_minigame::Frozen;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_vanilla_inventory::PlayerInventories;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

const DENIAL_SOURCE: RequestId = RequestId(40);
const REQUEST: RequestId = RequestId(9);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        ItemPlugin::new(ItemTable::leak((0..3).map(|_| Vec::new()))),
        VanillaInventoryPlugin::default(),
        ArenaRulesPlugin {
            denial_source: DENIAL_SOURCE,
        },
    ));
    app
}

fn items() -> PlayerItems {
    let stack = |kind: u16| ItemStack::new(Kind(kind), Count::new(5).unwrap());
    let mut items = PlayerItems::default();
    items.slots[8] = Some(stack(1));
    items.slots[10] = Some(stack(2));
    items
}

fn spawn_player(world: &mut World) -> Entity {
    let player = world.spawn(Player).id();
    world.commands().entity(player).restore_items(items());
    world.flush();
    player
}

fn capture(world: &mut World, player: Entity) -> PlayerItems {
    world
        .run_system_once(move |mut inventories: PlayerInventories| inventories.capture(player))
        .unwrap()
        .unwrap()
}

fn move_one(world: &mut World, player: Entity, from: u16) -> Result<(), Refusal> {
    let at = |index: u16| SlotRef {
        holder: player,
        index,
    };
    world
        .commands()
        .entity(player)
        .request(Request::new(REQUEST).split(at(from), at(20), 1));
    world.flush();
    let results: Vec<RequestResult> = world
        .resource_mut::<Messages<RequestResult>>()
        .drain()
        .collect();
    assert_eq!(results.len(), 1, "one request gives one result");
    results.first().unwrap().outcome
}

#[test]
fn a_player_in_the_game_cannot_take_from_the_locked_slot() {
    let mut app = app();
    let world = app.world_mut();
    let player = spawn_player(world);
    world.trigger(EnterGame { entity: player });
    world.flush();

    let outcome = move_one(world, player, 8).map_err(|refusal| match refusal {
        Refusal::Rejected(rejected) => Some(rejected.reason),
        _ => None,
    });
    assert_eq!(outcome, Err(Some(Reason::TakeDenied)));
    assert_eq!(
        capture(world, player),
        items(),
        "a refused take changes no slot"
    );

    assert_eq!(
        move_one(world, player, 10),
        Ok(()),
        "a take from another slot commits"
    );
}

#[test]
fn a_request_of_a_frozen_player_is_denied() {
    let mut app = app();
    let world = app.world_mut();
    let frozen = spawn_player(world);
    world.entity_mut(frozen).insert(Frozen);
    let free = spawn_player(world);

    assert_eq!(
        move_one(world, frozen, 10),
        Err(Refusal::Denied(Denial {
            source: DENIAL_SOURCE,
            code: 1,
        }))
    );
    assert_eq!(
        capture(world, frozen),
        items(),
        "a denied request changes no slot"
    );

    assert_eq!(
        move_one(world, free, 10),
        Ok(()),
        "a request of a player that is not frozen commits"
    );
}
