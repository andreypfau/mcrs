use std::sync::Arc;

use crate::fixture::{AIR, TORCH, filled, registry, state};
use bevy_app::{App, Last, TaskPoolPlugin};
use bevy_ecs::schedule::ScheduleLabel;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_light_engine::prelude::*;
use mcrs_minecraft_vanilla_light::{LightActor, LightPlugin, LightSet};
use mcrs_voxel::SectionPos;

fn settle(app: &mut App) {
    for _ in 0..1000 {
        app.update();
        if app.world().resource::<LightActor>().is_idle() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
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
            publish: Last.intern(),
        });

    let sections: Vec<SectionPos> = (0..2).map(|y| SectionPos::new(0, y, 0)).collect();
    for pos in &sections {
        let entity = app.world_mut().spawn(*pos).id();
        app.world_mut()
            .resource_mut::<LightActor>()
            .push(Edit::LoadSection {
                entity: entity.to_bits(),
                pos: *pos,
                blocks: Arc::new(filled(AIR)),
            });
    }
    settle(&mut app);

    app.world_mut()
        .resource_mut::<LightActor>()
        .push(Edit::SetBlock {
            pos: mcrs_voxel::BlockPos::new(8, 8, 8),
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
        mut actor: ResMut<LightActor>,
        added: Query<(Entity, &SectionPos), Added<SectionPos>>,
    ) {
        for (entity, pos) in &added {
            actor.push(Edit::LoadSection {
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
