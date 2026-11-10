# Performance and Profiling (0.19)

Measure against the budget the app actually has: a frame (16.67 ms at 60 Hz,
8.33 ms at 120 Hz) for a game, a tick for a server or simulation. The executor
charges for every system it runs, so an idle app has a cost of its own;
measure it separately from the cost of real work.

## Cost model to keep in mind

| Thing | Cost |
|---|---|
| A system run | executor overhead even when its query is empty; gate with `run_if` |
| `With<T>`/`Without<T>` | per archetype, cheap |
| `Changed<T>`/`Added<T>` | per candidate entity, a tick comparison |
| `&mut T` deref | marks `T` changed, so `Changed<T>` consumers do work next run |
| insert/remove a component | moves the entity to another archetype (also with `SparseSet`) |
| `&mut World` system | runs alone; every other system in the schedule waits |
| observer trigger | one observer run per trigger |
| `Commands` | recorded, applied at a sync point; each sync point is a barrier |

## Work proportional to change

`Changed<T>` is fine for small populations. For many entities and few changes,
the producer records what it touched:

```rust
#[derive(Message, Clone, Copy)]
pub struct TileChanged(pub Entity);

fn rebuild_paths(
    mut changed: MessageReader<TileChanged>,
    mut seen: Local<EntityHashSet>,
    mut tiles: Query<&mut PathCost>,
) {
    seen.clear();
    for &TileChanged(tile) in changed.read() {
        if seen.insert(tile) {
            if let Ok(mut cost) = tiles.get_mut(tile) {
                // recompute only this tile
            }
        }
    }
}
```

- Gate the consumer with `run_if(on_message::<TileChanged>)`, so an idle
  frame does not run it.
- A wake-up that has no producer signal ("re-test every pending entry every
  frame") is the defect to fix: add the signal at the producer.
- Scanning a whole pending map to find ready entries is O(pending) per run. Keep
  a ready queue that the code that makes an entry ready pushes to.

## Allocation in hot systems

A system that runs every frame or tick should not allocate in the steady
state. Typical offenders:

- `Vec::new()`, `collect()`, `format!` or a temporary `HashSet` inside a system.
  Keep the buffer in a `Local<T>` or a resource and `clear()` it.
- Cloning a large payload once per recipient. Encode a broadcast once into
  shared bytes and hand each recipient a reference to them.
- `insert_resource(x.clone())` every update to copy state into another world.
  Copy only when a version changes.
- `Arc::make_mut` on a value another holder still shares. That clones the whole
  value.

Prove it with a counting global allocator in a test, or with tracy memory
tracking (`trace_tracy_memory`).

## Exclusive systems and blocking I/O

```rust
use bevy::tasks::{futures::check_ready, IoTaskPool, Task};

#[derive(Component)]
struct PendingSave(Task<std::io::Result<()>>);

fn start_autosave(mut commands: Commands, players: Query<(Entity, &PlayerData), Changed<PlayerData>>) {
    let pool = IoTaskPool::get();
    for (entity, data) in &players {
        let bytes = data.encode();
        let task = pool.spawn(async move { write_atomically(bytes).await });
        commands.entity(entity).insert(PendingSave(task));
    }
}

fn finish_autosave(mut commands: Commands, mut saves: Query<(Entity, &mut PendingSave)>) {
    for (entity, mut save) in &mut saves {
        if let Some(result) = check_ready(&mut save.0) {
            if let Err(error) = result {
                warn!(?error, "autosave failed");
            }
            commands.entity(entity).remove::<PendingSave>();
        }
    }
}
```

- `check_ready` is `bevy_tasks::futures::check_ready` (`bevy::tasks::futures`).
  It polls once and never blocks.
- Dropping a `Task` cancels it. A task that must finish is held until it is done
  or detached with `.detach()`.
- A channel from another thread is drained by an ordinary system that reads
  the receiver from a resource and spawns through `Commands`, not by a
  `&mut World` system that runs every update.
- A synchronous file read on spawn or on despawn also blocks the schedule.
  Start the I/O on `IoTaskPool` and finish the spawn when the result arrives.

## Parallel iteration

```rust
q.par_iter_mut()
    .batching_strategy(BatchingStrategy::fixed(256))
    .for_each(|(mut pos, vel)| pos.0 += vel.0);
```

- `par_iter` pays task overhead. It helps when there are thousands of entities
  or the per-entity work is heavy. Measure before and after.
- `BatchingStrategy` is `bevy_ecs::batching::BatchingStrategy`.
- Inside `par_iter_mut` each entity writes only itself. Cross-entity effects go
  out as messages or through `ParallelCommands` and are resolved in a later step
  in a stable order.

## Ambiguity and determinism

- Two systems with conflicting access and no edge run in an unspecified order.
  `ScheduleBuildSettings { ambiguity_detection: LogLevel::Error, .. }` turns
  that into a build error. Use it in a test (`testing.md`).
- When results must be reproducible (lockstep networking, replays, tests), break
  ties between entities by a stable key (entity id, arrival order), never by
  which thread ran first.
- `Instant::now()` in game logic makes the result depend on the machine. Use
  `Time`.

## Tracing

| Cargo feature (umbrella `bevy`) | Output |
|---|---|
| `trace_chrome` | a `trace-*.json` for Perfetto |
| `trace_tracy` | live connection to the Tracy GUI |
| `trace_tracy_memory` | Tracy plus allocation tracking |

With only the sub-crates, enable `bevy_ecs/trace` and `bevy_app/trace` and install a
`tracing` subscriber yourself. Add spans around heavy blocks, not per element:

```rust
let _span = info_span!("rebuild_paths", tiles = queue.len()).entered();
```

## Diagnostics

```rust
use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};

const PENDING: DiagnosticPath = DiagnosticPath::const_new("jobs/pending");

app.register_diagnostic(Diagnostic::new(PENDING))
    .add_systems(Last, |mut d: Diagnostics, q: Res<Pending>| {
        d.add_measurement(&PENDING, || q.len() as f64);
    });
```

- A gauge records a level (queue length); a counter records a rate (jobs
  finished per second). Do not mix them in one path.
- `FrameTimeDiagnosticsPlugin`, `EntityCountDiagnosticsPlugin`,
  `SystemInformationDiagnosticsPlugin` and `LogDiagnosticsPlugin` are in
  `bevy_diagnostic`. `LogDiagnosticsPlugin` writes one summary per interval.

## Anti-patterns

- Optimizing without a trace or a number.
- Logging per frame or per entity. Use a diagnostic, or `warn_once!`.
- Spawning OS threads for ECS work instead of the task pools.
- `SingleThreadedExecutor` to "make it deterministic". Fix the missing edges.
- Toggling a marker to "avoid" inserting a heavy component. It is still an
  archetype move; use a value field.
