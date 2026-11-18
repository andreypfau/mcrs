use bevy_app::App;
use bevy_app::AppExit;
use bevy_ecs::prelude::*;
use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_client::local_server::LocalServerPlugin;
use mcrs_minecraft_client::test_support::ServerAnswers;
use mcrs_minecraft_client::test_support::count_server_answers;
use mcrs_minecraft_client::test_support::creative_player;
use mcrs_minecraft_client::test_support::give;
use mcrs_minecraft_client::test_support::item_kind;
use mcrs_minecraft_client::test_support::joined;
use mcrs_minecraft_client::test_support::open;
use mcrs_minecraft_client::test_support::player_slot;
use mcrs_minecraft_client::test_support::round_trip;
use mcrs_minecraft_client::test_support::update_until;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_bot_click::ClickScript;
use mcrs_minecraft_inventory_bot_click::ClickScriptPlugin;
use mcrs_minecraft_inventory_bot_click::HeadlessClientPlugin;
use mcrs_minecraft_inventory_client::RemoteMenu;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_save::SaveLock;
use mcrs_minecraft_server_settings::ServerSettings;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const FIRST_HOTBAR_ENTRY: u16 = 36;
const ENTRIES: std::ops::RangeInclusive<u16> = 9..=45;
const STONE: &str = "minecraft:stone";

#[test]
fn the_bot_script_ends_with_the_expected_slots_and_no_correction() {
    let folder = std::env::temp_dir().join(format!("mcrs-bot-click-{}", std::process::id()));
    std::fs::remove_dir_all(&folder).ok();
    let mut app = bot();
    open(&mut app, &folder);
    update_until(&mut app, joined);
    update_until(&mut app, |app| {
        creative_player(app.world_mut()).is_some() && has_state_id(app.world_mut())
    });
    round_trip(&mut app);
    let viewer = creative_player(app.world_mut()).unwrap();
    let stone = ItemStack::new(item_kind(app.world(), STONE), Count::new(10).unwrap());
    give(app.world_mut(), viewer, FIRST_HOTBAR_ENTRY, Some(stone));
    round_trip(&mut app);

    *app.world_mut().resource_mut::<ServerAnswers>() = ServerAnswers::default();
    app.world_mut().init_resource::<ClickScript>();
    update_until(&mut app, |app| {
        app.world().resource::<ClickScript>().is_done()
    });
    round_trip(&mut app);

    assert_eq!(
        *app.world().resource::<ServerAnswers>(),
        ServerAnswers::default(),
        "the server sends no slot after a click that the client predicted"
    );
    let entries: Vec<(u16, u8)> = ENTRIES
        .filter_map(|entry| Some((entry, stone_at(app.world(), viewer, player_slot(entry))?)))
        .collect();
    assert_eq!(
        entries,
        [(9, 5), (FIRST_HOTBAR_ENTRY, 5)],
        "half of the stack stays in entry 9, the other half goes back to the hotbar"
    );
    let cursor = app.world().get::<PlayerHolders>(viewer).unwrap().cursor;
    assert_eq!(stone_at(app.world(), cursor, 0), None, "the cursor");

    app.world_mut().write_message(AppExit::Success);
    app.update();
    let lock = SaveLock::take(&folder).expect("the server released the world");
    drop(lock);
    std::fs::remove_dir_all(&folder).ok();
}

fn bot() -> App {
    let mut app = App::new();
    app.add_plugins((
        HeadlessClientPlugin {
            packs: PackRoots {
                assets: ASSETS.to_owned(),
                vanilla: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
            },
        },
        ClickScriptPlugin,
        LocalServerPlugin {
            asset_path: ASSETS.to_owned(),
            data_pack: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
            ops_file: None,
            username: "Bot".to_owned(),
            open_to_lan: false,
            settings: ServerSettings::default(),
            column_traces: None,
            extend_server: |_| {},
        },
    ));
    count_server_answers(&mut app);
    app.finish();
    app.cleanup();
    app
}

fn has_state_id(world: &mut World) -> bool {
    let menus: Vec<Entity> = world
        .query_filtered::<&Menus, With<PlayerHolders>>()
        .iter(world)
        .filter_map(|menus| menus.iter().next())
        .collect();
    menus
        .into_iter()
        .any(|menu| world.get::<RemoteMenu>(menu).is_some())
}

fn stone_at(world: &World, holder: Entity, index: u16) -> Option<u8> {
    let stack = world.entity(world.get::<Slots>(holder)?.get(usize::from(index))?);
    assert_eq!(
        *stack.get::<Kind>()?,
        item_kind(world, STONE),
        "the test gives only stone"
    );
    Some(stack.get::<Count>()?.get())
}
