//! The upkeep of the [`BirthClock`]: it rises at the start of every server tick, and the save
//! keeps it.

use bevy_app::{App, FixedFirst, Plugin, Startup};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_save::{SaveError, read_birth_clock, read_level_dat};
use tracing::error;

use crate::persistence::{
    AUTOSAVE_INTERVAL, AutosavePlugin, advance_autosave_cadence, autosave_due,
};
use crate::world_state::{WorldStateWrite, WorldStateWriter};

pub struct BirthClockPlugin;

/// Whether the birth clock may write itself back. An unreadable file that is not missing stays
/// as it is, so a later run can recover it instead of reading the defaults this run wrote.
#[derive(Resource, Debug)]
struct BirthClockWritable(bool);

impl Default for BirthClockWritable {
    fn default() -> Self {
        Self(true)
    }
}

impl Plugin for BirthClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BirthClock>();
        app.init_resource::<BirthClockWritable>();
        app.init_resource::<WorldStateWriter>();
        if !app.is_plugin_added::<AutosavePlugin>() {
            app.add_plugins(AutosavePlugin);
        }
        app.add_systems(Startup, (load_birth_clock, save_birth_clock).chain());
        app.add_systems(
            FixedFirst,
            (
                advance_birth_clock.after(advance_autosave_cadence),
                save_birth_clock.run_if(autosave_due),
            )
                .chain(),
        );
    }
}

/// Continues from the saved clock; a save without one starts at its game time.
fn load_birth_clock(
    save: Option<Res<WorldSave>>,
    mut clock: ResMut<BirthClock>,
    mut writable: ResMut<BirthClockWritable>,
) {
    let Some(save) = save else {
        return;
    };
    let saved = match read_birth_clock(&save.0) {
        Ok(tick) => Some(tick),
        Err(SaveError::Missing { .. }) => read_level_dat(&save.0)
            .ok()
            .map(|level| u64::try_from(level.time).unwrap_or_default()),
        Err(err) => {
            error!("birth clock unreadable, starting from the game time: {err}");
            writable.0 = false;
            None
        }
    };
    clock.0 = saved.unwrap_or_default();
}

pub(crate) fn advance_birth_clock(mut clock: ResMut<BirthClock>) {
    clock.0 = clock.0.saturating_add(1);
}

/// Saves the value that the clock reaches at the next save, not the current one: after a crash,
/// the clock then continues above every key that a saved entity can hold.
fn save_birth_clock(
    save: Option<Res<WorldSave>>,
    clock: Res<BirthClock>,
    writable: Res<BirthClockWritable>,
    mut writer: ResMut<WorldStateWriter>,
) {
    let Some(save) = save else {
        return;
    };
    if !writable.0 {
        return;
    }
    let reserved = clock.0.saturating_add(u64::from(AUTOSAVE_INTERVAL));
    writer.submit(WorldStateWrite::birth_clock(save.0.clone(), reserved));
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
    fn an_unreadable_birth_clock_is_left_alone() {
        let save = std::env::temp_dir().join(format!(
            "mcrs_birth_clock_unreadable_{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&save).ok();
        let path = save.join("data/mcrs/birth_clock.dat");
        std::fs::create_dir_all(path.parent().expect("a data directory")).expect("a save");
        let broken = b"not a saved clock";
        std::fs::write(&path, broken).expect("a broken clock");

        let [start, end] = run(&save, AUTOSAVE_INTERVAL + 5);
        let kept = std::fs::read(&path).expect("the broken clock");
        std::fs::remove_dir_all(&save).ok();

        assert_eq!(start, 0);
        assert_eq!(end, u64::from(AUTOSAVE_INTERVAL) + 5);
        assert_eq!(kept, broken, "the unreadable file is not replaced");
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
