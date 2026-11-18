use std::sync::LazyLock;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::ScheduleBuildSettings;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_physics::EntityMotionPlugin;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::palette::SectionStates;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_vanilla_item_entity::Collect;
use mcrs_minecraft_vanilla_item_entity::ItemCollector;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;
use mcrs_minecraft_vanilla_item_entity::ItemEntityPlugin;
use mcrs_minecraft_vanilla_item_entity::Launch;
use mcrs_minecraft_vanilla_item_entity::PickedUp;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

pub const STONE: Kind = Kind(1);
pub const DIRT: Kind = Kind(2);
pub const FLOOR: f64 = 0.0;
pub const PLAYER: u128 = 7;
pub const PLAYER_ID: EntityId = EntityId(1);

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

pub fn app() -> App {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let blocks = TestBlocks::new();
    let stone = blocks.stone;
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
    .configure_sets(FixedUpdate, MobPublish.in_set(TickSet::Project))
    .insert_resource(ids)
    .insert_resource(PhysicsTables::new(Tables::new(blocks.table)))
    .init_resource::<BirthClock>()
    .insert_resource(*TABLE)
    .add_plugins((
        ItemPlugin::default(),
        EntityMotionPlugin,
        EntityMotionReplicationPlugin,
        ItemEntityPlugin,
    ))
    .add_systems(FixedUpdate, advance_clock.in_set(TickSet::DomainInput));
    let world = app.world_mut();
    let mut index = SectionIndex::new();
    for x in -1..=1 {
        for z in -1..=1 {
            for y in -4..=3 {
                let state = if y < 0 { stone } else { BlockStateId::AIR };
                let pos = SectionPos::new(x, y, z);
                let section = world.spawn_empty().id();
                index.insert(pos, section);
                index.land(
                    pos,
                    section,
                    ChunkBlocks::new(SectionStates::homogeneous(state)),
                    None,
                    false,
                );
            }
        }
    }
    world.spawn((Dimension, DimensionTypeConfig::new(-64, 384), index));
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

pub fn run(app: &mut App, ticks: u32) {
    for _ in 0..ticks {
        app.world_mut().run_schedule(FixedUpdate);
    }
}

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

pub fn slot_count(world: &World, holder: Entity, index: usize) -> Option<u8> {
    let stack = world.get::<Slots>(holder)?.get(index)?;
    world.get::<Count>(stack).map(|count| count.get())
}

pub fn fill_slot(world: &mut World, holder: Entity, index: u16, kind: Kind, count: u8) {
    let count = Count::new(count).expect("a count above 0");
    world
        .commands()
        .spawn_stack(ItemStack::new(kind, count))
        .put_at(holder, index);
    world.flush();
}
