use std::collections::BTreeMap;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_tasks::ComputeTaskPool;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_engine::block_drops::ItemDropped;
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClock;
use mcrs_minecraft_engine::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_physics::Birth;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BirthSource;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::BodyGrid;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::block_world::BlockWorld;
use crate::entity_pass::Pass;
use crate::entity_rows::EntityRows;
use crate::entity_write::EntityWrites;
use crate::tile_at::tile_at;

/// The entity pass of a dimension world: the merge point, the life cycle and the freeze in
/// `DomainInput`, the pass in `Simulate`, the tile moves in `Project`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EntityPhysicsPlugin;

/// The tables of the kernel, from the block definitions and tags of the world.
#[derive(Resource, Debug)]
pub(crate) struct PhysicsTables(pub(crate) Tables);

/// The body grid of the world.
#[derive(Resource, Debug, Default)]
pub(crate) struct Bodies(pub(crate) BodyGrid);

/// The stack of each item row.
#[derive(Resource, Debug, Default)]
pub struct ItemStacks(pub BTreeMap<EntityId, ItemStack>);

impl Plugin for EntityPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EntityRows>();
        app.init_resource::<Bodies>();
        app.init_resource::<EntityWrites>();
        app.init_resource::<ItemStacks>();
        app.init_resource::<BirthClock>();
        app.add_message::<ItemDropped>();
        app.add_message::<BlockPlaced>();
        app.add_systems(
            FixedUpdate,
            (
                build_tables.run_if(
                    resource_exists::<Blocks>
                        .and_then(resource_exists_and_changed::<CurrentRegistries>),
                ),
                apply_writes,
                spawn_dropped_items,
                stir_changed_blocks,
                freeze,
            )
                .chain()
                .in_set(TickSet::DomainInput),
        );
        app.add_systems(
            FixedUpdate,
            entity_tick
                .run_if(resource_exists::<PhysicsTables>)
                .in_set(TickSet::Simulate),
        );
        app.add_systems(FixedUpdate, move_rows.in_set(TickSet::Project));
    }
}

fn build_tables(mut commands: Commands, blocks: Res<Blocks>, registries: Res<CurrentRegistries>) {
    let Some(tags) = registries.tags::<Block>() else {
        tracing::error!("the registries declare no block tags; entities keep their tables");
        return;
    };
    commands.insert_resource(PhysicsTables(Tables::new(collision_table(&blocks, &tags))));
}

fn apply_writes(mut rows: ResMut<EntityRows>, mut writes: ResMut<EntityWrites>) {
    rows.apply_writes(&mut writes);
}

/// Each item that a block left becomes a row at the centre of the block, with the block as the
/// source of its birth key.
fn spawn_dropped_items(
    mut dropped: MessageReader<ItemDropped>,
    clock: Res<BirthClock>,
    mut ids: ResMut<CellEntityIds>,
    mut rows: ResMut<EntityRows>,
    mut stacks: ResMut<ItemStacks>,
) {
    let mut sequences: BTreeMap<_, u32> = BTreeMap::new();
    for drop in dropped.read() {
        let Some(id) = ids.allocate() else {
            tracing::warn!(at = ?drop.at, "the cell has no entity id left; the item is lost");
            continue;
        };
        let source = BirthSource::Block(drop.at);
        let sequence = sequences.entry(source.packed()).or_default();
        let key = BirthKey::born(clock.0, &source, *sequence);
        *sequence = sequence.wrapping_add(1);
        let body = Body {
            position: drop.at.as_dvec3() + DVec3::splat(0.5),
            ..Body::default()
        };
        let mut state = EntityState::new(key, body, None);
        state.phase = Birth::from(key).phase;
        rows.spawn(id, state, MotionRow::item());
        stacks.0.insert(id, drop.stack);
    }
}

fn stir_changed_blocks(mut placed: MessageReader<BlockPlaced>, mut rows: ResMut<EntityRows>) {
    for block in placed.read() {
        rows.stir(tile_at(block.block_pos.as_dvec3()));
    }
}

fn freeze(mut rows: ResMut<EntityRows>, mut bodies: ResMut<Bodies>) {
    rows.freeze(ComputeTaskPool::get(), &mut bodies.0);
}

fn entity_tick(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    clock: Res<BirthClock>,
    mut rows: ResMut<EntityRows>,
    mut bodies: ResMut<Bodies>,
    mut stacks: ResMut<ItemStacks>,
) {
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let pool = ComputeTaskPool::get();
    let tiles = rows.tiles.len();
    let pass = Pass {
        world: &BlockWorld {
            blocks: &blocks,
            table: &tables.0.collision,
        },
        tables: &tables.0,
        tick: clock.0,
        tiles_per_task: tiles.div_ceil(pool.thread_num().saturating_mul(4).max(1)),
    };
    for id in rows.tick(&pass, pool, &mut bodies.0) {
        stacks.0.remove(&id);
    }
}

fn move_rows(mut rows: ResMut<EntityRows>) {
    rows.move_rows();
}
