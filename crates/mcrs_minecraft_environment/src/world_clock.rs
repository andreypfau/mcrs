#[cfg(test)]
use mcrs_minecraft_resource::{ResourceKeyBuf, TagKeyBuf};
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(feature = "bevy")]
use bevy_app::{App, FixedUpdate, Plugin, PreUpdate};
#[cfg(feature = "bevy")]
use bevy_ecs::prelude::*;
use mcrs_minecraft_core::codec::is_default;
#[cfg(feature = "bevy")]
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::{Id, ResourceLocationBuf};
use serde::{Deserialize, Serialize};

use crate::timeline::Timeline;

/// Unit-shaped registry entry. Vanilla `WorldClock.DIRECT_CODEC` is
/// `MapCodec.unitCodec(WorldClock::new)`, so the wire payload is an
/// empty NBT compound.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldClock {}

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

    pub fn apply(&mut self, edit: ClockEdit) {
        match edit {
            ClockEdit::SetTotalTicks(ticks) => {
                self.total_ticks = ticks;
                self.partial_tick = 0.0;
            }
            ClockEdit::AddTicks(ticks) => {
                self.total_ticks = self.total_ticks.saturating_add(i64::from(ticks)).max(0);
            }
            ClockEdit::SetPaused(paused) => self.paused = paused,
            ClockEdit::SetRate(rate) => self.rate = rate,
        }
    }
}

/// A change to one clock state that an action requests of its owner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockEdit {
    /// Sets the total ticks, and the partial tick to 0.
    SetTotalTicks(i64),
    /// Adds to the total ticks, never below 0; the partial tick stays.
    AddTicks(i32),
    SetPaused(bool),
    SetRate(f32),
}

