//! The game time, weather, game rules and world clocks that the save keeps across a restart.

use bevy_app::{App, FixedFirst, Plugin, Startup};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_engine_world::world_settings::{Difficulty, EntityDifficulty};
use mcrs_minecraft_environment::{AdvanceTime, WorldClock, WorldClocks};
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_save::{
    Difficulty as SaveDifficulty, LevelDat, RespawnData, SaveError, WeatherData, WorldClockStates,
    read_game_rules, read_level_dat, read_weather, read_world_clocks, write_birth_clock,
    write_game_rules, write_level_dat, write_weather, write_world_clocks,
};
use std::path::PathBuf;
use tracing::error;

use crate::background_writer::{BackgroundWriter, WriteJob};
use crate::coordinator::birth_clock::advance_birth_clock;
use crate::coordinator::clock::GameTime;
use crate::persistence::{AutosavePlugin, advance_autosave_cadence, autosave_due};
use crate::server_edit::Weather;

/// Loads the server state that the save keeps, and writes it back on the autosave cadence from
/// the I/O pool.
pub struct WorldStatePlugin;

impl Plugin for WorldStatePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AutosavePlugin>() {
            app.add_plugins(AutosavePlugin);
        }
        app.init_resource::<WorldStateWriter>();
        app.add_systems(Startup, load_world_state);
        app.add_systems(
            FixedFirst,
            (
                reap_world_state.run_if(writer_busy),
                autosave_world_state
                    .after(advance_autosave_cadence)
                    .before(advance_birth_clock)
                    .run_if(autosave_due),
            )
                .chain(),
        );
    }
}

fn world_difficulty(value: SaveDifficulty) -> EntityDifficulty {
    match value {
        SaveDifficulty::Peaceful => EntityDifficulty::Peaceful,
        SaveDifficulty::Easy => EntityDifficulty::Easy,
        SaveDifficulty::Normal => EntityDifficulty::Normal,
        SaveDifficulty::Hard => EntityDifficulty::Hard,
    }
}

fn saved_difficulty(value: Difficulty) -> SaveDifficulty {
    match value.0 {
        EntityDifficulty::Peaceful => SaveDifficulty::Peaceful,
        EntityDifficulty::Easy => SaveDifficulty::Easy,
        EntityDifficulty::Normal => SaveDifficulty::Normal,
        EntityDifficulty::Hard => SaveDifficulty::Hard,
    }
}

/// The spawn point that `level.dat` held when the server started.
#[derive(Resource, Debug, Clone)]
pub struct SavedSpawn(pub RespawnData);

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource it loads is its own parameter so the scheduler sees the access"
)]
fn load_world_state(
    mut commands: Commands,
    save: Option<Res<WorldSave>>,
    mut time: ResMut<GameTime>,
    mut weather: ResMut<Weather>,
    mut rules: ResMut<GameRules>,
    mut advance: ResMut<AdvanceTime>,
    mut clocks: ResMut<WorldClocks>,
    mut difficulty: Option<ResMut<Difficulty>>,
    registries: Option<Res<CurrentRegistries>>,
) {
    let Some(save) = save else {
        return;
    };
    if let Some(level) = Loaded::read(read_level_dat(&save.0), "level.dat").present() {
        time.0 = level.time;
        commands.insert_resource(SavedSpawn(level.spawn));
        if let Some(difficulty) = &mut difficulty {
            difficulty.0 = world_difficulty(level.difficulty_settings.difficulty);
        }
    }
    if let Some(saved) = Loaded::read(read_weather(&save.0), "weather").present() {
        weather.clear_ticks = saved.clear_weather_time.max(0).cast_unsigned();
        weather.rain_ticks = saved.rain_time.max(0).cast_unsigned();
        weather.thunder_ticks = saved.thunder_time.max(0).cast_unsigned();
        weather.raining = saved.raining;
        weather.thundering = saved.thundering;
    }
    if let Some(saved) = Loaded::read(read_game_rules(&save.0), "game rules").present() {
        for (rule, value) in saved.iter() {
            rules.set(rule, value);
        }
        advance.set_if_neq(AdvanceTime(rules.is_on(GameRule::AdvanceTime)));
    }
    if let Some(registries) = registries
        && let Some(registry) = registries.registry::<WorldClock>()
        && let Some(saved) = Loaded::read(read_world_clocks(&save.0), "world clocks").present()
    {
        for (id, name, _) in registry.iter() {
            if let Some(state) = saved.get(name) {
                clocks.insert(id, *state);
            }
        }
    }
}

