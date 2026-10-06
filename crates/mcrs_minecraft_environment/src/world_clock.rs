use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_app::{App, FixedUpdate, Plugin, Startup};
use bevy_ecs::prelude::*;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_registry::shared::SharedResource;
use mcrs_minecraft_registry::{Id, Registry, RegistrySet};
use serde::{Deserialize, Serialize};

use crate::timeline::Timeline;

/// Unit-shaped registry entry. Vanilla `WorldClock.DIRECT_CODEC` is
/// `MapCodec.unitCodec(WorldClock::new)`, so the wire payload is an
/// empty NBT compound.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldClock {}

impl RegistryValue for WorldClock {
    type Registry = Self;
}

/// One clock's authoritative state.
///
/// `total_ticks` is the only fact; the tick within a timeline period, the
/// time of day and the network form are all derived from it on read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClockState {
    pub total_ticks: i64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub partial_tick: f32,
    #[serde(default = "default_rate", skip_serializing_if = "is_default_rate")]
    pub rate: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub paused: bool,
}

fn default_rate() -> f32 {
    1.0
}

fn is_default_rate(rate: &f32) -> bool {
    *rate == default_rate()
}

impl Default for ClockState {
    fn default() -> Self {
        Self {
            total_ticks: 0,
            partial_tick: 0.0,
            rate: 1.0,
            paused: false,
        }
    }
}

impl ClockState {
    /// Run the clock forward by `elapsed` host ticks.
    ///
    /// The server passes 1.0 once per fixed tick; a client catching up to a
    /// freshly received packet passes the game-time delta it missed. `rate` is
    /// clock ticks per host tick, so a rate below 1.0 leaves a remainder in
    /// `partial_tick` and a rate above 1.0 can cross several ticks at once.
    pub fn advance(&mut self, elapsed: f64) {
        if self.paused {
            return;
        }
        let partial = f64::from(self.partial_tick) + elapsed * f64::from(self.rate);
        let full = partial.floor();
        self.partial_tick = (partial - full) as f32;
        self.total_ticks += full as i64;
    }

    /// A clock a client received with `rate = 0.0` is stopped just as surely as
    /// one whose owner paused it, and the client is never told which.
    pub fn is_paused(&self) -> bool {
        self.paused || self.rate == 0.0
    }
}

/// Every clock of the `world_clock` registry, keyed by id and walked in id order.
///
/// A resource rather than an entity per clock: this crosses into every
/// dimension sub-world once per tick, and entities do not cross worlds.
#[derive(Resource, Debug, Clone, Default)]
pub struct WorldClocks(BTreeMap<Id<WorldClock>, ClockState>);

impl WorldClocks {
    pub fn get(&self, clock: Id<WorldClock>) -> Option<&ClockState> {
        self.0.get(&clock)
    }

    pub fn get_mut(&mut self, clock: Id<WorldClock>) -> Option<&mut ClockState> {
        self.0.get_mut(&clock)
    }

    pub fn insert(&mut self, clock: Id<WorldClock>, state: ClockState) {
        self.0.insert(clock, state);
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<WorldClock>, &ClockState)> {
        self.0.iter().map(|(id, state)| (*id, state))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<WorldClock>, &mut ClockState)> {
        self.0.iter_mut().map(|(id, state)| (*id, state))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Leave exactly one clock per registry entry: keep the supplied state for
    /// an entry that has one, default the rest, drop what the registry no
    /// longer knows.
    pub fn reconcile_with_registry(&mut self, registry: &Registry<WorldClock>) {
        self.0.retain(|id, _| {
            let known = registry.name(*id).is_some();
            if !known {
                tracing::warn!(clock = ?id, "discarding clock state with no world_clock registry entry");
            }
            known
        });
        for id in registry.ids() {
            self.0.entry(id).or_default();
        }
    }
}

/// A time marker resolved against the timeline that declared it.
///
/// The period is the *timeline's*, not the marker's: the same id declared in a
/// timeline with no period is a one-off instant rather than a recurring one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockTimeMarker {
    pub ticks: u32,
    pub period_ticks: Option<u32>,
    pub show_in_commands: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("time marker `{marker}` was defined more than once for clock `{clock}`")]
