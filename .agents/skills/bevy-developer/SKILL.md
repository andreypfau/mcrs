---
name: bevy-developer
description: "Use when writing or reviewing code against Bevy 0.20 (`bevy`, `bevy_ecs`, `bevy_app`, `bevy_time`, `bevy_tasks`, `bevy_asset`): schedules and system sets, run conditions, `Single`/`Populated`, `Message`, observers with `On<E>` and `EntityEvent`, change detection (`Changed<T>`, `set_if_neq`), `SubApp` extraction, `Time<Fixed>`, task pools, diagnostics and tracy traces, and tests with `App::update`. Triggers: Bevy, bevy_ecs, bevy_app, Bevy plugin, Bevy schedule, Bevy query, Bevy system, observer, SubApp, extract, FixedUpdate, ambiguity detection, Bevy test, Bevy 0.20."
license: MIT
metadata:
  bevy: "0.20.1"
  related-skills: bevy-ecs
---

# Bevy Developer (0.20)

Pair this skill with `bevy-ecs`: that skill decides what is stored, derived and
owned; this one covers the scheduler and the API traps. Project rules in
`AGENTS.md` win over every default here. For renamed or removed items, and for
behaviour that changed since 0.16, read `references/bevy-0-20-facts.md` before
you answer from memory.

## Rules

- Read time from `Res<Time>` (the fixed clock inside fixed schedules), never from
  `Instant::now()` in game logic.
- Give a same-run producer and consumer an ordering edge (`.chain`, `.before`,
  `.after`, a named `SystemSet`). `Commands` apply at the next sync point.
- Name every system set a plugin exposes so that other plugins can order against it.
- Write through `set_if_neq` or compare first. Every `&mut` deref marks the
  component changed.
- Use `Single<D, F>` or `Populated<D, F>` instead of `query.single().unwrap()`.
  They skip the system when the query does not match.
- Gate usually-idle systems with `run_if`. Every system run has a fixed
  scheduler cost.
- Keep reusable buffers in a `Local<T>` or a resource and `clear()` them.
- Run disk, network and heavy CPU work on `IoTaskPool` or `AsyncComputeTaskPool`
  and collect with `bevy::tasks::futures::check_ready`. Dropping a `Task` cancels it.
- A channel from another thread is drained by an ordinary system that holds the
  receiver in a resource and spawns through `Commands`.
- A library crate exposes plugins and never calls `App::run()`.
- Break ties between entities by a stable key (entity id, arrival order), never
  by which thread ran first.
- Inside `par_iter_mut` each entity writes only itself. Cross-entity effects go
  out as messages or `ParallelCommands` and resolve later in a stable order.
  Measure before and after; `par_iter` pays task overhead.

Do not:

- Use an exclusive `&mut World` system where `Commands`, a query or a
  `SystemParam` does the job.
- Block a schedule on I/O or `block_on` an unfinished task.
- Use `EventReader`, `EventWriter`, `trigger_targets` or `trigger.target()`.
- Wrap a resource in `Mutex` or `RwLock`, or spawn OS threads for ECS work.
- Use `SingleThreadedExecutor` to "make it deterministic". Fix the missing edges.
- Toggle a marker to avoid inserting a heavy component. It is still an archetype move.
- Hide ordering in a `Local` counter or in plugin add order.
- Register every feature from one plugin so that nothing can be disabled.

## Change detection

- `Changed<T>` is true after any `&mut T` deref, even when the value is equal.
- Narrow a `Mut` to one field with `map_unchanged(|v| &mut v.field)` and call
  `set_if_neq` on it.
- `Changed<T>` checks a tick per candidate. For sparse change over a large
  population, the producer writes touched ids into a `Message` or a dirty set;
  the consumer uses `iter_many`.
- `RemovedComponents<T>` is message-backed and keeps entries for two updates. A
  consumer that runs less often misses removals; use an `On<Remove, T>` observer
  or a message from the remover.
- `ResMut::set_if_neq` and `Res::is_changed` behave the same for resources.
- `Time::<Fixed>::from_hz(..)` sets the fixed rate. Keep a rate in floats or per
  tick in integers; `(rate * dt) as i32` truncates to 0.

## Sub-apps

A `SubApp` has its own `World` and schedules, for example one world per level.
Build it with an `AppLabel`, `SubApp::new()`, `set_extract(|main, sub| ..)` and
`app.insert_sub_app(label, sub)`. Dimension sub-apps in mcrs do not use `set_extract`:
main-to-sub data is a field of `DimInput` and sub-to-main data is a field of `DimOutput`.

- Global mutable state has one writer, in the main app. A sub-app receives a
  read-only copy and never advances its own copy.
- Keep a version next to the value. Extraction copies only on a version change.
  Re-inserting a clone every update allocates and makes `resource_changed` always true.
- Move or copy immutable data (tables, configuration) in once when the sub-app is built.
- Extraction holds `&mut World` on both sides and blocks both. Copy data; no game logic.
- A periodic job with its own `Local` counter per sub-app drifts. Derive the
  period from the shared clock.

## Tests

- Drive the production plugins with `App::update()`. Do not copy a system into
  a test and do not sleep.
- `TimeUpdateStrategy::FixedTimesteps(n)` runs the fixed schedules exactly `n`
  times per update; `ManualDuration(d)` advances by `d`.
- Assert on world state after the update, not on observer order.
- Test pure rules as functions without Bevy; test the system for wiring only.
- Check ordering with ambiguity detection set to error. Build the real app, run
  the check on the main app and on every `SubApp`, before the first `update()`
  (a schedule already built is not rebuilt). Filter conflicts inside Bevy's own crates.

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

## Tracing and diagnostics

| Cargo feature (umbrella `bevy`) | Output |
|---|---|
| `trace_chrome` | a `trace-*.json` for Perfetto |
| `trace_tracy` | live connection to the Tracy GUI |
| `trace_tracy_memory` | Tracy plus allocation tracking |

- With only sub-crates, enable `bevy_ecs/trace` and `bevy_app/trace` and install
  a `tracing` subscriber yourself. Add spans around heavy blocks, not per element:
  `let _span = info_span!("rebuild", tiles = queue.len()).entered();`
- Register a diagnostic with `DiagnosticPath::const_new("a/b")` and
  `app.register_diagnostic(Diagnostic::new(PATH))`; record with
  `Diagnostics::add_measurement`. A gauge records a level, a counter a rate; do
  not mix them in one path.
- `FrameTimeDiagnosticsPlugin`, `EntityCountDiagnosticsPlugin`,
  `SystemInformationDiagnosticsPlugin` and `LogDiagnosticsPlugin` live in
  `bevy_diagnostic`.
- Do not optimize without a trace or a number. Do not log per frame or per
  entity; use a diagnostic or `warn_once!`.