/// A host file as the disk has it. A file that exists but does not parse is never replaced: the
/// load keeps the values of the run and the write leaves the file alone.
enum Loaded<T> {
    Missing,
    Present(T),
    Unreadable,
}

impl<T> Loaded<T> {
    fn read(result: Result<T, SaveError>, name: &str) -> Self {
        match result {
            Ok(value) => Self::Present(value),
            Err(SaveError::Missing { .. }) => Self::Missing,
            Err(err) => {
                error!("{name} unreadable and left alone: {err}");
                Self::Unreadable
            }
        }
    }

    fn is_unreadable(&self) -> bool {
        matches!(self, Self::Unreadable)
    }

    fn present(self) -> Option<T> {
        match self {
            Self::Present(value) => Some(value),
            Self::Missing | Self::Unreadable => None,
        }
    }

    /// The file, or `missing()` when there is none; `None` when the file is unreadable.
    fn or_missing(self, missing: impl FnOnce() -> T) -> Option<T> {
        match self {
            Self::Present(value) => Some(value),
            Self::Missing => Some(missing()),
            Self::Unreadable => None,
        }
    }
}

/// The host files that a restart reads, as the values of one tick. The write that follows
/// reads the files of the save again, so a field that the tick does not own stays as it is.
#[derive(Default)]
pub struct WorldStateWrite {
    dir: PathBuf,
    time: Option<i64>,
    weather: Option<WeatherData>,
    rules: Option<mcrs_minecraft_save::GameRules>,
    clocks: Option<WorldClockStates>,
    difficulty: Option<SaveDifficulty>,
    birth_clock: Option<u64>,
}

impl WorldStateWrite {
    pub(crate) fn birth_clock(dir: PathBuf, value: u64) -> Self {
        Self {
            dir,
            birth_clock: Some(value),
            ..Self::default()
        }
    }

    /// Takes the fields that `newer` carries over those of `self`.
    fn absorb(&mut self, newer: Self) {
        self.dir = newer.dir;
        self.time = newer.time.or(self.time);
        self.weather = newer.weather.or(self.weather.take());
        self.rules = newer.rules.or(self.rules.take());
        self.clocks = newer.clocks.or(self.clocks.take());
        self.difficulty = newer.difficulty.or(self.difficulty);
        self.birth_clock = newer.birth_clock.or(self.birth_clock);
    }

    /// Writes every file. A file that exists but does not read back is left alone instead of
    /// being replaced by defaults. Gives whether no write failed.
    fn write(self) -> bool {
        let dir = self.dir.as_path();
        let mut ok = true;
        let mut report = |name: &str, result: Result<(), SaveError>| {
            if let Err(err) = result {
                error!("{name} not saved: {err}");
                ok = false;
            }
        };
        if let Some(time) = self.time
            && let Some(mut level) =
                Loaded::read(read_level_dat(dir), "level.dat").or_missing(|| LevelDat {
                    level_name: "world".to_string(),
                    ..LevelDat::default()
                })
        {
            level.time = time;
            if let Some(difficulty) = self.difficulty {
                level.difficulty_settings.difficulty = difficulty;
            }
            report("level.dat", write_level_dat(dir, &level));
        }
        if let Some(weather) = &self.weather
            && !Loaded::read(read_weather(dir), "weather").is_unreadable()
        {
            report("weather", write_weather(dir, weather));
        }
        if let Some(rules) = &self.rules
            && !Loaded::read(read_game_rules(dir), "game rules").is_unreadable()
        {
            report("game rules", write_game_rules(dir, rules));
        }
        if let Some(clocks) = self.clocks
            && let Some(mut saved) =
                Loaded::read(read_world_clocks(dir), "world clocks").or_missing(Default::default)
        {
            saved.extend(clocks);
            report("world clocks", write_world_clocks(dir, &saved));
        }
        if let Some(value) = self.birth_clock {
            report("birth clock", write_birth_clock(dir, value));
        }
        ok
    }
}

/// The host files on their way to the disk: one write at a time on the I/O pool, so a file has at
/// most one write in flight, and the newest values that came while it ran wait for the next one.
#[derive(Resource, Default)]
pub struct WorldStateWriter {
    writer: BackgroundWriter<WorldStateWrite>,
    failed: bool,
}

impl WriteJob for WorldStateWrite {
    type Output = bool;