pub struct DuplicateTimeMarker {
    pub marker: ResourceLocation<Arc<str>>,
    pub clock: ResourceLocation<Arc<str>>,
}

/// Every time marker the loaded timelines declare, grouped by the clock they
/// run on.
///
/// Markers have no registry of their own: this table is derived once from the
/// `timeline` column and never touched again.
#[derive(Resource, Debug, Clone, Default)]
pub struct ClockTimeMarkers(Arc<BTreeMap<Id<WorldClock>, MarkersOfClock>>);

type MarkersOfClock = BTreeMap<ResourceLocation<Arc<str>>, ClockTimeMarker>;

impl SharedResource for ClockTimeMarkers {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl ClockTimeMarkers {
    pub fn get(&self, clock: Id<WorldClock>, marker: &str) -> Option<&ClockTimeMarker> {
        self.0.get(&clock)?.get(marker)
    }

    pub fn of_clock(
        &self,
        clock: Id<WorldClock>,
    ) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &ClockTimeMarker)> {
        self.0.get(&clock).into_iter().flat_map(BTreeMap::iter)
    }

    pub fn clocks(&self) -> impl Iterator<Item = Id<WorldClock>> {
        self.0.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.0.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0.values().all(BTreeMap::is_empty)
    }

    /// A marker id may be reused on another clock but not on the same one; every
    /// repeat is returned with the index of the timeline that repeated it.
    pub fn derive(
        timelines: &[Timeline],
        clocks: &Registry<WorldClock>,
    ) -> Result<Self, Vec<(usize, DuplicateTimeMarker)>> {
        let mut table: BTreeMap<Id<WorldClock>, MarkersOfClock> = BTreeMap::new();
        let mut duplicates = Vec::new();
        for (index, timeline) in timelines.iter().enumerate() {
            let Some(clock) = clocks.name(timeline.clock) else {
                continue;
            };
            for (id, marker) in &timeline.time_markers {
                let resolved = ClockTimeMarker {
                    ticks: marker.ticks,
                    period_ticks: timeline.period_ticks,
                    show_in_commands: marker.show_in_commands,
                };
                let of_clock = table.entry(timeline.clock).or_default();
                if of_clock.contains_key(id) {
                    duplicates.push((
                        index,
                        DuplicateTimeMarker {
                            marker: id.clone(),
                            clock: clock.clone(),
                        },
                    ));
                } else {
                    of_clock.insert(id.clone(), resolved);
                }
            }
        }
        if duplicates.is_empty() {
            Ok(Self(Arc::new(table)))
        } else {
            Err(duplicates)
        }
    }
}

pub fn check_time_markers(timelines: &[Timeline], set: &RegistrySet) -> Vec<(usize, String)> {
    let Some(clocks) = set.registry::<WorldClock>() else {
        return Vec::new();
    };
    match ClockTimeMarkers::derive(timelines, &clocks) {
        Ok(_) => Vec::new(),
        Err(duplicates) => duplicates
            .into_iter()
            .map(|(index, duplicate)| (index, duplicate.to_string()))
            .collect(),
    }
}

/// The global `advance_time` gamerule. Truth until a save reader supplies it.
#[derive(Resource, Debug, Clone, Copy)]
pub struct AdvanceTime(pub bool);

impl Default for AdvanceTime {
    fn default() -> Self {
        Self(true)
    }
}

pub struct WorldClockPlugin;

impl Plugin for WorldClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClocks>()
            .init_resource::<AdvanceTime>()
            .add_systems(Startup, seed_world_clocks)
            .add_systems(
                FixedUpdate,
                advance_world_clocks.run_if(advance_time_enabled),
            );
    }
}

