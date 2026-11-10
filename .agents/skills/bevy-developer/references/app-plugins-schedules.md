# App, Plugins, Schedules, Sub-apps

Bevy 0.19.1. Snippets use `bevy::prelude::*`. A crate that depends only on
the sub-crates imports the same items from `bevy_app`, `bevy_ecs`, `bevy_time`
and so on.

## Building an App

```rust
use bevy::prelude::*;

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, animate)
        .run()
}
```

Headless (a server, a simulation, a CLI tool):

```rust
use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use core::time::Duration;

fn main() -> AppExit {
    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            Duration::from_secs_f64(1.0 / 60.0),
        )))
        .run()
}
```

- `DefaultPlugins` and `MinimalPlugins` exist only in the umbrella `bevy` crate.
  With only `bevy_app`/`bevy_ecs`, list the plugins yourself:
  `TaskPoolPlugin::default()`, `TimePlugin`, `ScheduleRunnerPlugin`.
- `TaskPoolPlugin` must be present before anything uses `ComputeTaskPool`,
  `AsyncComputeTaskPool` or `IoTaskPool`, including `par_iter`.
- A host can drive the app itself with `app.update()` instead of a runner.
  Tests always do (`testing.md`).
- `run_loop(d)` sets how often `App::update` runs. The fixed schedules have
  their own rate, `Time::<Fixed>::from_hz(..)`; set both on purpose.

## Plugins

A plugin is the public surface of a feature: it inserts resources, declares
sets, registers messages and adds systems and observers.

```rust
use bevy::prelude::*;

pub struct InventoryPlugin {
    pub slots: usize,
}

#[derive(Resource)]
pub struct InventoryConfig {
    pub slots: usize,
}

#[derive(SystemSet, Hash, Eq, PartialEq, Debug, Clone)]
pub enum InventorySet {
    Input,
    Apply,
    Ui,
}

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(InventoryConfig { slots: self.slots })
            .configure_sets(
                Update,
                (InventorySet::Input, InventorySet::Apply, InventorySet::Ui).chain(),
            )
            .add_systems(
                Update,
                (
                    read_clicks.in_set(InventorySet::Input),
                    apply_transactions.in_set(InventorySet::Apply),
                    refresh_slot_ui.in_set(InventorySet::Ui),
                ),
            );
    }
}

fn read_clicks() {}
fn apply_transactions() {}
fn refresh_slot_ui() {}
```

- A reusable plugin takes its configuration through its own fields, so the
  host decides where the values come from.
- A `PluginGroup` (`PluginGroupBuilder::start::<Self>().add(..)`) lets the host
  `disable::<P>()` one member. Ship removable behaviour as its own plugin.
- `Plugin::build` runs once. `Plugin::finish` and `Plugin::cleanup` run after all
  plugins are built; use `finish` when a plugin needs a resource that another
  plugin inserts.

## Schedules

| Schedule | Runs | Use for |
|---|---|---|
| `PreStartup` / `Startup` / `PostStartup` | once | world construction, loading level data |
| `First` / `PreUpdate` | every `App::update` | input and network ingest, time setup |
| `RunFixedMainLoop` | every `App::update` | runs the fixed schedules zero or more times |
| `FixedFirst` … `FixedLast` | at `Time<Fixed>` rate | physics, gameplay timers, a simulation tick |
| `Update` / `PostUpdate` | every `App::update` | per-frame work, interpolation, transform propagation |
| `Last` | every `App::update` | end-of-frame cleanup |
| `OnEnter(S)` / `OnExit(S)` / `OnTransition { exited, entered }` | on a state change | per-state setup and teardown |

- The fixed schedules run as many times as the accumulated time requires, so
  zero or several fixed ticks per `App::update` is normal.
- The default fixed rate is 64 Hz. Change it with
  `Time::<Fixed>::from_hz(..)` when the game or protocol needs another rate.
- `Time<Virtual>::set_max_delta` (default 250 ms) caps how much time one update
  adds. That cap is what prevents a catch-up spiral after a stall.
- A custom schedule is a `ScheduleLabel`. Run it from an exclusive system with
  `world.run_schedule(label)`, or insert it into `MainScheduleOrder` / `FixedMainScheduleOrder`.

## Ordering

Systems in one schedule run in parallel unless their access conflicts. A
conflicting pair without an edge runs in an arbitrary order, which differs from
run to run.

