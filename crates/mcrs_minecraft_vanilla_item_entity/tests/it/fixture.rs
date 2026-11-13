use std::sync::LazyLock;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::ScheduleBuildSettings;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_physics::EntityPass;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::Pass;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_vanilla_item_entity::Collect;
use mcrs_minecraft_vanilla_item_entity::ItemCollector;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;
use mcrs_minecraft_vanilla_item_entity::ItemEntityPlugin;
use mcrs_minecraft_vanilla_item_entity::Launch;
use mcrs_minecraft_vanilla_item_entity::PickedUp;
use mcrs_voxel::BlockPos;

pub const STONE: Kind = Kind(1);
pub const DIRT: Kind = Kind(2);
/// The top of the stone floor.
pub const FLOOR: f64 = 0.0;
pub const PLAYER: u128 = 7;
/// The network id of the player, below the ids of the cell.
pub const PLAYER_ID: EntityId = EntityId(1);

/// The items `stone` and `dirt` after `air`, whose wire id 0 is the empty stack, named so that
/// their stacks have a wire form.
static TABLE: LazyLock<ItemTable> = LazyLock::new(|| {
    let files = ["air", "stone", "dirt"].iter().zip(0..).map(|(name, id)| {
        format!(
            r#"{{"minecraft:item":{{"description":{{"identifier":"minecraft:{name}","protocol_id":{id}}},"components":{{"minecraft:max_stack_size":64}}}}}}"#
        )
    });
    let files: Vec<String> = files.collect();
    bake(
        files.iter().map(|bytes| DefinitionFile {
            name: "test",
            bytes: bytes.as_bytes(),
        }),
        &ItemTable::leak([]),
    )
    .expect("the test items bake")
    .table
});

#[derive(Resource)]
struct Scene {
    world: TestWorld,
    tables: Tables,
    grid: BodyGrid,
}

/// A dimension from `y = -64` with a stone floor under `y = 0`, its entity pass, stacks and item
/// entities.
pub fn app() -> App {
    let blocks = TestBlocks::new();
    let mut world = TestWorld::new();
    world.fill(
        BlockPos::new(-16, -4, -16),
        BlockPos::new(31, -1, 31),
        blocks.stone,
    );
    world.build_masks(&blocks.table);
    let mut ids = CellEntityIds::default();
    ids.add(EntityIdRange::starting_at(2).expect("a small range"));
    let mut app = App::new();
    app.configure_sets(
        FixedUpdate,
        (
            TickSet::DomainInput,
            TickSet::ClientInput,
            TickSet::Simulate,
            TickSet::WorldWrite,
            TickSet::Project,
            TickSet::Output,
        )
            .chain(),
    )
    .configure_sets(FixedUpdate, EntityPass.in_set(TickSet::Simulate))
    .configure_sets(FixedUpdate, MobPublish.in_set(TickSet::Project))
    .insert_resource(Scene {
        world,
        tables: Tables::new(blocks.table),
        grid: BodyGrid::default(),
    })
    .insert_resource(ids)
    .init_resource::<BirthClock>()
    .init_resource::<RowStore<()>>()
    .add_plugins((ItemPlugin::new(*TABLE), ItemEntityPlugin::<()>::default()))
    .add_systems(FixedUpdate, advance_clock.in_set(TickSet::DomainInput))
    .add_systems(FixedUpdate, pass.in_set(EntityPass))
    .add_systems(
        FixedUpdate,
        move_rows.in_set(TickSet::Project).before(MobPublish),
    );
    app.world_mut()
        .spawn((Dimension, DimensionTypeConfig::new(-64, 384)));
    app.edit_schedule(FixedUpdate, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..ScheduleBuildSettings::default()
        });
    });
    app
}

fn advance_clock(mut clock: ResMut<BirthClock>) {
    clock.0 += 1;
}

fn pass(mut scene: ResMut<Scene>, mut rows: ResMut<RowStore<()>>, clock: Res<BirthClock>) {
    let pool: &TaskPool = ComputeTaskPool::get_or_init(TaskPool::default);
    let scene = &mut *scene;
    rows.freeze(pool, &mut scene.grid);
    let pass = Pass {
        world: &scene.world,
        tables: &scene.tables,
        tick: clock.0,
        push: PushRules::default(),
        tiles_per_task: 1,
        crowd_damage_type: None,
    };
    rows.tick(&pass, pool, &scene.grid);
}

fn move_rows(mut rows: ResMut<RowStore<()>>) {
    rows.move_rows();
}

pub fn run(app: &mut App, ticks: u32) {
    for _ in 0..ticks {
        app.world_mut().run_schedule(FixedUpdate);
    }
}

/// Spawns a stack of `count` items of `kind` and launches it.
pub fn launch(world: &mut World, kind: Kind, count: u8, launch: Launch) -> Entity {
    let count = Count::new(count).expect("a count above 0");
    let stack = world
        .commands()
        .spawn_stack(ItemStack::new(kind, count))
        .launch(launch)
        .id();
    world.flush();
    stack
}

/// A player of the UUID [`PLAYER`] standing at `feet` with 41 slots and the hotbar slot 0
/// selected.
pub fn collector(world: &mut World, feet: DVec3, mode: Collect) -> Entity {
    world
        .spawn((
            Slots::new(41),
            Selected(0),
            ItemCollector {
                id: PLAYER_ID,
                uuid: PLAYER,
                bounds: Aabb::of_body(feet, 0.6, 1.8),
                vehicle: None,
                mode,
            },
        ))
        .id()
}

pub fn move_collector(world: &mut World, collector: Entity, feet: DVec3) {
    world
        .get_mut::<ItemCollector>(collector)
        .expect("a collector")
        .bounds = Aabb::of_body(feet, 0.6, 1.8);
}

/// Each item entity from west to east: its stack, position, count and values.
pub fn items(world: &mut World) -> Vec<(Entity, DVec3, u8, ItemEntity)> {
    let mut items: Vec<(Entity, DVec3, u8, ItemEntity)> = world
        .query::<(Entity, &Dropped, &Count, &ItemEntity)>()
        .iter(world)
        .map(|(stack, dropped, count, item)| (stack, dropped.pos, count.get(), *item))
        .collect();
    items.sort_by(|left, right| left.1.x.total_cmp(&right.1.x));
    items
}

pub fn picked_up(world: &mut World) -> Vec<PickedUp> {
    world.resource_mut::<Messages<PickedUp>>().drain().collect()
}

/// The count of the stack in the slot `index` of `holder`.
pub fn slot_count(world: &World, holder: Entity, index: usize) -> Option<u8> {
    let stack = world.get::<Slots>(holder)?.get(index)?;
    world.get::<Count>(stack).map(|count| count.get())
}

/// Fills the slot `index` of `holder` with `count` items of `kind`.
pub fn fill_slot(world: &mut World, holder: Entity, index: u16, kind: Kind, count: u8) {
    let count = Count::new(count).expect("a count above 0");
    world
        .commands()
        .spawn_stack(ItemStack::new(kind, count))
        .put_at(holder, index);
    world.flush();
}
