use std::sync::Arc;

use crate::fixture::{AIR, TORCH, filled, registry, state};
use bevy_app::{App, Last, TaskPoolPlugin};
use bevy_ecs::schedule::ScheduleLabel;
use bevy_tasks::block_on;
use mcrs_engine_link::Placement;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_light_engine::prelude::*;
use mcrs_minecraft_vanilla_light::{LightLink, LightPlugin, LightSet};
use mcrs_voxel::{BlockPos, SectionPos};

/// One update that hands the stage the pushed edits, a wait for its answer, and one update
/// that publishes the answer.
fn settle(app: &mut App) {
    app.update();
    block_on(app.world_mut().resource_mut::<LightLink>().answered());
    app.update();
    assert!(
        app.world().resource::<LightLink>().is_idle(),
        "the second update handed the stage nothing new"
    );
}

#[test]
fn a_torch_lights_its_neighbour_through_the_plugin() {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default())
        .add_plugins(LightPlugin {
            registry: registry(),
            bounds: LightBounds::new(0, 1),
            sky: true,
            publish: Last.intern(),
            placement: Placement::Thread,
        });

    let sections: Vec<SectionPos> = (0..2).map(|y| SectionPos::new(0, y, 0)).collect();
    for pos in &sections {
        let entity = app.world_mut().spawn(*pos).id();
        app.world_mut()
            .resource_mut::<LightLink>()
            .push(Edit::LoadSection {
                entity: entity.to_bits(),
                pos: *pos,
                blocks: Arc::new(filled(AIR)),
            });
    }
    settle(&mut app);

    app.world_mut()
        .resource_mut::<LightLink>()
        .push(Edit::SetBlock {
            pos: BlockPos::new(8, 8, 8),
            block: state(TORCH),
        });
    settle(&mut app);

    let mut lit = app
        .world_mut()
        .query::<(&SectionPos, &BlockLight, &SkyLight)>();
    let (_, block, sky) = lit
        .iter(app.world())
        .find(|(pos, _, _)| **pos == SectionPos::new(0, 0, 0))
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
    use bevy_ecs::prelude::*;

    fn load_added_sections(
        mut light: ResMut<LightLink>,
        added: Query<(Entity, &SectionPos), Added<SectionPos>>,
    ) {
        for (entity, pos) in &added {
            light.push(Edit::LoadSection {
                entity: entity.to_bits(),
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
            publish: Last.intern(),
            placement: Placement::Thread,
        })
        .add_systems(Last, load_added_sections.before(LightSet::Intake));

    let pos = SectionPos::new(0, 0, 0);
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

type Snapshot = Vec<(SectionPos, Option<BlockLight>, Option<SkyLight>)>;

/// The light of every section after each tick, where each tick waits for the stage to answer
/// what the tick handed it before the next one starts.
fn light_by_tick(placement: Placement) -> Vec<Snapshot> {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default())
        .add_plugins(LightPlugin {
            registry: registry(),
            bounds: LightBounds::new(0, 1),
            sky: true,
            publish: Last.intern(),
            placement,
        });
    for x in 0..2 {
        for y in 0..2 {
            let pos = SectionPos::new(x, y, 0);
            let entity = app.world_mut().spawn(pos).id();
            app.world_mut()
                .resource_mut::<LightLink>()
                .push(Edit::LoadSection {
                    entity: entity.to_bits(),
                    pos,
                    blocks: Arc::new(filled(AIR)),
                });
        }
    }

    let mut snapshots = Vec::new();
    for tick in 0..4 {
        if tick == 1 {
            for pos in [BlockPos::new(15, 15, 8), BlockPos::new(3, 20, 3)] {
                app.world_mut()
                    .resource_mut::<LightLink>()
                    .push(Edit::SetBlock {
                        pos,
                        block: state(TORCH),
                    });
            }
        }
        app.update();
        block_on(app.world_mut().resource_mut::<LightLink>().answered());
        let mut sections = app
            .world_mut()
            .query::<(&SectionPos, Option<&BlockLight>, Option<&SkyLight>)>();
        let mut snapshot: Snapshot = sections
            .iter(app.world())
            .map(|(pos, block, sky)| (*pos, block.cloned(), sky.cloned()))
            .collect();
        snapshot.sort_by_key(|(pos, ..)| (pos.x, pos.y, pos.z));
        snapshots.push(snapshot);
    }
    snapshots
}

#[test]
fn inline_and_thread_placements_publish_the_same_light_at_the_same_ticks() {
    let inline = light_by_tick(Placement::Inline);
    assert!(
        inline
            .last()
            .is_some_and(|last| last.iter().all(|(_, block, _)| block.is_some())),
        "every section is lit by the end"
    );
    assert_eq!(inline, light_by_tick(Placement::Thread));
}