    fn merge(&mut self, newer: Self) {
        self.absorb(newer);
    }

    fn run(self) -> bool {
        self.write()
    }
}

impl WorldStateWriter {
    pub fn submit(&mut self, write: WorldStateWrite) {
        let failed = &mut self.failed;
        self.writer.submit(write, |ok| *failed = !ok);
    }

    /// Collects a finished write and starts the one that waits.
    pub fn reap(&mut self) {
        let failed = &mut self.failed;
        self.writer.reap(|ok| *failed = !ok);
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.writer.is_idle()
    }

    /// Whether the last finished write failed to write a file.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed
    }
}

/// What a write of the host files reads from the world.
#[derive(SystemParam)]
pub struct WorldStateSources<'w> {
    save: Option<Res<'w, WorldSave>>,
    time: Option<Res<'w, GameTime>>,
    weather: Option<Res<'w, Weather>>,
    rules: Option<Res<'w, GameRules>>,
    clocks: Option<Res<'w, WorldClocks>>,
    difficulty: Option<Res<'w, Difficulty>>,
    registries: Option<Res<'w, CurrentRegistries>>,
}

impl WorldStateSources<'_> {
    /// The values to write, or `None` when the world has no save.
    #[must_use]
    pub fn snapshot(&self) -> Option<WorldStateWrite> {
        let save = self.save.as_ref()?;
        let time = self.time.as_deref().copied().unwrap_or_default();
        let weather = self.weather.as_deref().map(|weather| WeatherData {
            clear_weather_time: weather.clear_ticks.cast_signed(),
            rain_time: weather.rain_ticks.cast_signed(),
            thunder_time: weather.thunder_ticks.cast_signed(),
            raining: weather.raining,
            thundering: weather.thundering,
        });
        let rules = self.rules.as_deref().map(|rules| {
            let mut saved = mcrs_minecraft_save::GameRules::default();
            for &rule in GameRule::ALL {
                saved.set(rule, rules.get(rule));
            }
            saved
        });
        let clocks = match (self.clocks.as_deref(), self.registries.as_deref()) {
            (Some(clocks), Some(registries)) => {
                registries.registry::<WorldClock>().map(|registry| {
                    registry
                        .iter()
                        .filter_map(|(id, name, _)| {
                            clocks.get(id).map(|state| (name.clone(), *state))
                        })
                        .collect()
                })
            }
            _ => None,
        };
        Some(WorldStateWrite {
            dir: save.0.clone(),
            time: Some(time.0),
            weather,
            rules,
            clocks,
            difficulty: self
                .difficulty
                .as_deref()
                .map(|difficulty| saved_difficulty(*difficulty)),
            birth_clock: None,
        })
    }
}

fn autosave_world_state(sources: WorldStateSources, mut writer: ResMut<WorldStateWriter>) {
    if let Some(write) = sources.snapshot() {
        writer.submit(write);
    }
}

fn writer_busy(writer: Res<WorldStateWriter>) -> bool {
    !writer.is_idle()
}

