use bevy_ecs::prelude::Entity;
use std::sync::Arc;

use bevy_app::{App, TaskPoolPlugin};
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_storage::{PalettedContainer, VoxelId, VoxelPalette};

fn filled(block: VoxelId) -> SectionBlocks {
    VoxelPalette(PalettedContainer::Homogeneous(block))
}

const AIR: VoxelId = VoxelId(0);
const STONE: VoxelId = VoxelId(1);
const TORCH: VoxelId = VoxelId(2);

fn registry() -> Arc<LightRegistry> {
    Arc::new(LightRegistry::new(
        vec![
            LightProperties::AIR,
            LightProperties::SOLID,
            LightProperties::emitter(14),
        ],
        SpecialBlocks {
            unloaded: STONE,
            outside: AIR,
        },
    ))
}

// The epoch runs on a worker thread, so the loop has to give it a chance to
// finish rather than spinning the schedule.
fn settle(app: &mut App) {
    for _ in 0..200 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let world = app.world();
        if !world.resource::<LightEpoch>().is_running()
            && world.resource::<PendingEdits>().is_empty()
            && world.resource::<LightWorkQueue>().0.is_empty()
        {
            return;
        }
    }
    panic!("light never settled");
}

#[test]
fn a_torch_lights_its_neighbour_through_the_plugin() {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default())
        .add_plugins(LightPlugin {
            registry: registry(),
            bounds: LightBounds::new(0, 1),
            sky: true,
        });

    let sections: Vec<ChunkPos> = (0..2).map(|y| ChunkPos::new(0, y, 0)).collect();
    for pos in &sections {
        let entity = app.world_mut().spawn(*pos).id();
        app.world_mut()
            .resource_mut::<PendingEdits>()
            .push(Edit::LoadSection {
                entity,
                pos: *pos,
                blocks: Arc::new(filled(AIR)),
            });
    }
    settle(&mut app);

    app.world_mut()
        .resource_mut::<PendingEdits>()
        .push(Edit::SetBlock {
            pos: mcrs_voxel_math::BlockPos::new(8, 8, 8),
            block: TORCH,
        });
    settle(&mut app);

    let mut lit = app
        .world_mut()
        .query::<(&ChunkPos, &BlockLight, &SkyLight)>();
    let (_, block, sky) = lit
        .iter(app.world())
        .find(|(pos, _, _)| **pos == ChunkPos::new(0, 0, 0))
        .expect("the bottom section was published");

    assert_eq!(block.0.get(8, 8, 8), 14);
    assert_eq!(block.0.get(9, 8, 8), 13);
    assert_eq!(block.0.get(8, 8, 10), 12);
    assert_eq!(sky.0.get(0, 15, 0), 15);
}

/// A section can be despawned and respawned at the same position within one
/// tick. The later load names the new entity, so it supersedes the old one
/// without anything having to notice that the old one died.
#[test]
fn a_section_respawned_the_tick_its_predecessor_died_is_still_published() {
    use bevy_app::Last;
    use bevy_ecs::prelude::*;

    fn load_added_sections(
        mut pending: ResMut<PendingEdits>,
        added: Query<(Entity, &ChunkPos), Added<ChunkPos>>,
    ) {
        for (entity, pos) in &added {
            pending.push(Edit::LoadSection {
                entity,
                pos: *pos,
                blocks: Arc::new(filled(AIR)),
            });
        }
    }

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default())
        .add_plugins(LightPlugin {
            registry: registry(),
            bounds: LightBounds::new(0, 0),
            sky: true,
        })
        .add_systems(Last, load_added_sections.before(LightSet::Intake));

    let pos = ChunkPos::new(0, 0, 0);
    let old = app.world_mut().spawn(pos).id();
    settle(&mut app);
    assert!(app.world().get::<SkyLight>(old).is_some());

    app.world_mut().despawn(old);
    let new = app.world_mut().spawn(pos).id();
    settle(&mut app);

    assert!(
        app.world().get::<SkyLight>(new).is_some(),
        "the respawned section was published"
    );
}

fn budgeted_app() -> App {
    let mut app = App::new();
    app.add_plugins(LightPlugin {
        registry: registry(),
        bounds: LightBounds::new(0, 1),
        sky: true,
    });
    app
}

fn load_columns(app: &mut App, columns: i32) {
    let mut pending = app.world_mut().resource_mut::<PendingEdits>();
    for x in 0..columns {
        for y in 0..2 {
            pending.push(Edit::LoadSection {
                entity: Entity::PLACEHOLDER,
                pos: ChunkPos::new(x, y, 0),
                blocks: Arc::new(filled(AIR)),
            });
        }
    }
}

#[test]
fn intake_is_bounded_per_tick_and_loses_nothing() {
    let mut app = budgeted_app();
    let limit = app.world().resource::<IntakeBudget>().columns_per_tick;
    let columns = limit as i32 * 4 + 7;
    load_columns(&mut app, columns);

    app.update();
    let loaded = |app: &App| {
        app.world()
            .resource::<Lighting>()
            .0
            .loaded_sections()
            .count()
    };
    assert_eq!(loaded(&app), limit * 2);
    assert!(!app.world().resource::<PendingEdits>().is_empty());

    let mut ticks = 1;
    while !app.world().resource::<PendingEdits>().is_empty() {
        app.update();
        ticks += 1;
        assert!(ticks < 100, "intake never drained");
    }
    assert_eq!(ticks, (columns as usize).div_ceil(limit));
    assert_eq!(loaded(&app), columns as usize * 2);
}

#[test]
fn a_block_change_is_not_deferred_behind_a_bulk_load() {
    let mut app = budgeted_app();
    let limit = app.world().resource::<IntakeBudget>().columns_per_tick;
    let pos = mcrs_voxel_math::BlockPos::new(8, 8, 8);
    load_columns(&mut app, 1);
    app.update();

    load_columns(&mut app, limit as i32 * 3);
    app.world_mut()
        .resource_mut::<PendingEdits>()
        .push(Edit::SetBlock { pos, block: TORCH });
    app.update();

    assert!(!app.world().resource::<PendingEdits>().is_empty());
    assert_eq!(app.world().resource::<Lighting>().0.block_at(pos), TORCH);
}

#[test]
fn the_most_urgent_column_is_admitted_before_the_backlog() {
    let mut app = budgeted_app();
    let limit = app.world().resource::<IntakeBudget>().columns_per_tick;
    load_columns(&mut app, limit as i32 * 2);

    let urgent = ChunkPos::new(-1, 0, 0);
    app.world_mut()
        .resource_mut::<PendingEdits>()
        .push_with_priority(
            Edit::LoadSection {
                entity: Entity::PLACEHOLDER,
                pos: urgent,
                blocks: Arc::new(filled(AIR)),
            },
            0,
        );
    app.update();

    assert!(
        app.world()
            .resource::<Lighting>()
            .0
            .section(urgent)
            .is_some()
    );
    assert!(!app.world().resource::<PendingEdits>().is_empty());
}
