//! The upkeep of the [`BirthClock`]: it rises at the start of every server tick, and the save
//! keeps it.

use bevy_app::{App, FixedFirst, Plugin, Startup};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::aoi::every_n_ticks;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_save::{SaveError, read_birth_clock, read_level_dat, write_birth_clock};
use tracing::error;

use crate::persistence::AUTOSAVE_INTERVAL;

pub struct BirthClockPlugin;

impl Plugin for BirthClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BirthClock>();
        app.add_systems(Startup, (load_birth_clock, save_birth_clock).chain());
        app.add_systems(
            FixedFirst,
            (
                advance_birth_clock,
                save_birth_clock.run_if(every_n_ticks(AUTOSAVE_INTERVAL)),
            )
                .chain(),
        );
    }
}

/// Continues from the saved clock; a save without one starts at its game time.
fn load_birth_clock(save: Option<Res<WorldSave>>, mut clock: ResMut<BirthClock>) {
    let Some(save) = save else {
        return;
    };
    let saved = read_birth_clock(&save.0).or_else(|err| {
        if !matches!(err, SaveError::Missing { .. }) {
            error!("birth clock unreadable, starting from the game time: {err}");
        }
        read_level_dat(&save.0).map(|level| u64::try_from(level.time).unwrap_or_default())
    });
    clock.0 = saved.unwrap_or_default();
}

fn advance_birth_clock(mut clock: ResMut<BirthClock>) {
    clock.0 = clock.0.saturating_add(1);
}

/// Saves the value that the clock reaches at the next save, not the current one: after a crash,
/// the clock then continues above every key that a saved entity can hold.
fn save_birth_clock(save: Option<Res<WorldSave>>, clock: Res<BirthClock>) {
    let Some(save) = save else {
        return;
    };
    let reserved = clock.0.saturating_add(u64::from(AUTOSAVE_INTERVAL));
    if let Err(err) = write_birth_clock(&save.0, reserved) {
        error!("birth clock not saved: {err}");
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// Starts a server on `save`, runs `ticks` ticks and stops it without a last save, as a
    /// crash does. Gives the clock at the start and at the end.
    fn run(save: &Path, ticks: u32) -> [u64; 2] {
        let mut app = App::new();
        app.add_plugins(BirthClockPlugin);
        app.insert_resource(WorldSave(save.to_path_buf()));
        app.world_mut().run_schedule(Startup);
        let start = app.world().resource::<BirthClock>().0;
        for _ in 0..ticks {
            app.world_mut().run_schedule(FixedFirst);
        }
        [start, app.world().resource::<BirthClock>().0]
    }

    #[test]
    fn cmp_013_birth_ticks_do_not_repeat_after_a_restart() {
        let save = std::env::temp_dir().join(format!("mcrs_birth_clock_{}", std::process::id()));
        let [first_start, first_end] = run(&save, AUTOSAVE_INTERVAL + 5);
        let [second_start, _] = run(&save, 1);
        let [third_start, _] = run(&save, 0);
        std::fs::remove_dir_all(&save).ok();

        assert_eq!(first_start, 0, "a new save starts at its game time");
        assert!(
            second_start > first_end,
            "the clock continues above every used tick"
        );
        assert!(
            third_start > second_start,
            "also after a run shorter than a save interval"
        );
    }
}
