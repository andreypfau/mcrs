use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use mcrs_minecraft_render::WorldPass;

#[derive(Resource, Default)]
struct Ran(Vec<u8>);

fn record(step: u8) -> impl FnMut(ResMut<Ran>) {
    move |mut ran: ResMut<Ran>| ran.0.push(step)
}

#[test]
fn a_system_from_another_crate_runs_in_its_stage() {
    let mut schedule = Core3d::base_schedule();
    schedule.configure_sets(WorldPass::order());
    schedule.add_systems(record(0).in_set(Core3dSystems::Prepass));
    schedule.add_systems((
        record(6)
            .after(WorldPass::Forward)
            .in_set(Core3dSystems::MainPass),
        record(5).in_set(WorldPass::Forward),
        record(4).in_set(WorldPass::Lighting),
        record(3).in_set(WorldPass::OpaqueSecond),
        record(2).in_set(WorldPass::Opaque),
        record(1).in_set(WorldPass::Upload),
    ));
    schedule.add_systems(record(7).in_set(Core3dSystems::EarlyPostProcess));

    let mut world = World::new();
    world.init_resource::<Ran>();
    schedule.run(&mut world);

    assert_eq!(world.resource::<Ran>().0, [0, 1, 2, 3, 4, 5, 6, 7]);
}
