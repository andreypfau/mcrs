---
name: bevy-developer
description: "Use when writing or reviewing code against Bevy 0.19 (`bevy`, `bevy_ecs`, `bevy_app`, `bevy_time`, `bevy_tasks`, `bevy_asset`): Plugins, `Schedule`s and `SystemSet`s, run conditions, `Query`/`Single`/`Populated`, `Commands`, `Message`/`MessageReader`, observers with `On<E>` and `EntityEvent`, change detection (`Changed<T>`, `set_if_neq`), `SubApp` extraction, `Time<Fixed>`, task pools, diagnostics and tracy traces, and tests with `App::update`. Covers windowed games, headless servers, tools and library crates. Triggers: Bevy, bevy_ecs, bevy_app, Bevy plugin, Bevy schedule, Bevy query, Bevy system, observer, SubApp, extract, FixedUpdate, ambiguity detection, Bevy test, Bevy 0.19."
license: MIT
metadata:
  version: "2.1.0"
  bevy: "0.19.1"
  related-skills: bevy-ecs
---

# Bevy Developer (0.19)

Writes Bevy 0.19 code for any kind of Bevy app: a game with a window, a headless
server, an editor tool, a reusable plugin crate, a test.

Pair this skill with `bevy-ecs` before designing state. This skill covers the API
and the scheduler. `bevy-ecs` decides what is stored, what is derived and who owns
each projection. When a project has its own rules (a `CLAUDE.md`, a perf budget,
a tick rate), those rules win over every default here.

## Core workflow

1. **Classify the data first.** Run the `bevy-ecs` classification on every new
   field: authoritative truth, derived state, materialized projection, or
   temporal fact. Only then choose `Component`, `Resource`, `Message` or a local.
2. **Pick the schedule from the clock it follows.** Work that must be
   independent of frame rate (physics, gameplay timers, a network simulation
   tick) goes in the fixed schedules at the rate the project chooses with
   `Time::<Fixed>::from_hz(..)` (default 64 Hz). Per-frame work (input sampling,
   interpolation, animation, UI) goes in `Update`/`PostUpdate`. One-time setup
   goes in `Startup`.
3. **Order with sets, never with luck.** A producer and a consumer in the same
   schedule run get an explicit edge: `.chain()`, `.before()`/`.after()`, or a
   named `SystemSet`. Check important schedules with ambiguity detection set to
   error in a test (`references/testing.md`).
4. **Give each system one transformation.** A system applies one rule to every
   entity that matches its query. A wide system that does everything for one
   entity at a time serializes the schedule and is hard to reorder.
5. **Keep work proportional to change where populations are large.**
   `Changed<T>` still visits every candidate. For sparse change over many
   entities, the producer writes the touched ids into a queue (a `Message` or a
   set in a resource).
6. **Gate systems that are usually idle** with `run_if`. Each system run has a
   fixed scheduler cost even when its query is empty.
7. **Measure.** Use tracy spans and `bevy_diagnostic` measurements against the
   project's frame or tick budget (`references/performance-and-profiling.md`).

## Reference guide

| Topic | Reference | Load when |
|---|---|---|
| App, plugins, schedules, sub-apps | `references/app-plugins-schedules.md` | Building an `App` (windowed or headless), writing a `Plugin`, `SystemSet`s, run conditions, states, `SubApp` and `set_extract` |
| ECS API patterns | `references/ecs-patterns.md` | Queries and filters, `Commands`, observers and `EntityEvent`, messages, change detection, relationships, `SystemParam` |
| 0.19 facts and renames | `references/bevy-0-19-facts.md` | Any API that changed in 0.17–0.19, a compile error after an upgrade, an answer from memory that might be stale |
| Testing | `references/testing.md` | Tests with `App::update`, deterministic time, reading messages in a test, ambiguity checks |
| Performance | `references/performance-and-profiling.md` | Frame or tick budget, idle cost, work queues, allocation, exclusive systems, async I/O, tracy, diagnostics |
| Assets and rendering | `references/assets-and-rendering.md` | `AssetLoader`, `AssetServer`, hot reload, materials and shaders, background meshing |

## Constraints

### MUST

