use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::StatesPlugin;
use mcrs_voxel_world::entity::ChunkEntities;
use mcrs_voxel_world::voxel_update::SectionVoxels;
use mcrs_voxel_world::voxel_update::VoxelUpdateFlags;
use mcrs_voxel_world::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_voxel_world::world::lifecycle::markers::ChunkLoaded;
use mcrs_voxel_world::world::storage::chunk::Chunk;
use mcrs_voxel_world::world::storage::column::ColumnPlugin;
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_math::voxel_shape::VoxelShape;
use mcrs_voxel_storage::VoxelId;

use crate::LightingPlugin;
use crate::components::{BlockBfsPending, SkyBfsPending};
use crate::table::{BlockStateLightTable, flag_bits};

pub const TEST_DIM_HEIGHT: u32 = 384;
pub const TEST_DIM_MIN_Y: i32 = -64;

pub fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    let mut emission = vec![0u8; state_count].into_boxed_slice();
    let mut dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let mut flags = vec![0u8; state_count].into_boxed_slice();
    emission[0] = 0;
    dampening[0] = 0;
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    emission[1] = 0;
    dampening[1] = 15;
    flags[1] = flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

pub fn make_stub_block_light_table_with_torch() -> BlockStateLightTable {
    let state_count = 3usize;
    let mut emission = vec![0u8; state_count].into_boxed_slice();
    let mut dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let mut flags = vec![0u8; state_count].into_boxed_slice();
    emission[0] = 0;
    dampening[0] = 0;
    flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
    emission[1] = 0;
    dampening[1] = 15;
    flags[1] = flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
    emission[2] = 14;
    dampening[2] = 0;
    flags[2] = 0;
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

pub fn spawn_test_dimension(app: &mut App, sky: bool) -> Entity {
    let entity = app
        .world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new(if sky { "test:sky" } else { "test:skyless" }),
            DimensionTypeConfig::new(TEST_DIM_MIN_Y, TEST_DIM_HEIGHT),
        ))
        .id();
    if sky {
        app.world_mut().entity_mut(entity).insert(HasSkyLight);
    }
    entity
}

pub fn spawn_test_chunk(
    app: &mut App,
    dim: Entity,
    chunk_pos: ChunkPos,
    palette: SectionVoxels,
) -> Entity {
    app.world_mut()
        .spawn((
            InDimension(dim),
            chunk_pos,
            ChunkEntities::default(),
            Chunk,
            ChunkLoaded,
            palette,
        ))
        .id()
}

pub fn air_palette() -> SectionVoxels {
    let mut p = SectionVoxels::default();
    p.fill(VoxelId(0));
    p
}

pub fn solid_palette() -> SectionVoxels {
    let mut p = SectionVoxels::default();
    p.fill(VoxelId(1));
    p
}

pub fn torch_palette_with_one_emitter() -> SectionVoxels {
    let mut p = SectionVoxels::default();
    p.fill(VoxelId(0));
    p.set((8i32, 8i32, 8i32), VoxelId(2));
    p
}

pub fn tnt_3x3x3_palette() -> SectionVoxels {
    let mut p = SectionVoxels::default();
    p.fill(VoxelId(0));
    for x in 7i32..=9 {
        for y in 7i32..=9 {
            for z in 7i32..=9 {
                p.set((x, y, z), VoxelId(1));
            }
        }
    }
    p
}

pub fn stone_cap_then_air_palette() -> SectionVoxels {
    let mut p = SectionVoxels::default();
    p.fill(VoxelId(0));
    for x in 0i32..16 {
        for z in 0i32..16 {
            p.set((x, 15i32, z), VoxelId(1));
        }
    }
    p
}

pub fn solid_column_palette() -> SectionVoxels {
    let mut p = SectionVoxels::default();
    p.fill(VoxelId(1));
    p
}

pub fn install_lighting_plugins<F: VoxelUpdateFlags>(app: &mut App) -> Entity {
    app.add_plugins(StatesPlugin);
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin::<F>::default());
    app.insert_resource(make_stub_block_light_table());
    spawn_test_dimension(app, true)
}

pub fn build_single_torch_app<F: VoxelUpdateFlags>() -> App {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin::<F>::default());
    app.insert_resource(make_stub_block_light_table_with_torch());
    let dim = spawn_test_dimension(&mut app, true);
    spawn_test_chunk(
        &mut app,
        dim,
        ChunkPos::new(0, 0, 0),
        torch_palette_with_one_emitter(),
    );
    app
}

