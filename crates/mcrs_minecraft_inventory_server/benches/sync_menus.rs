#![expect(clippy::expect_used, reason = "a bench reports a failure by panicking")]

use std::time::Duration;
use std::time::Instant;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::message::Messages;
use bevy_ecs::world::Mut;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::persisted_registry::PersistedRegistry;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_inventory_server::InventoryServerPlugin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_vanilla_inventory::InventoryPersistencePlugin;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryPlugin;

const WARM_UP: usize = 20;
const MEASURED: usize = 200;

fn table() -> ItemTable {
    let files = ["air", "stone", "dirt"].iter().zip(0..).map(|(name, id)| {
        format!(
            r#"{{"minecraft:item":{{"description":{{"identifier":"minecraft:{name}","protocol_id":{id}}},"components":{{"minecraft:max_stack_size":64}}}}}}"#
        )
    });
    let files: Vec<String> = files.collect();
    bake(
        files.iter().map(|bytes| DefinitionFile {
            name: "bench",
            bytes: bytes.as_bytes(),
        }),
        &ItemTable::leak([]),
    )
    .expect("the bench items bake")
    .table
}

fn app(table: ItemTable, players: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CurrentRegistries::new(test_registries().clone()))
        .insert_resource(table)
        .add_plugins((
            ItemPlugin::default(),
            VanillaInventoryPlugin,
            InventoryServerPlugin,
            InventoryPersistencePlugin,
        ));
    let world = app.world_mut();
    for session in 0..players {
        let player = world.spawn((Player, Owner(PlayerSession(session)))).id();
        world
            .resource_scope(|world, registry: Mut<PersistedRegistry>| {
                registry.apply(world, player, &PersistedSnapshot::new())
            })
            .expect("the empty snapshot applies");
        for index in 0..36_u8 {
            let count = Count::new(index + 1).expect("a count above 0");
            let kind = if index % 2 == 0 { Kind(1) } else { Kind(2) };
            world
                .commands()
                .spawn_stack(ItemStack::new(kind, count))
                .put_at(player, u16::from(index));
        }
        world.flush();
    }
    app
}

fn main() {
    let table = table();
    for players in [100, 1_000, 10_000] {
        let mut app = app(table, players);
        let world = app.world_mut();
        for _ in 0..WARM_UP {
            world.run_schedule(FixedUpdate);
        }
        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .clear();
        let mut ticks: Vec<Duration> = Vec::with_capacity(MEASURED);
        for _ in 0..MEASURED {
            let start = Instant::now();
            world.run_schedule(FixedUpdate);
            ticks.push(start.elapsed());
        }
        let sent = world.resource::<Messages<OutboundPlayerPacket>>().len();
        assert_eq!(sent, 0, "an unchanged inventory sends nothing");
        ticks.sort_unstable();
        let median = ticks.get(MEASURED / 2).copied().unwrap_or_default();
        let per_player = median.as_secs_f64() * 1e6 / players as f64;
        println!(
            "{players:>6} players: median tick {:>8.3} ms, {per_player:.3} us per player",
            median.as_secs_f64() * 1e3
        );
    }
}
