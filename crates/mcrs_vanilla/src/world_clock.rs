use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetLoader, AssetServer, Assets, LoadContext, UntypedAssetId,
    VisitAssetDependencies,
};
use bevy_ecs::prelude::*;
use bevy_reflect::TypePath;
use bevy_state::state::OnEnter;
use mcrs_core::registry::snapshot::rl_from_asset_path;
use mcrs_core::{AppState, ResourceLocation};
use serde::{Deserialize, Serialize};

use crate::timeline::Timeline;

/// Unit-shaped registry entry. Vanilla `WorldClock.DIRECT_CODEC` is
/// `MapCodec.unitCodec(WorldClock::new)`, so the wire payload is an
/// empty NBT compound.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TypePath)]
pub struct WorldClock {}

impl Asset for WorldClock {}

impl VisitAssetDependencies for WorldClock {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct WorldClockLoader;

#[derive(Debug, thiserror::Error)]
pub enum WorldClockLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for WorldClockLoader {
    type Asset = WorldClock;
    type Settings = ();
    type Error = WorldClockLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<WorldClock, WorldClockLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        if bytes.iter().all(|b| b.is_ascii_whitespace()) {
            return Ok(WorldClock {});
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// One clock's authoritative state.
///
/// `total_ticks` is the only fact; the tick within a timeline period, the
/// time of day and the network form are all derived from it on read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClockState {
    pub total_ticks: i64,
    #[serde(default)]
    pub partial_tick: f32,
    #[serde(default = "default_rate")]
    pub rate: f32,
    #[serde(default)]
    pub paused: bool,
}

fn default_rate() -> f32 {
    1.0
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

    pub fn network_state(&self, advance_time: bool) -> ClockNetworkState {
        ClockNetworkState {
            total_ticks: self.total_ticks,
            partial_tick: self.partial_tick,
            rate: if self.paused || !advance_time {
                0.0
            } else {
                self.rate
            },
        }
    }
}

/// The wire form of a [`ClockState`]: no `paused` field, a zeroed `rate`
/// standing in for both a paused clock and a disabled `advance_time`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockNetworkState {
    pub total_ticks: i64,
    pub partial_tick: f32,
    pub rate: f32,
}

/// Every clock of the `world_clock` registry, keyed by id.
///
/// A resource rather than an entity per clock: this crosses into every
/// dimension sub-world once per tick, and entities do not cross worlds.
#[derive(Resource, Debug, Clone, Default)]
pub struct WorldClocks(HashMap<ResourceLocation<Arc<str>>, ClockState>);

impl WorldClocks {
    pub fn get(&self, clock: &str) -> Option<&ClockState> {
        self.0.get(clock)
    }

    pub fn get_mut(&mut self, clock: &str) -> Option<&mut ClockState> {
        self.0.get_mut(clock)
    }

