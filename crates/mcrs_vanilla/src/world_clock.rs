use std::collections::HashMap;
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockState {
    pub total_ticks: i64,
    pub partial_tick: f32,
    pub rate: f32,
    pub paused: bool,
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
            .init_resource::<AdvanceTime>()
            .add_systems(OnEnter(AppState::WorldgenFreeze), seed_world_clocks)
            .add_systems(
                FixedUpdate,
                advance_world_clocks.run_if(advance_time_enabled),
            );
    }
}

fn seed_world_clocks(
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

    #[test]
    fn a_day_track_samples_through_a_clock() {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/minecraft/timeline/day.json"),
        )
        .unwrap();
        let timeline: Timeline = serde_json::from_slice(&bytes).unwrap();
        let sky_light = timeline.bake().unwrap()["minecraft:gameplay/sky_light_level"].clone();

        let mut app = app_with(clocks(&[OVERWORLD]), true);
        let sample = |app: &App| {
            let state = app
                .world()
                .resource::<WorldClocks>()
                .get(&timeline.clock)
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
