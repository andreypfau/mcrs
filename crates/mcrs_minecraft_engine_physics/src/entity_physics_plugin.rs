use std::collections::BTreeMap;
use std::marker::PhantomData;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::system::Single;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::block_drops::ItemDropped;
use mcrs_minecraft_engine_world::entity_births::EntityBirths;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::Pass;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::tiles_per_task;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_item::ItemStack;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_voxel::BlockPos;

use crate::EntityWrites;
use crate::block_world::BlockWorld;
use crate::mob_life::MobChanges;
use crate::mob_life::PendingSpawns;
use crate::mob_life::end_mob_changes;
use crate::mob_life::spawn_mobs;
use crate::mob_spawn_request::MobSpawnRequest;
use crate::mobs::MobUuids;
use crate::mobs::Mobs;

/// The entity pass of a dimension world: the merge point, the life cycle and the freeze in
/// `DomainInput`, the pass in `Simulate`, and in `Project` the tile moves, then the systems of
/// [`MobPublish`], then the end of the tick of the mobs. The rows are a [`RowStore<M>`] resource with the mob column `M`, which the pass
/// moves with its row and never reads.
#[derive(Debug)]
pub struct EntityPhysicsPlugin<M> {
    column: PhantomData<fn() -> M>,
}

impl<M> Clone for EntityPhysicsPlugin<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for EntityPhysicsPlugin<M> {}

impl<M> Default for EntityPhysicsPlugin<M> {
    fn default() -> Self {
        Self {
            column: PhantomData,
        }
    }
}

/// The systems that publish what the mobs did in the tick, such as their tile events: in
/// `Project`, after the tile moves of the rows, while [`Mobs`] still holds the mobs whose rows
/// left the world and [`MobChanges`] the changes of the tick.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MobPublish;

/// The tables of the kernel, from the block definitions and tags of the world.
#[derive(Resource, Debug)]
pub struct PhysicsTables(pub(crate) Tables);

impl PhysicsTables {
    /// The block shapes, flags and path types that the entity pass and the mob AI read.
    #[must_use]
    pub fn collision(&self) -> &CollisionTable {
        &self.0.collision
    }
}

/// The body grid of the world.
#[derive(Resource, Debug, Default)]
pub(crate) struct Bodies(pub(crate) BodyGrid);

/// The stack of each item row.
#[derive(Resource, Debug, Default)]
pub struct ItemStacks(pub BTreeMap<EntityId, ItemStack>);

impl<M: Send + Sync + 'static> Plugin for EntityPhysicsPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_resource::<RowStore<M>>();
        app.init_resource::<Bodies>();
        app.init_resource::<EntityWrites>();
        app.init_resource::<ItemStacks>();
        app.init_resource::<BirthClock>();
        app.init_resource::<Mobs>();
        app.init_resource::<MobUuids>();
        app.init_resource::<MobChanges>();
        app.init_resource::<PendingSpawns>();
        app.add_message::<MobSpawnRequest>();
        app.add_message::<ItemDropped>();
        app.add_message::<BlockPlaced>();
        app.add_systems(
            FixedUpdate,
            (
                build_tables.run_if(
                    resource_exists::<Blocks>
                        .and_then(resource_exists_and_changed::<CurrentRegistries>),
                ),
                apply_writes::<M>,
                spawn_dropped_items::<M>,
                spawn_mobs::<M>,
                stir_changed_blocks::<M>,
                freeze::<M>,
            )
                .chain()
                .in_set(TickSet::DomainInput),
        );
        app.add_systems(
            FixedUpdate,
            entity_tick::<M>
                .run_if(resource_exists::<PhysicsTables>)
                .in_set(TickSet::Simulate),
        );
        app.configure_sets(
            FixedUpdate,
            MobPublish
                .in_set(TickSet::Project)
                .after(move_rows::<M>)
                .before(end_mob_changes::<M>),
        );
        app.add_systems(
            FixedUpdate,
            (move_rows::<M>, end_mob_changes::<M>)
                .chain()
                .in_set(TickSet::Project),
        );
    }
}

fn build_tables(mut commands: Commands, blocks: Res<Blocks>, registries: Res<CurrentRegistries>) {
    let Some(tags) = registries.tags::<Block>() else {
        tracing::error!("the registries declare no block tags; entities keep their tables");
        return;
    };
    commands.insert_resource(PhysicsTables(Tables::new(collision_table(&blocks, &tags))));
}

fn apply_writes<M: Send + Sync + 'static>(
    mut rows: ResMut<RowStore<M>>,
    mut writes: ResMut<EntityWrites>,
) {
    rows.apply_writes(&mut writes.0);
}

/// Each item that a block left becomes a row at the centre of the block, with the block as the
/// source of its new UUID.
fn spawn_dropped_items<M: Send + Sync + 'static>(
    mut dropped: MessageReader<ItemDropped>,
    births: EntityBirths,
    mut ids: ResMut<CellEntityIds>,
    mut rows: ResMut<RowStore<M>>,
    mut stacks: ResMut<ItemStacks>,
) {
    let mut sequences: BTreeMap<[i32; 3], u32> = BTreeMap::new();
    for drop in dropped.read() {
        let Some(id) = ids.allocate() else {
            tracing::warn!(at = ?drop.at, "the cell has no entity id left; the item is lost");
            continue;
        };
        let source = BirthSource::Block(drop.at);
        let sequence = sequences
            .entry([drop.at.x, drop.at.y, drop.at.z])
            .or_default();
        let key = births.born(&source, *sequence);
        *sequence = sequence.wrapping_add(1);
        let row = MotionRow::item();
        let body = dropped_item(drop.at, &KeyedStream::new(&key, births.tick()), &row);
        let mut state = EntityState::new(key, body, None);
        state.phase = key.phase_offset();
        rows.spawn(id, state, row);
        stacks.0.insert(id, drop.stack);
    }
}