/// Every clock of the `world_clock` registry, keyed by id and walked in id order.
///
/// A resource rather than an entity per clock: this crosses into every
/// dimension sub-world once per tick, and entities do not cross worlds.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "bevy", derive(Resource))]
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

    /// Leave exactly one clock per entry of a registry of `entries` entries: keep the
    /// supplied state for an entry that has one, default the rest, drop what the
    /// registry no longer holds.
    pub fn reconcile(&mut self, entries: usize) {
        self.0.retain(|id, _| {
            let known = id.index() < entries;
            if !known {
                tracing::warn!(clock = ?id, "discarding clock state with no world_clock registry entry");
            }
            known
        });
        for id in (0..entries).filter_map(|n| Id::checked(u16::try_from(n).ok()?, entries)) {
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
    /// The marker occurs at the total ticks `t`.
    pub fn occurs_at(&self, t: i64) -> bool {
        let ticks = i64::from(self.ticks);
        match self.period_ticks {
            Some(period) => t.checked_rem(i64::from(period)) == Some(ticks),
            None => t == ticks,
        }
    }

    /// The total ticks at which the marker occurs next for a clock at `t`. Without a period
    /// that is its tick, which can lie before `t`.
    pub fn next_after(&self, t: i64) -> i64 {
        let ticks = i64::from(self.ticks);
        let Some(period) = self.period_ticks.map(i64::from) else {
            return ticks;
        };
        let d = ticks - t.checked_rem(period).unwrap_or(0);
        let step = if d > 0 { d } else { period + d };
        t.saturating_add(step)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("time marker `{marker}` was defined more than once for clock `{clock}`")]
pub struct DuplicateTimeMarker {
    pub marker: ResourceLocationBuf,
    pub clock: ResourceLocationBuf,
}

/// Every time marker the loaded timelines declare, grouped by the clock they
/// run on.
///
/// Markers have no registry of their own: this table is derived once from the
/// `timeline` column and never touched again.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct ClockTimeMarkers(Arc<BTreeMap<Id<WorldClock>, MarkersOfClock>>);

type MarkersOfClock = BTreeMap<ResourceLocationBuf, ClockTimeMarker>;

impl ClockTimeMarkers {
    pub fn get(&self, clock: Id<WorldClock>, marker: &str) -> Option<&ClockTimeMarker> {
        self.0.get(&clock)?.get(marker)
    }

    pub fn of_clock(
        &self,
        clock: Id<WorldClock>,
    ) -> impl Iterator<Item = (&ResourceLocationBuf, &ClockTimeMarker)> {
        self.0.get(&clock).into_iter().flat_map(BTreeMap::iter)
    }

    /// A marker id may be reused on another clock but not on the same one; every
    /// repeat is returned with the index of the timeline that repeated it.
    pub fn derive<'a>(
        timelines: &[Timeline],
        clock_name: impl Fn(Id<WorldClock>) -> Option<&'a ResourceLocationBuf>,
    ) -> Result<Self, Vec<(usize, DuplicateTimeMarker)>> {
        let mut table: BTreeMap<Id<WorldClock>, MarkersOfClock> = BTreeMap::new();
        let mut duplicates = Vec::new();
        for (index, timeline) in timelines.iter().enumerate() {
            let Some(clock) = clock_name(timeline.clock) else {
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

pub fn check_time_markers(
    timelines: &[Timeline],
    clocks: &mcrs_minecraft_registries::Registry<WorldClock>,
) -> Vec<(usize, String)> {
    let clock_name =
        |clock: Id<WorldClock>| clocks.name(Id::checked(clock.number(), clocks.len())?);
    match ClockTimeMarkers::derive(timelines, clock_name) {
        Ok(_) => Vec::new(),
        Err(duplicates) => duplicates
            .into_iter()
            .map(|(index, duplicate)| (index, duplicate.to_string()))
            .collect(),
    }
}

/// The global `advance_time` gamerule. Truth until a save reader supplies it.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "bevy", derive(Resource))]
pub struct AdvanceTime(pub bool);

impl Default for AdvanceTime {
    fn default() -> Self {
        Self(true)
    }
}

#[cfg(feature = "bevy")]
pub struct WorldClockPlugin;

#[cfg(feature = "bevy")]
impl Plugin for WorldClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClocks>()
            .init_resource::<AdvanceTime>()
            .add_systems(
                PreUpdate,
                seed_world_clocks.run_if(resource_exists_and_changed::<CurrentRegistries>),
            )
            .add_systems(
                FixedUpdate,
                advance_world_clocks.run_if(advance_time_enabled),
            );
    }
}

#[cfg(feature = "bevy")]
fn seed_world_clocks(mut clocks: ResMut<WorldClocks>, registries: Res<CurrentRegistries>) {
    let Some(registry) = registries.registry::<WorldClock>() else {
        tracing::error!("the registries hold no world_clock registry to seed the clocks from");
        return;
    };
    clocks.reconcile(registry.len());
    tracing::info!(clocks = clocks.len(), "seeded world clocks");
}

#[cfg(feature = "bevy")]
fn advance_time_enabled(advance_time: Res<AdvanceTime>) -> bool {
    advance_time.0
}

#[cfg(feature = "bevy")]
fn advance_world_clocks(mut clocks: ResMut<WorldClocks>) {
    for state in clocks.0.values_mut() {
        state.advance(1.0);
    }
}

/// Push the main app's clocks into a dimension sub-world. The sub-world holds
/// a read-only copy: it is overwritten wholesale every tick, so any local
/// advancement it attempted would be silently discarded.
#[cfg(feature = "bevy")]
pub fn extract_world_clocks(main_world: &mut World, sub_world: &mut World) {
    if let Some(clocks) = main_world.get_resource::<WorldClocks>() {
        sub_world.insert_resource(clocks.clone());
    }
}

/// The shipped world clocks numbered in name order, standing in for the registries a value that
/// names a clock is resolved and named against.
#[cfg(test)]
pub(crate) struct TestClocks(Vec<ResourceLocationBuf>);

#[cfg(test)]
pub(crate) static TEST_CLOCKS: std::sync::LazyLock<TestClocks> = std::sync::LazyLock::new(|| {
    let base = mcrs_minecraft_worldgen_testing::vanilla_dir().join("world_clock");
    let mut names: Vec<_> = mcrs_minecraft_worldgen_testing::json_files(&base)
        .iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_str().unwrap();
            ResourceLocationBuf::read(&format!("minecraft:{name}")).unwrap()
        })
        .collect();
    names.sort();
    TestClocks(names)
});

#[cfg(test)]
impl TestClocks {
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn id(&self, name: &str) -> Id<WorldClock> {
        let at = self
            .0
            .iter()
            .position(|known| known.as_str() == name)
            .unwrap();
        Id::checked(at as u16, self.0.len()).unwrap()
    }

    /// `json` read as the named form of `T` and resolved against the shipped clocks.
    pub(crate) fn read<T>(&self, json: serde_json::Value) -> Result<T, String>
    where
        T: mcrs_minecraft_registries::Resolve,
        T::Named: serde::de::DeserializeOwned,
    {
        let named = serde_json::from_value(json).map_err(|e| e.to_string())?;
        T::resolve(named, self).map_err(|e| e.to_string())
    }

    pub(crate) fn named<T: mcrs_minecraft_registries::Resolve>(&self, value: &T) -> T::Named {
        value.name(self).unwrap()
    }
}

#[cfg(test)]
impl mcrs_minecraft_registries::Resolver for TestClocks {
    fn id<R: mcrs_minecraft_resource::Registered>(
        &self,
        key: &ResourceKeyBuf<R>,
    ) -> Result<Id<R>, mcrs_minecraft_registries::ResolveError> {
        let at = self.0.iter().position(|name| name == key.location());
        at.and_then(|at| Id::checked(at as u16, self.0.len()))
            .ok_or_else(|| {
                format!("unknown entry {} of minecraft:world_clock", key.location()).into()
            })
    }

    fn tag<R: mcrs_minecraft_resource::Registered>(
        &self,
        tag: &TagKeyBuf<R>,
    ) -> Result<mcrs_minecraft_resource::TagId<R>, mcrs_minecraft_registries::ResolveError> {
        Err(format!("unknown tag #{}", tag.as_str()).into())
    }
}

#[cfg(test)]
impl mcrs_minecraft_registries::Namer for TestClocks {
    fn name<R: mcrs_minecraft_resource::Registered>(
        &self,
        id: Id<R>,
    ) -> Option<&ResourceLocationBuf> {
        self.0.get(id.index())
    }

    fn tag_name<R: mcrs_minecraft_resource::Registered>(
        &self,
        _: mcrs_minecraft_resource::TagId<R>,
    ) -> Option<&ResourceLocationBuf> {
        None
    }
}

#[cfg(test)]
#[cfg(feature = "bevy")]
mod tests {
    use super::*;
    use crate::timeline::Timeline;

    const OVERWORLD: &str = "minecraft:overworld";

    const THE_END: &str = "minecraft:the_end";

    fn id(name: &str) -> Id<WorldClock> {
        TEST_CLOCKS.id(name)
    }

    fn all_clocks() -> WorldClocks {
        let mut clocks = WorldClocks::default();
        clocks.reconcile(TEST_CLOCKS.len());
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
        let shipped = TEST_CLOCKS.len();
        let removed = Id::<WorldClock>::checked(shipped as u16, shipped + 1).unwrap();
        let mut clocks = WorldClocks::default();
        clocks.insert(
            id(OVERWORLD),
            ClockState {
                total_ticks: 500,
                ..ClockState::default()
            },
        );
        clocks.insert(removed, ClockState::default());

        clocks.reconcile(shipped);

        assert_eq!(clocks.len(), shipped);
        assert_eq!(clocks.get(id(OVERWORLD)).unwrap().total_ticks, 500);
        assert_eq!(clocks.get(id(THE_END)).unwrap().total_ticks, 0);
        assert!(clocks.get(removed).is_none());
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

    #[test]
    fn a_marker_occurs_and_comes_next_as_its_period_says() {
        let marker = |ticks, period_ticks| ClockTimeMarker {
            ticks,
            period_ticks,
            show_in_commands: true,
        };
        let cases = [
            (marker(100, None), 500, false, 100),
            (marker(100, None), 100, true, 100),
            (marker(1_000, Some(24_000)), 50_000, false, 73_000),
            (marker(1_000, Some(24_000)), 49_000, true, 73_000),
            (marker(13_000, Some(24_000)), 50_000, false, 61_000),
            (marker(0, Some(24_000)), 500, false, 24_000),
            (marker(0, Some(24_000)), 0, true, 24_000),
        ];
        for (marker, t, occurs, next) in cases {
            assert_eq!(
                (marker.occurs_at(t), marker.next_after(t)),
                (occurs, next),
                "{marker:?} at {t}"
            );
        }
    }

    #[test]
    fn an_edit_keeps_what_it_does_not_name() {
        let start = ClockState {
            total_ticks: 100,
            partial_tick: 0.5,
            rate: 2.0,
            paused: false,
        };
        let cases = [
            (ClockEdit::SetTotalTicks(7), (7, 0.0, 2.0, false)),
            (ClockEdit::AddTicks(-24_000), (0, 0.5, 2.0, false)),
            (ClockEdit::AddTicks(5), (105, 0.5, 2.0, false)),
            (ClockEdit::SetPaused(true), (100, 0.5, 2.0, true)),
            (ClockEdit::SetRate(0.25), (100, 0.5, 0.25, false)),
        ];
        for (edit, expected) in cases {
            let mut state = start;
            state.apply(edit);
            assert_eq!(
                (
                    state.total_ticks,
                    state.partial_tick,
                    state.rate,
                    state.paused
                ),
                expected,
                "{edit:?}"
            );
        }
    }

    // ── Time markers ─────────────────────────────────────────────────────────

    const DAY_TIMELINE: &str = "day.json";

    fn shipped_timeline(name: &str) -> Timeline {
        let bytes = mcrs_minecraft_worldgen_testing::vanilla_dir()
            .join("timeline")
            .join(name);
        TEST_CLOCKS
            .read(serde_json::from_slice(&std::fs::read(bytes).unwrap()).unwrap())
            .unwrap()
    }

    fn timeline_json(clock: &str, period: Option<u32>, markers: serde_json::Value) -> Timeline {
        let mut json = serde_json::json!({"clock": clock, "time_markers": markers});
        if let Some(period) = period {
            json["period_ticks"] = period.into();
        }
        TEST_CLOCKS.read(json).unwrap()
    }

    fn markers_of(
        timelines: &[Timeline],
    ) -> Result<ClockTimeMarkers, Vec<(usize, DuplicateTimeMarker)>> {
        ClockTimeMarkers::derive(timelines, |clock| {
            mcrs_minecraft_registries::Namer::name(&*TEST_CLOCKS, clock)
        })
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