```rust
app.configure_sets(FixedUpdate, (Phase::Ingest, Phase::Simulate, Phase::Resolve).chain())
    .add_systems(FixedUpdate, integrate.in_set(Phase::Simulate))
    .add_systems(FixedUpdate, write_blocks.in_set(Phase::Resolve).after(collect_writes));
```

- A same-run producer and consumer need an edge. A `Commands` insert is visible
  to a later system only after a sync point; Bevy inserts `ApplyDeferred`
  automatically between ordered systems that need it.
- `.ambiguous_with(other)` and `App::ignore_ambiguity(schedule, a, b)` document
  a conflict that does not matter. Use them for a reason, never to silence a
  test.
- Ambiguity detection is `LogLevel::Ignore` by default. Turn it on in a test, not
  in production builds (`testing.md`).

## Run conditions

A run condition is a read-only system that returns `bool`. When it returns
`false`, the scheduler skips the system and its cost.

```rust
app.add_systems(
    FixedUpdate,
    (
        rebuild_navmesh.run_if(on_message::<TerrainChanged>),
        save_game.run_if(resource_changed::<SaveRequested>),
        advance_weather.run_if(resource_exists::<Weather>.and_then(not(paused))),
    ),
);
```

- Built-in conditions are function items and are written without parentheses:
  `on_message::<M>`, `resource_exists::<R>`, `resource_changed::<R>`,
  `resource_exists_and_changed::<R>`, `any_with_component::<C>`,
  `in_state(S::X)` (this one takes an argument), `state_changed::<S>`.
- Combine them with `.and_then(..)`, `.or_else(..)`, `.nand_then(..)`, or the
  `*_eager` forms. `.and(..)` and `.or(..)` are deprecated since 0.19.0.
- `on_message::<M>` is true when messages arrived since the condition last ran,
  not "this frame".
- A run condition cannot mutate (`SystemCondition` requires `ReadOnlySystem`).
  Change-detection-based conditions therefore depend on how often the condition
  itself runs.

## States

```rust
use bevy::prelude::*;

#[derive(States, Default, Hash, Eq, PartialEq, Debug, Clone, Copy)]
enum AppState {
    #[default]
    Loading,
    MainMenu,
    InGame,
}

app.init_state::<AppState>()
    .add_systems(OnEnter(AppState::InGame), spawn_level)
    .add_systems(OnExit(AppState::InGame), despawn_level)
    .add_systems(Update, gameplay.run_if(in_state(AppState::InGame)));
```

- `NextState::set(s)` always runs `OnExit`/`OnEnter`, even when `s` equals the
  current state. `NextState::set_if_neq(s)` skips the transition in that case.
- `SubStates` exist only while a parent state has a given value.
  `ComputedStates` are a pure function of other states and must not be set by
  hand.
- States are for coarse lifecycle phases. A per-entity mode is a component value.

## Sub-apps

A `SubApp` has its own `World` and its own schedules. The render world is one;
an app can add its own for an isolated simulation, for example one world per
level or per match on a server.

```rust
use bevy::{app::{AppLabel, SubApp}, prelude::*};

#[derive(AppLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct LevelApp;

#[derive(Resource, Clone)]
struct Clocks {
    version: u64,
    ticks: Vec<(ClockId, u64)>,
}

let mut level = SubApp::new();
level.update_schedule = Some(FixedUpdate.intern());
level.set_extract(|main, level| {
    let Some(clocks) = main.get_resource::<Clocks>() else { return };
    if level.get_resource::<Clocks>().is_some_and(|held| held.version == clocks.version) {
        return;
    }
    level.insert_resource(clocks.clone());
});
app.insert_sub_app(LevelApp, level);
```

- Global mutable state has one writer, in the main app. Each sub-app receives a
  read-only copy through `set_extract`. A sub-app that advances its own copy
  diverges silently from the others.
- Extraction compares a version and copies only when it changed. Re-inserting a
  clone every update allocates and makes `resource_changed` always true.
- Immutable data (lookup tables, configuration) is moved or copied in once when the
  sub-app is built.
- Extraction runs with `&mut World` on both sides and blocks both worlds. Keep
  it to copying data, with no game logic in it.

## Anti-patterns

- A library plugin that calls `App::run()`.
- One plugin that registers every feature, so that nothing can be disabled.
- Ordering hidden in a `Local` counter, or "it runs first because it was added
  first".
- A periodic job with its own `Local` counter in each sub-app. Each copy starts
  at a different time; derive the period from the shared clock.
