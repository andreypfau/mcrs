use std::sync::LazyLock;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::ScheduleBuildSettings;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_engine::block_simulation::BlockSimulationPlugin;
use mcrs_minecraft_engine::block_simulation::InertBlocks;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_item::ItemSet;
use mcrs_minecraft_engine_physics::EntityMotionPlugin;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::palette::SectionStates;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_falling_block::CONCRETE_PAIRS_PATH;
use mcrs_minecraft_falling_block::ConcretePairsFile;
use mcrs_minecraft_falling_block::PackConcretePairs;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLevelEvent;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_vanilla_falling_block::FallRequest;
use mcrs_minecraft_vanilla_falling_block::Falling;
use mcrs_minecraft_vanilla_falling_block::FallingBlockPlugin;
use mcrs_minecraft_vanilla_item_entity::ItemEntityPlugin;
use mcrs_minecraft_vanilla_item_entity::ItemEntitySet;
use mcrs_minecraft_volume::SectionBlocks;
use mcrs_minecraft_volume::SectionBlocksMut;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

static PAIRS: LazyLock<ConcretePairsFile> = LazyLock::new(|| {
    let path = format!(
        "{}/../../assets/mcrs/datapacks/mcrs/{CONCRETE_PAIRS_PATH}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
});

static TABLES: LazyLock<Tables> = LazyLock::new(|| {
    let tags = test_registries().tags::<Block>().unwrap();
    Tables::new(collision_table(&test_corpus().0, &tags))
});

pub fn blocks() -> &'static Blocks {
    &test_corpus().0
}

pub fn state(name: &str) -> BlockStateId {
    blocks()
        .default_state(&format!("minecraft:{name}"))
        .unwrap()
}

pub fn state_with(name: &str, properties: &[(&str, &str)]) -> BlockStateId {
    let entry = blocks().block(&format!("minecraft:{name}")).unwrap();
    properties
        .iter()
        .fold(entry.default_state_id, |id, (property, value)| {
            entry.with_text(id, property, value).unwrap()
        })
}

pub fn item_of(name: &str) -> Kind {
    let placed = state(name);
    let item = test_corpus()
        .1
        .iter()
        .find(|item| item.block_placer == Some(placed));
    Kind(item.unwrap().id.number())
}

pub fn app(settings: Settings) -> App {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let (blocks, items) = test_corpus();
    let mut ids = CellEntityIds::default();
    ids.add(EntityIdRange::starting_at(2).unwrap());
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
    .configure_sets(
        FixedUpdate,
        ItemSet.in_set(TickSet::Simulate).after(ItemEntitySet),
    )
    .insert_resource(ids)
    .insert_resource(blocks.clone())
    .insert_resource(items.clone())
    .insert_resource(CurrentRegistries::new(test_registries().clone()))
    .insert_resource(PhysicsTables::new(TABLES.clone()))
    .init_resource::<GameTime>()
    .init_resource::<BirthClock>()
    .insert_resource(test_item_table())
    .add_plugins((
        ItemPlugin::default(),
        EntityMotionPlugin,
        EntityMotionReplicationPlugin,
        ItemEntityPlugin,
        BlockSimulationPlugin::new(InertBlocks),
        FallingBlockPlugin {
            settings,
            concrete_pairs: vec![PackConcretePairs {
                pack: "mcrs".to_owned(),
                file: PAIRS.clone(),
            }],
        },
    ))
    .add_systems(FixedUpdate, advance_clock.in_set(TickSet::DomainInput));
    let stone = state("stone");
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

fn with_index<R>(world: &mut World, action: impl FnOnce(&mut SectionIndex) -> R) -> R {
    let mut query = world.query_filtered::<&mut SectionIndex, With<Dimension>>();
    action(&mut query.single_mut(world).unwrap())
}

pub fn set_block(world: &mut World, pos: BlockPos, state: BlockStateId) {
    with_index(world, |index| {
        index
            .section_mut(SectionPos::from(pos), None)
            .unwrap()
            .set(LocalPos::from(pos), state);
    });
}

pub fn block(world: &mut World, pos: BlockPos) -> BlockStateId {
    with_index(world, |index| {
        index
            .section_mut(SectionPos::from(pos), None)
            .unwrap()
            .get(LocalPos::from(pos))
    })
}

pub fn drop_block(world: &mut World, pos: BlockPos, state: BlockStateId) {
    set_block(world, pos, state);
    world.write_message(FallRequest { pos, state });
}

pub fn falling_blocks(world: &mut World) -> Vec<(Entity, Falling)> {
    world
        .query::<(Entity, &Falling)>()
        .iter(world)
        .map(|(entity, falling)| (entity, *falling))
        .collect()
}

pub fn set_velocity(world: &mut World, entity: Entity, velocity: DVec3) {
    world
        .get_mut::<MotionState>(entity)
        .unwrap()
        .0
        .body
        .velocity = velocity;
}

pub fn item_kinds(world: &mut World) -> Vec<Kind> {
    world
        .query_filtered::<&Kind, With<ItemEntity>>()
        .iter(world)
        .copied()
        .collect()
}

pub fn level_events(world: &mut World) -> Vec<ClientboundLevelEvent> {
    world
        .resource_mut::<Messages<ColumnChange>>()
        .drain()
        .filter_map(|change| match change.payload {
            PacketPayload::LevelEvent(event) => Some(event),
            _ => None,
        })
        .collect()
}
