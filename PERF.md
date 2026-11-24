# Performance record

This file records benchmark results that a pull request changes. Each section covers one
benchmark. Each table compares `main` with the pull request on the same machine, run back to back.

## `motion_pass`

Command: `cargo bench -p mcrs_minecraft_engine_physics --bench motion_pass`.

- Machine: Apple M4 Max, 16 threads.
- Date: 2026-11-21.
- Load average: 8 to 10 during the runs, from other processes. The machine was not quiet.
- Change: push, body grid and mob intent in the motion pass (`Refs #1067`).

The benchmark spawns the items without `MotionId`. On the server, each replicated item has a
`MotionId`, and `MotionId` requires a `GridSlot`. One run with a `MotionId` on each item was
10% to 18% slower than the runs without `MotionId`, at a load average of 20 to 30. The body grid owner
visits each entity with a `GridSlot` on each tick, also when the entity can not have a grid
row.

## `entity_pass`

Command: `cargo bench -p mcrs_minecraft_engine_physics --bench entity_pass`.

- Machine: Apple M4 Max.
- Change: mobs run as ECS entities, and the rows crate is deleted (`Closes #1067`).

The case is one tick of 50k walkers and 50k items. The time is the criterion median of one tick.

| Case | `main` | This change | Delta |
| - | - | - | - |
| 50k walkers + 50k items, one tick | 2.03 ms | 2.96 ms | 1.46x |

The owner accepts this cost for the migration. Issue #1074 tracks the work to recover it.

## `entity_pass` and `motion_pass`: the hit pipeline

Commands: `cargo bench -p mcrs_minecraft_engine_physics --bench entity_pass` and
`--bench motion_pass`, with `--save-baseline main` on `main` and `--baseline main` on the change,
run back to back.

- Machine: Apple M4 Max, 16 threads.
- Date: 2026-11-22.
- Load average: 14 to 20 for both sides, from other processes. The machine was not quiet.
- Change: the health of a living entity moves into `Vitals`, and each hit goes through the sorted
  queue of `mcrs_minecraft_engine_damage` (`Closes #103`).

| Case | `main` | This change | Delta |
| - | - | - | - |
| 50k walkers + 50k items, one tick | 2.97 ms | 2.82 ms | -4.9% |
| 100k items at rest, components | 920 µs | 991 µs | +6.3% |
| 100k items in flight, components | 3.23 ms | 3.18 ms | -1.7% |
| 1M items, 25% awake, components | 16.46 ms | 17.21 ms | +1.9% (p = 0.28) |
| spawn 10k items, components | 351 µs | 342 µs | -2.5% |

Three more runs of the case "100k items at rest" against the same `main` baseline gave +1.0%
(p = 0.24), +3.7% and +11.9%. The load of the machine changed during these runs. The step query
of each entity now also reads the optional `Vitals`. The motion step no longer runs the counters
of a mob.