/// The start of an item that the block at `at` drops (PHY-CMP-025): a random offset from the
/// centre of the block and a random horizontal velocity, from the stream of its UUID.
fn dropped_item(at: BlockPos, stream: &KeyedStream, row: &MotionRow) -> Body {
    let uniform = |draw: u64, low: f64, high: f64| low + stream.unit(draw) * (high - low);
    let half_height = f64::from(row.dimensions.height) / 2.0;
    let centre = at.as_dvec3() + DVec3::splat(0.5);
    Body {
        position: DVec3::new(
            centre.x + uniform(0, -0.25, 0.25),
            centre.y + uniform(1, -0.25, 0.25) - half_height,
            centre.z + uniform(2, -0.25, 0.25),
        ),
        velocity: DVec3::new(uniform(3, -0.1, 0.1), 0.2, uniform(4, -0.1, 0.1)),
        ..Body::default()
    }
}

fn stir_changed_blocks<M: Send + Sync + 'static>(
    mut placed: MessageReader<BlockPlaced>,
    mut rows: ResMut<RowStore<M>>,
) {
    for block in placed.read() {
        rows.stir(TileKey::at(block.block_pos.as_dvec3()));
    }
}

fn freeze<M: Send + Sync + 'static>(mut rows: ResMut<RowStore<M>>, mut bodies: ResMut<Bodies>) {
    let pool: &TaskPool = ComputeTaskPool::get();
    rows.freeze(pool, &mut bodies.0);
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
fn entity_tick<M: Send + Sync + 'static>(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    clock: Res<BirthClock>,
    mut rows: ResMut<RowStore<M>>,
    bodies: Res<Bodies>,
    mut stacks: ResMut<ItemStacks>,
    rules: Option<Res<GameRules>>,
) {
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let pool: &TaskPool = ComputeTaskPool::get();
    let pass = Pass {
        world: &BlockWorld {
            blocks: &blocks,
            table: &tables.0.collision,
        },
        tables: &tables.0,
        tick: clock.0,
        push: PushRules {
            max_entity_cramming: rules.map_or(PushRules::default().max_entity_cramming, |rules| {
                rules.get(GameRule::MaxEntityCramming)
            }),
            ..PushRules::default()
        },
        tiles_per_task: tiles_per_task(pool, rows.tile_count()),
    };
    for id in rows.tick(&pass, pool, &bodies.0) {
        stacks.0.remove(&id);
    }
}

fn move_rows<M: Send + Sync + 'static>(mut rows: ResMut<RowStore<M>>) {
    rows.move_rows();
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_entity_key::BirthSource;
    use mcrs_minecraft_entity_key::EntityKey;

    use super::*;

    /// E54: 1000 drops of dirt from one block give offsets of `x` and `z` in `[0.25, 0.75)`, of
    /// `y` in `[0.125, 0.625)`, `v.y` of 0.2 and `v.x`, `v.z` in `[-0.1, 0.1)`, each spread over
    /// its range.
    #[test]
    fn cmp_025_a_dropped_item_starts_in_the_ranges_of_the_vanilla_drops() {
        let at = BlockPos::new(10, -50, 30);
        let row = MotionRow::item();
        let source = BirthSource::Block(at);
        let bodies: Vec<Body> = (0..1000)
            .map(|tick| {
                let key = EntityKey::born(0, "minecraft:overworld", tick, &source, 0);
                dropped_item(at, &KeyedStream::new(&key, tick), &row)
            })
            .collect();
        let offsets: Vec<DVec3> = bodies
            .iter()
            .map(|body| body.position - at.as_dvec3())
            .collect();
        let ranges: [(&str, Vec<f64>, f64, f64); 5] = [
            (
                "x",
                offsets.iter().map(|offset| offset.x).collect(),
                0.25,
                0.75,
            ),
            (
                "y",
                offsets.iter().map(|offset| offset.y).collect(),
                0.125,
                0.625,
            ),
            (
                "z",
                offsets.iter().map(|offset| offset.z).collect(),
                0.25,
                0.75,
            ),
            (
                "v.x",
                bodies.iter().map(|body| body.velocity.x).collect(),
                -0.1,
                0.1,
            ),
            (
                "v.z",
                bodies.iter().map(|body| body.velocity.z).collect(),
                -0.1,
                0.1,
            ),
        ];
        for (name, values, low, high) in ranges {
            let (min, max) = values
                .iter()
                .fold((f64::MAX, f64::MIN), |(min, max), &value| {
                    (min.min(value), max.max(value))
                });
            let margin = (high - low) * 0.01;
            assert!(
                low <= min && min < low + margin && max < high && high - margin < max,
                "{name} spans [{low}, {high}): {min} to {max}"
            );
        }
        assert!(
            bodies.iter().all(|body| body.velocity.y == 0.2),
            "v.y is 0.2"
        );
    }
}