/// Like `build_single_torch_app` but uses a single-section dimension
/// (`min_y=0, height=16`) so the heightmap scan can finalize in the first
/// tick. `build_single_torch_app` uses a 24-section dimension height; with
/// only one chunk loaded the scan hits an absent top section
/// and returns `AbsentSection` every tick, preventing `BlockBfsPending` from
/// ever being inserted and leaving `run_until_converged` with nothing to
/// converge. This variant sizes the dimension to exactly one section so the
/// single loaded chunk is both the top and bottom of the column.
pub fn build_single_torch_app_single_section<F: VoxelUpdateFlags>() -> App {
    use mcrs_voxel_world::world::dimension::{
        DimensionBundle, DimensionId, DimensionTypeConfig, HasSkyLight,
    };

    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin::<F>::default());
    app.insert_resource(make_stub_block_light_table_with_torch());
    let dim = app
        .world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new("test:sky_single"),
            DimensionTypeConfig::new(0, 16),
        ))
        .id();
    app.world_mut().entity_mut(dim).insert(HasSkyLight);
    spawn_test_chunk(
        &mut app,
        dim,
        ChunkPos::new(0, 0, 0),
        torch_palette_with_one_emitter(),
    );
    app
}

pub fn build_tnt_chain_app<F: VoxelUpdateFlags>() -> App {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin::<F>::default());
    app.insert_resource(make_stub_block_light_table());
    let dim = spawn_test_dimension(&mut app, true);
    spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0), tnt_3x3x3_palette());
    app
}

pub fn build_roof_removal_app<F: VoxelUpdateFlags>() -> App {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin::<F>::default());
    app.insert_resource(make_stub_block_light_table());
    let dim = spawn_test_dimension(&mut app, true);
    spawn_test_chunk(
        &mut app,
        dim,
        ChunkPos::new(0, 0, 0),
        stone_cap_then_air_palette(),
    );
    app
}

pub fn build_pit_dig_app<F: VoxelUpdateFlags>() -> App {
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.add_plugins(ColumnPlugin);
    app.add_plugins(LightingPlugin::<F>::default());
    app.insert_resource(make_stub_block_light_table());
    let dim = spawn_test_dimension(&mut app, true);
    for chunk_y in 0..4i32 {
        spawn_test_chunk(
            &mut app,
            dim,
            ChunkPos::new(0, chunk_y, 0),
            solid_column_palette(),
        );
    }
    app
}

pub fn build_warmed_vd12_app_factory<F: VoxelUpdateFlags>() -> Box<dyn Fn() -> App + Send + Sync> {
    Box::new(|| {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.add_plugins(ColumnPlugin);
        app.add_plugins(LightingPlugin::<F>::default());
        app.insert_resource(make_stub_block_light_table());
        let dim = spawn_test_dimension(&mut app, true);
        for chunk_x in -12i32..=12 {
            for chunk_z in -12i32..=12 {
                for chunk_y in 0..24i32 {
                    let palette = if chunk_y % 2 == 0 {
                        stone_cap_then_air_palette()
                    } else {
                        air_palette()
                    };
                    spawn_test_chunk(
                        &mut app,
                        dim,
                        ChunkPos::new(chunk_x, chunk_y, chunk_z),
                        palette,
                    );
                }
            }
        }
        let ticks = run_until_converged(&mut app);
        if ticks > 10 {
            tracing::warn!(ticks, "vd12 warmup took more ticks than expected");
        }
        app
    })
}

pub fn build_warmed_vd12_app_in_place(app: &mut App) {
    let dim = spawn_test_dimension(app, true);
    for chunk_x in -12i32..=12 {
        for chunk_z in -12i32..=12 {
            for chunk_y in 0..24i32 {
                let palette = if chunk_y % 2 == 0 {
                    stone_cap_then_air_palette()
                } else {
                    air_palette()
                };
                spawn_test_chunk(app, dim, ChunkPos::new(chunk_x, chunk_y, chunk_z), palette);
            }
        }
    }
    let ticks = run_until_converged(app);
    if ticks > 10 {
        tracing::warn!(ticks, "vd12 warmup took more ticks than expected");
    }
}

pub fn spawn_edge_column(app: &mut App) -> Entity {
    let dim = app
        .world_mut()
        .query::<Entity>()
        .iter(app.world())
        .find(|&e| app.world().get::<HasSkyLight>(e).is_some())
        .expect("no sky-having dimension entity found");
    let first = spawn_test_chunk(
        app,
        dim,
        ChunkPos::new(13, 0, 0),
        stone_cap_then_air_palette(),
    );
    for chunk_y in 1..24i32 {
        spawn_test_chunk(app, dim, ChunkPos::new(13, chunk_y, 0), air_palette());
    }
    first
}

pub fn run_until_converged(app: &mut App) -> usize {
    let mut ticks = 0;
    loop {
        app.world_mut().run_schedule(FixedUpdate);
        ticks += 1;
        if !has_any_light_dirty(app) {
            return ticks;
        }
        if ticks >= 256 {
            panic!("bench failed to converge in 256 ticks");
        }
    }
}

pub fn has_any_light_dirty(app: &mut App) -> bool {
    let mut q = app
        .world_mut()
        .query_filtered::<(), Or<(With<BlockBfsPending>, With<SkyBfsPending>)>>();
    q.iter(app.world()).next().is_some()
}