pub fn seed_world_clocks(mut clocks: ResMut<WorldClocks>, set: Res<RegistrySet>) {
    let Some(registry) = set.registry::<WorldClock>() else {
        tracing::error!("the registry set holds no world_clock registry to seed the clocks from");
        return;
    };
    clocks.reconcile_with_registry(&registry);
    tracing::info!(clocks = clocks.len(), "seeded world clocks");
}

fn advance_time_enabled(advance_time: Res<AdvanceTime>) -> bool {
    advance_time.0
}

fn advance_world_clocks(mut clocks: ResMut<WorldClocks>) {
    for state in clocks.0.values_mut() {
        state.advance(1.0);
    }
}

/// Push the main app's clocks into a dimension sub-world. The sub-world holds
/// a read-only copy: it is overwritten wholesale every tick, so any local
/// advancement it attempted would be silently discarded.
pub fn extract_world_clocks(main_world: &mut World, sub_world: &mut World) {
    if let Some(clocks) = main_world.get_resource::<WorldClocks>() {
        sub_world.insert_resource(clocks.clone());
    }
}

#[cfg(test)]
pub(crate) static TEST_CLOCKS: std::sync::LazyLock<RegistrySet> = std::sync::LazyLock::new(|| {
    mcrs_minecraft_worldgen_testing::shipped_registry_set::<WorldClock>("world_clock")
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Timeline;

    const OVERWORLD: &str = "minecraft:overworld";

    const THE_END: &str = "minecraft:the_end";

    fn id(name: &str) -> Id<WorldClock> {
        TEST_CLOCKS
            .registry::<WorldClock>()
            .unwrap()
            .require_by_name(name)
            .unwrap()
    }

    fn all_clocks() -> WorldClocks {
        let mut clocks = WorldClocks::default();
        clocks.reconcile_with_registry(&TEST_CLOCKS.registry().unwrap());
        clocks
    }

    fn tick_app(app: &mut App) {
        app.world_mut().run_schedule(FixedUpdate);
    }

    fn app_with(clocks: WorldClocks, advance_time: bool) -> App {
        let mut app = App::new();
        app.insert_resource(clocks);
        app.insert_resource(AdvanceTime(advance_time));
        app.add_systems(
            FixedUpdate,
            advance_world_clocks.run_if(advance_time_enabled),
        );
        app
    }

    fn total_ticks(app: &App, clock: Id<WorldClock>) -> i64 {
        app.world()
            .resource::<WorldClocks>()
            .get(clock)
            .unwrap()
            .total_ticks
    }

    #[test]
    fn reconcile_gives_one_clock_per_registry_entry() {
        let registry = TEST_CLOCKS.registry::<WorldClock>().unwrap();
        let removed = Registry::<WorldClock>::new(
            crate::keys::WORLD_CLOCK,
            ["minecraft:overworld", THE_END, "datapack:removed"]
                .map(|name| ResourceLocation::<Arc<str>>::read(name).unwrap()),
        )
        .unwrap()
        .require_by_name("datapack:removed")
        .unwrap();
        let mut clocks = WorldClocks::default();
        clocks.insert(
            id(OVERWORLD),
            ClockState {
                total_ticks: 500,
                ..ClockState::default()
            },
        );
        clocks.insert(removed, ClockState::default());

        clocks.reconcile_with_registry(&registry);

        assert_eq!(clocks.len(), registry.len());
        assert_eq!(clocks.get(id(OVERWORLD)).unwrap().total_ticks, 500);
        assert_eq!(clocks.get(id(THE_END)).unwrap().total_ticks, 0);
        assert!(clocks.get(removed).is_none());
    }

    #[test]
    fn clocks_are_read_and_advanced_by_id() {
        let registry = TEST_CLOCKS.registry::<WorldClock>().unwrap();
        let (overworld, end) = (id(OVERWORLD), id(THE_END));

        let mut app = App::new();
        app.insert_resource(TEST_CLOCKS.clone())
            .insert_resource(AdvanceTime(true))
            .init_resource::<WorldClocks>()
            .add_systems(Startup, seed_world_clocks)
            .add_systems(FixedUpdate, advance_world_clocks);
        app.world_mut().run_schedule(Startup);

        let seeded: Vec<_> = app
            .world()
            .resource::<WorldClocks>()
            .iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(seeded, registry.ids().collect::<Vec<_>>());

        app.world_mut()
            .resource_mut::<WorldClocks>()
            .get_mut(end)
            .unwrap()
            .paused = true;
        for _ in 0..10 {
            tick_app(&mut app);
        }
        assert_eq!(total_ticks(&app, overworld), 10);
        assert_eq!(total_ticks(&app, end), 0);
    }

    #[test]
    fn a_fractional_rate_carries_the_remainder() {
        let mut state = ClockState {
            rate: 0.25,
            ..ClockState::default()
        };
        for expected in [(0, 0.25), (0, 0.5), (0, 0.75), (1, 0.0)] {
            state.advance(1.0);
            assert_eq!((state.total_ticks, state.partial_tick), expected);
        }

        for _ in 0..3_996 {
            state.advance(1.0);
        }
        assert_eq!(state.total_ticks, 1_000);
    }

    #[test]
    fn a_rate_above_one_crosses_several_ticks_per_advance() {
        let mut state = ClockState {
            rate: 2.5,
            ..ClockState::default()
        };

        state.advance(1.0);
        assert_eq!((state.total_ticks, state.partial_tick), (2, 0.5));
        state.advance(1.0);
        assert_eq!((state.total_ticks, state.partial_tick), (5, 0.0));

        for _ in 0..398 {
            state.advance(1.0);
        }
        assert_eq!(state.total_ticks, 1_000);
    }

    #[test]
    fn a_client_catching_up_advances_by_the_missed_ticks() {
        let mut caught_up = ClockState {
            rate: 0.75,
            ..ClockState::default()
        };
        caught_up.advance(8.0);

        let mut stepped = ClockState {
            rate: 0.75,
            ..ClockState::default()
        };
        for _ in 0..8 {
            stepped.advance(1.0);
        }

        assert_eq!(caught_up, stepped);
        assert_eq!(caught_up.total_ticks, 6);
    }

    #[test]
    fn pausing_a_clock_stops_only_that_clock() {
        let mut app = app_with(all_clocks(), true);
        app.world_mut()
            .resource_mut::<WorldClocks>()
            .get_mut(id(OVERWORLD))
            .unwrap()
            .paused = true;

        for _ in 0..10 {
            tick_app(&mut app);
        }

        assert_eq!(total_ticks(&app, id(OVERWORLD)), 0);
        assert_eq!(total_ticks(&app, id(THE_END)), 10);
    }

    #[test]
    fn disabling_advance_time_stops_every_clock_without_pausing_any() {
        let mut app = app_with(all_clocks(), false);
        for _ in 0..10 {
            tick_app(&mut app);
        }

        assert_eq!(total_ticks(&app, id(OVERWORLD)), 0);
        assert_eq!(total_ticks(&app, id(THE_END)), 0);
        assert!(
            !app.world()
                .resource::<WorldClocks>()
                .get(id(OVERWORLD))
                .unwrap()
                .paused
        );

        app.world_mut().resource_mut::<AdvanceTime>().0 = true;
        for _ in 0..10 {
            tick_app(&mut app);
        }
        assert_eq!(total_ticks(&app, id(OVERWORLD)), 10);
    }

    #[test]
    fn a_zero_rate_reads_as_paused() {
        let running = ClockState {
            rate: 0.5,
            ..ClockState::default()
        };
        let paused = ClockState {
            paused: true,
            ..running
        };
        assert!(paused.is_paused());

        let received = ClockState {
            rate: 0.0,
            ..ClockState::default()
        };
        assert!(received.is_paused());
        assert!(!running.is_paused());
    }

    // ── Time markers ─────────────────────────────────────────────────────────

    const DAY_TIMELINE: &str = "day.json";

    fn shipped_timeline(name: &str) -> Timeline {
        let bytes = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/minecraft/timeline")
            .join(name);
        TEST_CLOCKS.scope(|| serde_json::from_slice(&std::fs::read(bytes).unwrap()).unwrap())
    }

    fn timeline_json(clock: &str, period: Option<u32>, markers: serde_json::Value) -> Timeline {
        let mut json = serde_json::json!({"clock": clock, "time_markers": markers});
        if let Some(period) = period {
            json["period_ticks"] = period.into();
        }
        TEST_CLOCKS.scope(|| serde_json::from_value(json).unwrap())
    }

    fn markers_of(
        timelines: &[Timeline],
    ) -> Result<ClockTimeMarkers, Vec<(usize, DuplicateTimeMarker)>> {
        ClockTimeMarkers::derive(timelines, &TEST_CLOCKS.registry::<WorldClock>().unwrap())
    }

    #[test]
    fn the_shipped_markers_land_on_the_overworld_clock() {
        let markers = markers_of(&[
            shipped_timeline(DAY_TIMELINE),
            shipped_timeline("moon.json"),
            shipped_timeline("early_game.json"),
            shipped_timeline("villager_schedule.json"),
        ])
        .unwrap();

        let names: Vec<&str> = markers
            .of_clock(id(OVERWORLD))
            .map(|(id, _)| id.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "minecraft:day",
                "minecraft:midnight",
                "minecraft:night",
                "minecraft:noon",
                "minecraft:roll_village_siege",
                "minecraft:wake_up_from_sleep",
            ]
        );

        let day = markers.get(id(OVERWORLD), "minecraft:day").unwrap();
        assert_eq!(day.ticks, 1000);
        assert_eq!(day.period_ticks, Some(24_000));
        assert!(day.show_in_commands);
        assert!(
            !markers
                .get(id(OVERWORLD), "minecraft:roll_village_siege")
                .unwrap()
                .show_in_commands
        );

        // Two markers may share a tick; two markers may not share an id.
        assert_eq!(
            markers
                .get(id(OVERWORLD), "minecraft:midnight")
                .unwrap()
                .ticks,
            18_000
        );
        assert_eq!(
            markers
                .get(id(OVERWORLD), "minecraft:roll_village_siege")
                .unwrap()
                .ticks,
            18_000
        );
    }

    #[test]
    fn a_clock_with_no_markers_answers_none_rather_than_panicking() {
        let markers = markers_of(&[shipped_timeline(DAY_TIMELINE)]).unwrap();
        assert!(markers.get(id(THE_END), "minecraft:day").is_none());
        assert_eq!(markers.of_clock(id(THE_END)).count(), 0);
        assert!(markers.get(id(OVERWORLD), "datapack:nothing").is_none());
    }

    #[test]
    fn one_marker_id_may_not_be_declared_twice_for_one_clock() {
        let repeats = markers_of(&[
            timeline_json(
                OVERWORLD,
                Some(24_000),
                serde_json::json!({"minecraft:noon": 6_000}),
            ),
            timeline_json(
                OVERWORLD,
                Some(24_000),
                serde_json::json!({"minecraft:noon": 7_000}),
            ),
        ])
        .unwrap_err();

        let [(index, repeat)] = repeats.as_slice() else {
            panic!("{repeats:?}");
        };
        assert_eq!(*index, 1);
        assert_eq!(
            repeat.to_string(),
            "time marker `minecraft:noon` was defined more than once for clock `minecraft:overworld`"
        );
    }

    #[test]
    fn one_marker_id_on_two_clocks_is_fine() {
        let markers = markers_of(&[
            timeline_json(
                OVERWORLD,
                Some(24_000),
                serde_json::json!({"minecraft:noon": 6_000}),
            ),
            timeline_json(
                "minecraft:the_end",
                None,
                serde_json::json!({"minecraft:noon": 7_000}),
            ),
        ])
        .unwrap();
        assert_eq!(
            markers.get(id(OVERWORLD), "minecraft:noon").unwrap().ticks,
            6_000
        );
        let end = markers.get(id(THE_END), "minecraft:noon").unwrap();
        assert_eq!((end.ticks, end.period_ticks), (7_000, None));
    }
}
