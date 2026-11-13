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
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity::damage_type_keys;
use mcrs_minecraft_entity::registry_keys::DAMAGE_TYPE;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::Pass;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::tiles_per_task;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::EntityWrites;
use crate::block_world::BlockWorld;
use crate::mob_damage::report_crowd_hits;
use crate::mob_life::MobChanges;
use crate::mob_life::PendingSpawns;
use crate::mob_life::end_mob_changes;
use crate::mob_life::spawn_mobs;
use crate::mob_spawn_request::MobSpawnRequest;
use crate::mobs::MobUuids;
use crate::mobs::Mobs;
use crate::row_damage::RowDamaged;
use crate::row_damage::RowDied;
use crate::row_damage::RowHitApply;
use crate::row_damage::RowHits;
use crate::row_damage::apply_row_hits;

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

/// The entity pass in `Simulate`. After it, and before the tile moves of `Project`, each row is
/// still in the tile of its position at the start of the tick.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityPass;

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

/// The network id of the damage type of the crowd hit (DMG-CRW-001), from the registries of the
/// world; `None` while they hold no such type.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub(crate) struct CrowdDamageType(pub(crate) Option<u32>);

impl<M: Send + Sync + 'static> Plugin for EntityPhysicsPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_resource::<RowStore<M>>();
        app.init_resource::<Bodies>();
        app.init_resource::<EntityWrites>();
        app.init_resource::<BirthClock>();
        app.init_resource::<Mobs>();
        app.init_resource::<MobUuids>();
        app.init_resource::<MobChanges>();
        app.init_resource::<CrowdDamageType>();
        app.init_resource::<PendingSpawns>();
        app.init_resource::<RowHits>();
        app.add_message::<MobSpawnRequest>();
        app.add_message::<RowDamaged>();
        app.add_message::<RowDied>();
        app.add_message::<BlockPlaced>();
        app.add_systems(
            FixedUpdate,
            (
                build_tables.run_if(
                    resource_exists::<Blocks>
                        .and_then(resource_exists_and_changed::<CurrentRegistries>),
                ),
                find_crowd_damage_type.run_if(resource_exists_and_changed::<CurrentRegistries>),
                apply_writes::<M>,
                spawn_mobs::<M>,
                stir_changed_blocks::<M>,
                freeze::<M>,
            )
                .chain()
                .in_set(TickSet::DomainInput),
        );
        app.configure_sets(FixedUpdate, EntityPass.in_set(TickSet::Simulate));
        app.configure_sets(
            FixedUpdate,
            RowHitApply.in_set(TickSet::Simulate).after(EntityPass),
        );
        app.add_systems(FixedUpdate, apply_row_hits::<M>.in_set(RowHitApply));
        app.add_systems(
            FixedUpdate,
            entity_tick::<M>
                .run_if(resource_exists::<PhysicsTables>)
                .in_set(EntityPass),
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

fn find_crowd_damage_type(mut found: ResMut<CrowdDamageType>, registries: Res<CurrentRegistries>) {
    found.0 = registries
        .id_of(
            DAMAGE_TYPE.location(),
            damage_type_keys::CRAMMING.location(),
        )
        .map(u32::from);
    if found.0.is_none() {
        tracing::error!("the registries hold no damage type `cramming`; crowds take no damage");
    }
}

fn apply_writes<M: Send + Sync + 'static>(
    mut rows: ResMut<RowStore<M>>,
    mut writes: ResMut<EntityWrites>,
) {
    rows.apply_writes(&mut writes.0);
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
    mut changes: ResMut<MobChanges>,
    crowd: Res<CrowdDamageType>,
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
        crowd_damage_type: crowd.0,
        tiles_per_task: tiles_per_task(pool, rows.tile_count()),
    };
    rows.tick(&pass, pool, &bodies.0);
    if let Some(damage_type) = crowd.0 {
        report_crowd_hits(&rows, &mut changes, damage_type);
    }
}

fn move_rows<M: Send + Sync + 'static>(mut rows: ResMut<RowStore<M>>) {
    rows.move_rows();
}
