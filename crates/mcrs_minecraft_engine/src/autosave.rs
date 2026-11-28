use bevy_app::{App, FixedFirst, Plugin};
use bevy_ecs::prelude::{Res, ResMut, Resource};

pub const AUTOSAVE_INTERVAL: u32 = 6000;

#[derive(Resource, Default, Debug)]
pub struct AutosaveCadence {
    ticks: u32,
    due: bool,
}

impl AutosaveCadence {
    #[must_use]
    pub fn is_due(&self) -> bool {
        self.due
    }

    fn advance(&mut self) {
        self.ticks = (self.ticks + 1) % AUTOSAVE_INTERVAL;
        self.due = self.ticks == 0;
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "counts every fixed tick toward the next autosave"
    )
)]
pub(crate) fn advance_autosave_cadence(mut cadence: ResMut<AutosaveCadence>) {
    cadence.advance();
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
pub(crate) fn autosave_due(cadence: Res<AutosaveCadence>) -> bool {
    cadence.is_due()
}

pub struct AutosavePlugin;

impl Plugin for AutosavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutosaveCadence>();
        app.add_systems(FixedFirst, advance_autosave_cadence);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_ticks_come_every_interval_and_never_on_the_first() {
        let mut cadence = AutosaveCadence::default();
        let mut due = Vec::new();
        for tick in 1..=2 * AUTOSAVE_INTERVAL {
            cadence.advance();
            if cadence.is_due() {
                due.push(tick);
            }
        }
        assert_eq!(due, [AUTOSAVE_INTERVAL, 2 * AUTOSAVE_INTERVAL]);
    }
}
