use bevy_app::App;
use bevy_ecs::resource::Resource;

/// What plugins run on the host between two ticks, in the order they were added: after the
/// dimensions' output is taken and before the players that changed dimension move.
#[derive(Resource, Default)]
pub struct BetweenTicks(Vec<fn(&mut App)>);

impl BetweenTicks {
    pub fn add(&mut self, step: fn(&mut App)) {
        self.0.push(step);
    }
}

/// Runs every step of [`BetweenTicks`].
pub fn run_between_ticks(app: &mut App) {
    let Some(steps) = app.world_mut().remove_resource::<BetweenTicks>() else {
        return;
    };
    for step in &steps.0 {
        step(app);
    }
    app.world_mut().insert_resource(steps);
}
