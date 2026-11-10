# Testing Bevy Code (0.19)

A test drives the production plugins with `App::update()`. It does not copy a
system into the test, and it does not sleep.

## Layout

- Prefer one integration-test binary per crate (`tests/it/main.rs` plus
  modules). Each file directly under `tests/` is a separate binary that links
  all of Bevy, which costs link time.
- Share expensive immutable fixtures (parsed data, loaded tables) through a
  `LazyLock`, and build the cheap per-test `App` from them.
- Put the rule under test in a function without Bevy where possible, and test
  that function directly. Test the system for its wiring: schedule, ordering,
  run condition.

## Deterministic time

```rust
use bevy::{prelude::*, time::TimeUpdateStrategy};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
        .add_plugins(CombatPlugin);
    app.update();
    app
}

#[test]
fn regen_reaches_max_and_stops() {
    let mut app = app();
    let e = app.world_mut().spawn((Health { current: 1.0, max: 3.0 }, Regen { per_second: 64.0 })).id();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(app.world().get::<Health>(e).map(|h| h.current), Some(3.0));
}
```

- `FixedTimesteps(n)` makes each `update()` run the fixed schedules exactly `n`
  times. `ManualDuration(d)` advances by `d` per update, which lets a test
  produce a zero-tick update or a catch-up update on purpose.
- The first `update()` only initializes the clock: its delta is zero and no
  fixed tick runs. Call `update()` once in the fixture before the test counts
  ticks.
- Never read `Instant::now()` in code under test. A wall-clock timer makes the
  result depend on the machine.

## Reading messages in a test

```rust
let mut cursor = app.world().resource::<Messages<ItemPickedUp>>().get_cursor();
app.update();
let messages = app.world().resource::<Messages<ItemPickedUp>>();
let picked: Vec<_> = cursor.read(messages).collect();
```

- `get_cursor()` starts at the oldest retained message; `get_cursor_current()`
  starts after the newest, so only later messages are read.
- Messages are kept for two updates. Read them before the second `update()`
  after they were written.
- Write input with `app.world_mut().write_message(..)`.

## Observers and commands

- `world.trigger(ev)` runs observers before it returns.
  `commands.trigger(ev)` runs them when the queue is applied, so a test that
  uses `Commands` must call `update()` or `world.flush()` first.
- Assert on world state after the update, not on the order in which observers
  ran.

## Async tasks

Do not sleep. Either run the task's body as a plain function in the test, or
loop `app.update()` until the result appears, with a bound on iterations:

```rust
for _ in 0..100 {
    app.update();
    if app.world().get::<Loaded>(e).is_some() {
        break;
    }
    std::thread::yield_now();
}
assert!(app.world().get::<Loaded>(e).is_some());
```

`TaskPoolPlugin` must be in the test app when the code under test uses a task
pool.

## Ambiguity check

Schedules whose order matters (simulation, networking, anything replayed) are
worth a test with ambiguity detection set to error. Build the real app, then
rebuild each schedule with stricter settings:

```rust
use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings, Schedules};

fn ambiguous_schedules(world: &mut World) -> Vec<String> {
    let labels: Vec<_> = world.resource::<Schedules>().iter().map(|(_, s)| s.label()).collect();
    labels
        .into_iter()
        .filter_map(|label| {
            world
                .try_schedule_scope(label, |world, schedule| {
                    let lenient = schedule.get_build_settings().clone();
                    schedule.set_build_settings(ScheduleBuildSettings {
                        ambiguity_detection: LogLevel::Error,
                        ..lenient.clone()
                    });
                    let report = schedule
                        .initialize(world)
                        .err()
                        .map(|e| format!("{label:?}: {}", e.to_string(schedule.graph(), world)));
                    schedule.set_build_settings(lenient);
                    report
                })
                .ok()
                .flatten()
        })
        .collect()
}
```

- A schedule that was already built before the check is not rebuilt; check it
  before the first `update()`, or mark it changed.
- Run the check on the main app and on every `SubApp`, for every app the
  project builds (for example both client and server).
- Filter out conflicts that belong to Bevy's own crates, not your systems.

## What makes a test worth keeping

- It can fail because of our code: a wrong rule, a missing ordering edge, a lost
  message, an allocation on a hot path (assert with a counting allocator).
- It does not restate a constant, a derive, or Bevy's own behaviour.
- Cases that differ only in data are one table-driven test.
