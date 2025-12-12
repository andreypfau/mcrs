use bevy_app::{App, Plugin};

pub struct ParallelWorldPlugin;

impl Plugin for ParallelWorldPlugin {
    fn build(&self, app: &mut App) {
        // app.add_systems(Startup, init_dimensions);
        // app.add_systems(FixedUpdate, fixed_update_dimensions);
    }
}

// #[derive(Component, Default)]
// struct TickingDimensionWorld(World);
//
// struct TickingDimension(Entity);
//
// impl RegistryEntry for TickingDimension {}
//
// #[derive(Default, Resource)]
// struct TickingDimensions {
//     registry: Registry<TickingDimension>,
// }
//
// fn fixed_pre_update_dimensions(mut query: Query<(&mut TickingDimensionWorld)>) {
//     query.par_iter_mut().for_each(|(mut world)| {
//         let _ = world.0.try_run_schedule(FixedPreUpdate);
//     })
// }
//
// fn fixed_update_dimensions(mut query: Query<(&mut TickingDimensionWorld)>) {
//     query.par_iter_mut().for_each(|(mut world)| {
//         let _ = world.0.try_run_schedule(FixedUpdate);
//     })
// }
//
// fn fixed_post_update_dimensions(mut query: Query<(&mut TickingDimensionWorld)>) {
//     query.par_iter_mut().for_each(|(mut world)| {
//         let _ = world.0.try_run_schedule(FixedPostUpdate);
//     })
// }
//
// fn init_dimensions(mut commands: Commands, dimensions: ResMut<TickingDimensions>) {
//     init_dimension(&mut commands, dimensions, ident!("overworld"));
// }
//
// fn init_dimension<T: Into<Ident<String>>>(
//     commands: &mut Commands,
//     mut dimensions: ResMut<TickingDimensions>,
//     ident: T,
// ) {
//     let mut world = World::default();
//
//     world.add_schedule(Schedule::new(FixedUpdate));
//     world.add_schedule(Schedule::new(FixedPreUpdate));
//     world.add_schedule(Schedule::new(FixedPostUpdate));
//
//     let entity = commands.spawn((TickingDimensionWorld(world),)).id();
//     dimensions
//         .registry
//         .insert(ident.into(), TickingDimension(entity));
// }
//
//
//
// #[derive(Bundle)]
// struct PlayerBundle {
//
// }
//
// fn spawn_player(
//     commands: &mut Commands
// ) {
//
// }