fn reap_world_state(mut writer: ResMut<WorldStateWriter>) {
    writer.reap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use mcrs_minecraft_save::{write_game_rules, write_level_dat, write_weather};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("mcrs-world-state-{}-{name}", std::process::id()))
    }

    fn world(dir: &std::path::Path) -> World {
        let mut world = World::new();
        world.insert_resource(WorldSave(dir.to_path_buf()));
        world.init_resource::<GameTime>();
        world.init_resource::<Weather>();
        world.init_resource::<GameRules>();
        world.init_resource::<AdvanceTime>();
        world.init_resource::<WorldClocks>();
        world.init_resource::<Difficulty>();
        world.init_resource::<WorldStateWriter>();
        world
    }

    #[test]
    fn the_host_state_loads_and_saves_every_game_rule_and_level_field() {
        let dir = temp_dir("round_trip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let level = LevelDat {
            level_name: "kept".to_owned(),
            time: 42,
            last_played: 9,
            ..LevelDat::default()
        };
        write_level_dat(&dir, &level).unwrap();
        write_weather(
            &dir,
            &WeatherData {
                clear_weather_time: 90,
                rain_time: 20,
                thunder_time: 10,
                raining: true,
                thundering: false,
            },
        )
        .unwrap();
        let mut saved_rules = mcrs_minecraft_save::GameRules::default();
        saved_rules.set(GameRule::AdvanceTime, 0);
        saved_rules.set(GameRule::RandomTickSpeed, 7);
        write_game_rules(&dir, &saved_rules).unwrap();

        let mut world = world(&dir);
        world.run_system_once(load_world_state).unwrap();
        assert_eq!(world.resource::<GameTime>().0, 42);
        assert_eq!(world.resource::<Weather>().clear_ticks, 90);
        assert!(world.resource::<Weather>().raining);
        assert!(!world.resource::<GameRules>().is_on(GameRule::AdvanceTime));
        assert!(!world.resource::<AdvanceTime>().0);
        assert_eq!(
            world.resource::<GameRules>().get(GameRule::RandomTickSpeed),
            7
        );

        world.resource_mut::<GameTime>().0 = 77;
        world.resource_mut::<Weather>().raining = false;
        world.run_system_once(autosave_world_state).unwrap();
        let read = mcrs_minecraft_save::read_level_dat(&dir).unwrap();
        assert_eq!(read.time, 77);
        assert_eq!(read.level_name, "kept");
        assert_eq!(read.last_played, 9);
        assert!(!mcrs_minecraft_save::read_weather(&dir).unwrap().raining);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_difficulty_loads_and_saves_without_touching_its_lock() {
        let dir = temp_dir("difficulty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let level = LevelDat {
            difficulty_settings: mcrs_minecraft_save::DifficultySettings {
                difficulty: SaveDifficulty::Hard,
                hardcore: true,
                locked: true,
            },
            ..LevelDat::default()
        };
        write_level_dat(&dir, &level).unwrap();

        let mut world = world(&dir);
        world.run_system_once(load_world_state).unwrap();
        assert_eq!(
            world.resource::<Difficulty>().0,
            EntityDifficulty::Hard,
            "the loaded world reads its difficulty from level.dat"
        );

        world.resource_mut::<Difficulty>().0 = EntityDifficulty::Peaceful;
        world.run_system_once(autosave_world_state).unwrap();
        let read = mcrs_minecraft_save::read_level_dat(&dir).unwrap();
        assert_eq!(
            read.difficulty_settings.difficulty,
            SaveDifficulty::Peaceful
        );
        assert!(read.difficulty_settings.hardcore);
        assert!(read.difficulty_settings.locked);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unreadable_host_state_files_are_left_alone() {
        let dir = temp_dir("unreadable");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data/minecraft")).unwrap();
        let files = [
            (dir.join("level.dat"), b"not a level".as_slice()),
            (
                dir.join("data/minecraft/weather.dat"),
                b"not weather".as_slice(),
            ),
            (
                dir.join("data/minecraft/game_rules.dat"),
                b"not rules".as_slice(),
            ),
            (
                dir.join("data/minecraft/world_clocks.dat"),
                b"not clocks".as_slice(),
            ),
        ];
        for (path, bytes) in &files {
            std::fs::write(path, bytes).unwrap();
        }

        world(&dir).run_system_once(autosave_world_state).unwrap();

        for (path, bytes) in &files {
            assert_eq!(std::fs::read(path).unwrap(), *bytes);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unreadable_game_rules_keep_the_defaults_and_stay_on_disk() {
        let dir = temp_dir("corrupt_game_rules");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data/minecraft")).unwrap();
        let path = dir.join("data/minecraft/game_rules.dat");
        std::fs::write(&path, b"not rules").unwrap();

        let mut world = world(&dir);
        let default_speed = GameRules::default().get(GameRule::RandomTickSpeed);
        world.run_system_once(load_world_state).unwrap();
        assert_eq!(
            world.resource::<GameRules>().get(GameRule::RandomTickSpeed),
            default_speed,
            "a corrupt game rules file must not change the rules of this run"
        );

        world.run_system_once(autosave_world_state).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"not rules");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unreadable_world_clocks_stay_on_disk() {
        let dir = temp_dir("corrupt_world_clocks");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data/minecraft")).unwrap();
        let path = dir.join("data/minecraft/world_clocks.dat");
        std::fs::write(&path, b"not clocks").unwrap();

        let registries = test_registries();
        assert!(
            registries.registry::<WorldClock>().is_some(),
            "the test registries declare world clocks, so the load reads the file"
        );
        let mut world = world(&dir);
        world.insert_resource(CurrentRegistries::new(registries.clone()));
        world.run_system_once(load_world_state).unwrap();

        world.run_system_once(autosave_world_state).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"not clocks");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