- Read time from `Res<Time>` (or `Res<Time<Fixed>>` inside the fixed
  schedules), never from `std::time::Instant` inside game logic. A wall-clock
  timer makes behaviour depend on machine speed and breaks pause and time scale.
- Express every same-run dependency as an ordering edge, and remember that
  `Commands` apply at the next sync point.
- Write through `set_if_neq` (requires `PartialEq`) or compare before writing.
  A `&mut` deref marks the component changed even when the value is the same.
- Choose marker or value by churn (`bevy-ecs` Rule 10). State that flips often is
  a value field; a marker is for rare membership changes that queries filter on.
  `SparseSet` storage makes insert and remove cheaper but still changes the
  archetype.
- Keep buffers that a hot system reuses in a `Local<T>` or a resource and
  `clear()` them, instead of allocating each run.
- Put disk, network and heavy CPU work on `IoTaskPool` or
  `AsyncComputeTaskPool`, and collect finished results with
  `bevy::tasks::futures::check_ready`.
- Give global mutable state one writer. A `SubApp` receives it through
  `set_extract`, copied only when it changed.
- Name every system set a plugin exposes, so that other plugins can order
  against it.

### MUST NOT

- Do not reach for an exclusive (`&mut World`) system when `Commands`, a query
  or a `SystemParam` can do the job. It runs alone and stops every other system
  in the schedule. Keep it for real world-wide access: extraction between
  worlds, a queued `Command`, driving a sub-app, scene or save application, test
  setup.
- Do not block a schedule on I/O: no synchronous file or network access inside a
  system, and no `block_on` on a task that has not finished.
- Do not call `query.single().unwrap()`. Use `Single<D, F>` or
  `Populated<D, F>`, which skip the system when the query does not match, or
  handle the `Result`.
- Do not use `EventReader`/`EventWriter`, `trigger_targets` or
  `trigger.target()`. They do not exist in 0.19. See
  `references/bevy-0-19-facts.md`.
- Do not insert a resource clone every frame to "sync" it. That allocates and
  marks it changed every frame, so a `resource_changed` gate never skips.
- Do not wrap a resource in `Mutex`/`RwLock`, and do not spawn OS threads for
  ECS work; use the task pools.
- Do not call `App::run()` from a library crate. Expose plugins; the binary or
  the test owns the loop.

## Key code patterns

### Plugin with a public set, a fixed-rate system and an observer

```rust
use bevy::prelude::*;

pub struct CombatPlugin;

#[derive(SystemSet, Hash, Eq, PartialEq, Debug, Clone)]
pub struct CombatSet;

#[derive(Component, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component)]
pub struct Regen {
    pub per_second: f32,
}

#[derive(EntityEvent)]
pub struct DamageTaken {
    pub entity: Entity,
    pub amount: f32,
}

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, regenerate.in_set(CombatSet))
            .add_observer(apply_damage);
    }
}

fn regenerate(time: Res<Time>, mut q: Query<(&mut Health, &Regen)>) {
    let dt = time.delta_secs();
    for (mut hp, regen) in &mut q {
        let next = (hp.current + regen.per_second * dt).min(hp.max);
        hp.map_unchanged(|h| &mut h.current).set_if_neq(next);
    }
}

fn apply_damage(damage: On<DamageTaken>, mut q: Query<&mut Health>) {
    if let Ok(mut hp) = q.get_mut(damage.entity) {
        hp.current = (hp.current - damage.amount).max(0.0);
    }
}
```

- Trigger it with `commands.trigger(DamageTaken { entity, amount: 4.0 })`.
- `regenerate` has no `Changed` filter: regeneration must run while nothing else
  touches `Health`. `set_if_neq` stops it from marking full-health entities
  changed.
- Inside `FixedUpdate`, `Res<Time>` is the fixed clock. Keep the rate in
  floating point or per tick in integers; `(rate * dt) as i32` truncates to 0.

### State-driven flow

```rust
app.init_state::<AppState>()
    .add_systems(OnEnter(AppState::InGame), spawn_level)
    .add_systems(OnExit(AppState::InGame), despawn_level)
    .add_systems(Update, gameplay.run_if(in_state(AppState::InGame)));
```

Full examples, headless setup and sub-apps are in
`references/app-plugins-schedules.md`.