    pub fn insert(&mut self, clock: ResourceLocation<Arc<str>>, state: ClockState) {
        self.0.insert(clock, state);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &ClockState)> {
        self.0.iter()
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
    pub fn reconcile_with_registry(
        &mut self,
        registered: impl IntoIterator<Item = ResourceLocation<Arc<str>>>,
    ) {
        let registered: Vec<_> = registered.into_iter().collect();
        self.0.retain(|id, _| {
            let known = registered.contains(id);
            if !known {
                tracing::warn!(clock = %id, "discarding clock state with no world_clock registry entry");
            }
            known
        });
        for id in registered {
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

impl ClockTimeMarker {
    pub fn occurs_at(&self, total_ticks: i64) -> bool {
        i64::from(self.ticks)
            == match self.period_ticks {
                Some(period) => total_ticks.rem_euclid(i64::from(period)),
                None => total_ticks,
            }
    }

    /// The total tick count a clock must be moved to for this marker to occur
    /// next. Always strictly forward: standing on the marker moves a whole
    /// period, never nowhere.
    pub fn resolve_time_to_move_to(&self, total_ticks: i64) -> i64 {
        let Some(period) = self.period_ticks.map(i64::from) else {
            return i64::from(self.ticks);
        };
        let duration = i64::from(self.ticks) - total_ticks.rem_euclid(period);
        total_ticks + if duration > 0 { duration } else { period + duration }
    }

    pub fn repetition_count(&self, total_ticks: i64) -> i64 {
        let Some(period) = self.period_ticks.map(i64::from) else {
            return i64::from(total_ticks >= i64::from(self.ticks));
        };
        total_ticks.div_euclid(period)
            + i64::from(total_ticks.rem_euclid(period) >= i64::from(self.ticks))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TimeMarkerError {
    #[error("time marker `{marker}` at tick {ticks} must be in range [0; {period})")]
    OutsidePeriod {
        marker: String,
        ticks: u32,
        period: u32,
    },
    #[error("time marker `{marker}` is declared more than once on clock `{clock}`")]
    Duplicate { marker: String, clock: String },
    #[error("`{0}` is not a valid resource location")]
    Malformed(String),
}

/// Every time marker the loaded timelines declare, grouped by the clock they
/// run on.
///
/// Markers have no registry of their own: this table is the projection of the
/// `timeline` registry that makes them addressable, and it is rebuilt from
/// that registry rather than accumulated.
#[derive(Resource, Debug, Clone, Default)]
pub struct ClockTimeMarkers(
    HashMap<ResourceLocation<Arc<str>>, BTreeMap<ResourceLocation<Arc<str>>, ClockTimeMarker>>,
);

impl ClockTimeMarkers {
    pub fn get(&self, clock: &str, marker: &str) -> Option<&ClockTimeMarker> {
        self.0.get(clock)?.get(marker)
    }

    pub fn of_clock(
        &self,
        clock: &str,
    ) -> impl Iterator<Item = (&ResourceLocation<Arc<str>>, &ClockTimeMarker)> {
        self.0.get(clock).into_iter().flat_map(BTreeMap::iter)
    }

    pub fn clocks(&self) -> impl Iterator<Item = &ResourceLocation<Arc<str>>> {
        self.0.keys()
    }

    pub fn len(&self) -> usize {
        self.0.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0.values().all(BTreeMap::is_empty)
    }

    /// Fold the markers of every loaded timeline into one table per clock.
    ///
    /// Returns everything the data got wrong; a rejected marker is left out of
    /// the table rather than replacing what is already there.
    pub fn rebuild<'a>(
        &mut self,
        timelines: impl IntoIterator<Item = &'a Timeline>,
    ) -> Vec<TimeMarkerError> {
        self.0.clear();
        let mut errors = Vec::new();
        for timeline in timelines {
            let clock = &timeline.clock;
            for (id, marker) in &timeline.time_markers {
                // Exclusive at the top, unlike the inclusive keyframe bound:
                // a marker on the period boundary would occur twice a period.
                if let Some(period) = timeline.period_ticks
                    && marker.ticks >= period
                {
                    errors.push(TimeMarkerError::OutsidePeriod {
                        marker: id.clone(),
                        ticks: marker.ticks,
                        period,
                    });
                    continue;
                }
                let Ok(id_rl) = ResourceLocation::<Arc<str>>::parse(id) else {
                    errors.push(TimeMarkerError::Malformed(id.clone()));
                    continue;
                };
                let resolved = ClockTimeMarker {
                    ticks: marker.ticks,
                    period_ticks: timeline.period_ticks,
                    show_in_commands: marker.show_in_commands,
                };
                match self.0.entry(clock.clone()).or_default().entry(id_rl) {
                    Entry::Vacant(slot) => {
                        slot.insert(resolved);
                    }
                    Entry::Occupied(_) => errors.push(TimeMarkerError::Duplicate {
                        marker: id.clone(),
                        clock: clock.to_string(),
                    }),
                }
            }
        }
        errors
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
        app.init_asset::<WorldClock>()
            .register_asset_loader(WorldClockLoader)
            .init_resource::<WorldClocks>()
            .init_resource::<ClockTimeMarkers>()
            .init_resource::<AdvanceTime>()
            .add_systems(OnEnter(AppState::WorldgenFreeze), seed_world_clocks)
            .add_systems(
                FixedUpdate,
                advance_world_clocks.run_if(advance_time_enabled),
            );
    }
}

pub fn seed_world_clocks(
    mut clocks: ResMut<WorldClocks>,
    assets: Res<Assets<WorldClock>>,
    asset_server: Res<AssetServer>,
) {
    clocks.reconcile_with_registry(
        assets
            .iter()
            .filter_map(|(id, _)| rl_from_asset_path(asset_server.get_path(id)?.path()))
            .collect::<Vec<_>>(),
    );
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
mod tests {
    use super::*;
    use crate::attribute::AttributeValue;
    use crate::timeline::Timeline;

    const OVERWORLD: &str = "minecraft:overworld";

    fn rl(id: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(id).unwrap()
    }

    fn clocks(ids: &[&str]) -> WorldClocks {
        let mut clocks = WorldClocks::default();
        clocks.reconcile_with_registry(ids.iter().map(|id| rl(id)));
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

    fn total_ticks(app: &App, clock: &str) -> i64 {
        app.world()
            .resource::<WorldClocks>()
            .get(clock)
            .unwrap()
            .total_ticks
    }

    #[test]
    fn reconcile_gives_one_clock_per_registry_entry() {
        let mut clocks = WorldClocks::default();
        clocks.insert(
            rl(OVERWORLD),
            ClockState {
                total_ticks: 500,
                ..ClockState::default()
            },
        );
        clocks.insert(rl("datapack:removed"), ClockState::default());

        clocks.reconcile_with_registry([rl(OVERWORLD), rl("minecraft:the_end")]);

        assert_eq!(clocks.len(), 2);
        assert_eq!(clocks.get(OVERWORLD).unwrap().total_ticks, 500);
        assert_eq!(clocks.get("minecraft:the_end").unwrap().total_ticks, 0);
        assert!(clocks.get("datapack:removed").is_none());
    }

    #[test]
    fn a_full_day_returns_the_clock_to_the_same_point() {
        let mut app = app_with(clocks(&[OVERWORLD]), true);
        for _ in 0..24_000 {
            tick_app(&mut app);
        }

        let state = *app.world().resource::<WorldClocks>().get(OVERWORLD).unwrap();
        assert_eq!(state.total_ticks, 24_000);
        assert_eq!(state.total_ticks.rem_euclid(24_000), 0);
        assert_eq!(state.partial_tick, 0.0);
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
        let mut app = app_with(clocks(&[OVERWORLD, "minecraft:the_end"]), true);
        app.world_mut()
            .resource_mut::<WorldClocks>()
            .get_mut(OVERWORLD)
            .unwrap()
            .paused = true;

        for _ in 0..10 {
            tick_app(&mut app);
        }

        assert_eq!(total_ticks(&app, OVERWORLD), 0);
        assert_eq!(total_ticks(&app, "minecraft:the_end"), 10);
    }

    #[test]
    fn disabling_advance_time_stops_every_clock_without_pausing_any() {
        let mut app = app_with(clocks(&[OVERWORLD, "minecraft:the_end"]), false);
        for _ in 0..10 {
            tick_app(&mut app);
        }

        assert_eq!(total_ticks(&app, OVERWORLD), 0);
        assert_eq!(total_ticks(&app, "minecraft:the_end"), 0);
        assert!(!app.world().resource::<WorldClocks>().get(OVERWORLD).unwrap().paused);

        app.world_mut().resource_mut::<AdvanceTime>().0 = true;
        for _ in 0..10 {
            tick_app(&mut app);
        }
        assert_eq!(total_ticks(&app, OVERWORLD), 10);
    }

    #[test]
    fn the_network_state_reports_a_zero_rate_for_either_kind_of_stop() {
        let running = ClockState {
            rate: 0.5,
            ..ClockState::default()
        };
        assert_eq!(running.network_state(true).rate, 0.5);
        assert_eq!(running.network_state(false).rate, 0.0);

        let paused = ClockState {
            paused: true,
            ..running
        };
        assert_eq!(paused.network_state(true).rate, 0.0);

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
        serde_json::from_slice(&std::fs::read(bytes).unwrap()).unwrap()
    }

    fn timeline_json(clock: &str, period: Option<u32>, markers: serde_json::Value) -> Timeline {
        let mut json = serde_json::json!({"clock": clock, "time_markers": markers});
        if let Some(period) = period {
            json["period_ticks"] = period.into();
        }
        serde_json::from_value(json).unwrap()
    }

    fn markers_of(timelines: &[Timeline]) -> (ClockTimeMarkers, Vec<TimeMarkerError>) {
        let mut table = ClockTimeMarkers::default();
        let errors = table.rebuild(timelines);
        (table, errors)
    }

    #[test]
    fn the_shipped_markers_land_on_the_overworld_clock() {
        let (markers, errors) = markers_of(&[
            shipped_timeline(DAY_TIMELINE),
            shipped_timeline("moon.json"),
            shipped_timeline("early_game.json"),
            shipped_timeline("villager_schedule.json"),
        ]);
        assert!(errors.is_empty(), "{errors:?}");

        let names: Vec<&str> = markers.of_clock(OVERWORLD).map(|(id, _)| id.as_str()).collect();
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

        let day = markers.get(OVERWORLD, "minecraft:day").unwrap();
        assert_eq!(day.ticks, 1000);
        assert_eq!(day.period_ticks, Some(24_000));
        assert!(day.show_in_commands);
        assert!(!markers.get(OVERWORLD, "minecraft:roll_village_siege").unwrap().show_in_commands);

        // Two markers may share a tick; two markers may not share an id.
        assert_eq!(markers.get(OVERWORLD, "minecraft:midnight").unwrap().ticks, 18_000);
        assert_eq!(markers.get(OVERWORLD, "minecraft:roll_village_siege").unwrap().ticks, 18_000);
    }

    #[test]
    fn a_clock_with_no_markers_answers_none_rather_than_panicking() {
        let (markers, _) = markers_of(&[shipped_timeline(DAY_TIMELINE)]);
        assert!(markers.get("minecraft:the_end", "minecraft:day").is_none());
        assert_eq!(markers.of_clock("minecraft:the_end").count(), 0);
        assert!(markers.get(OVERWORLD, "datapack:nothing").is_none());
    }

    #[test]
    fn a_marker_on_the_period_boundary_is_rejected() {
        let (markers, errors) = markers_of(&[timeline_json(
            OVERWORLD,
            Some(24_000),
            serde_json::json!({"minecraft:day": 24_000, "minecraft:noon": 23_999}),
        )]);

        assert!(matches!(
            errors.as_slice(),
            [TimeMarkerError::OutsidePeriod { ticks: 24_000, period: 24_000, .. }]
        ), "{errors:?}");
        assert!(markers.get(OVERWORLD, "minecraft:day").is_none());
        assert!(markers.get(OVERWORLD, "minecraft:noon").is_some());
    }

    #[test]
    fn a_keyframe_on_the_period_boundary_is_still_accepted() {
        let timeline: Timeline = serde_json::from_value(serde_json::json!({
            "clock": OVERWORLD,
            "period_ticks": 24_000,
            "tracks": {
                "minecraft:visual/star_brightness": {
                    "keyframes": [{"ticks": 0, "value": 0.0}, {"ticks": 24_000, "value": 1.0}]
                }
            }
        }))
        .unwrap();
        assert_eq!(timeline.bake().len(), 1);
    }

    #[test]
    fn one_marker_id_may_not_be_declared_twice_for_one_clock() {
        let (markers, errors) = markers_of(&[
            timeline_json(OVERWORLD, Some(24_000), serde_json::json!({"minecraft:noon": 6_000})),
            timeline_json(OVERWORLD, Some(24_000), serde_json::json!({"minecraft:noon": 7_000})),
        ]);
        assert!(matches!(errors.as_slice(), [TimeMarkerError::Duplicate { .. }]), "{errors:?}");
        assert_eq!(markers.get(OVERWORLD, "minecraft:noon").unwrap().ticks, 6_000);
    }

    #[test]
    fn one_marker_id_on_two_clocks_is_fine() {
        let (markers, errors) = markers_of(&[
            timeline_json(OVERWORLD, Some(24_000), serde_json::json!({"minecraft:noon": 6_000})),
            timeline_json("minecraft:the_end", None, serde_json::json!({"minecraft:noon": 7_000})),
        ]);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(markers.get(OVERWORLD, "minecraft:noon").unwrap().ticks, 6_000);
        let end = markers.get("minecraft:the_end", "minecraft:noon").unwrap();
        assert_eq!((end.ticks, end.period_ticks), (7_000, None));
    }

    #[test]
    fn rebuilding_replaces_the_table_rather_than_adding_to_it() {
        let mut markers = ClockTimeMarkers::default();
        markers.rebuild(&[shipped_timeline(DAY_TIMELINE)]);
        assert_eq!(markers.len(), 6);

        markers.rebuild(&[timeline_json(
            OVERWORLD,
            Some(24_000),
            serde_json::json!({"minecraft:noon": 6_000}),
        )]);
        assert_eq!(markers.len(), 1);
        assert!(markers.get(OVERWORLD, "minecraft:day").is_none());
    }

    #[test]
    fn a_periodic_marker_always_moves_forward() {
        let day = ClockTimeMarker { ticks: 1000, period_ticks: Some(24_000), show_in_commands: true };

        assert!(!day.occurs_at(2_000));
        assert_eq!(day.resolve_time_to_move_to(2_000), 25_000);

        // Standing on the marker jumps a whole period rather than nowhere.
        assert!(day.occurs_at(1_000));
        assert_eq!(day.resolve_time_to_move_to(1_000), 25_000);

        assert_eq!(day.resolve_time_to_move_to(0), 1_000);
        assert_eq!(day.resolve_time_to_move_to(24_000), 25_000);
        assert!(day.occurs_at(25_000));

        assert_eq!(day.repetition_count(0), 0);
        assert_eq!(day.repetition_count(999), 0);
        assert_eq!(day.repetition_count(1_000), 1);
        assert_eq!(day.repetition_count(24_000), 1);
        assert_eq!(day.repetition_count(25_000), 2);
    }

    #[test]
    fn a_marker_at_tick_zero_occurs_at_tick_zero() {
        let wake_up =
            ClockTimeMarker { ticks: 0, period_ticks: Some(24_000), show_in_commands: false };
        assert!(wake_up.occurs_at(0));
        assert_eq!(wake_up.repetition_count(0), 1);
        assert_eq!(wake_up.resolve_time_to_move_to(0), 24_000);
        assert_eq!(wake_up.repetition_count(24_000), 2);
    }

    #[test]
    fn a_marker_without_a_period_happens_once() {
        let once = ClockTimeMarker { ticks: 1_000, period_ticks: None, show_in_commands: false };

        assert!(once.occurs_at(1_000));
        assert!(!once.occurs_at(25_000));
        assert_eq!(once.resolve_time_to_move_to(2_000), 1_000);
        assert_eq!(once.resolve_time_to_move_to(0), 1_000);
        assert_eq!(once.repetition_count(999), 0);
        assert_eq!(once.repetition_count(1_000), 1);
        assert_eq!(once.repetition_count(1_000_000), 1);
    }

    #[test]
    fn a_day_track_samples_through_a_clock() {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/minecraft/timeline/day.json"),
        )
        .unwrap();
        let timeline: Timeline = serde_json::from_slice(&bytes).unwrap();
        let sky_light = timeline.bake()["minecraft:gameplay/sky_light_level"].clone();

        let mut app = app_with(clocks(&[OVERWORLD]), true);
        let sample = |app: &App| {
            let state = app
                .world()
                .resource::<WorldClocks>()
                .get(timeline.clock.as_str())
                .expect("the timeline names a clock the registry has");
            match sky_light.sample_argument(state.total_ticks) {
                AttributeValue::Float(v) => v,
                other => panic!("expected a float, got {other:?}"),
            }
        };

        for _ in 0..6_000 {
            tick_app(&mut app);
        }
        assert_eq!(sample(&app), 1.0);

        for _ in 0..12_000 {
            tick_app(&mut app);
        }
        assert_eq!(total_ticks(&app, OVERWORLD), 18_000);
        assert_eq!(sample(&app), 0.26666668);
    }
}
