use bevy::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuiTick(pub u64);

pub struct GuiTickPlugin;

impl Plugin for GuiTickPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuiTick>()
            .add_systems(FixedUpdate, advance_gui_tick);
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        mcrs::missed_set_if_neq,
        reason = "the counter advances on every fixed tick, so each run changes the value"
    )
)]
fn advance_gui_tick(mut tick: ResMut<GuiTick>) {
    tick.0 = tick.0.saturating_add(1);
}
