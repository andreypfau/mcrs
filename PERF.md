# Performance record

This file records benchmark results that a pull request changes. Each section covers one
benchmark. Each table compares `main` with the pull request on the same machine, run back to back.

## `motion_pass`

Command: `cargo bench -p mcrs_minecraft_engine_physics --bench motion_pass`.

- Machine: Apple M4 Max, 16 threads.
- Date: 2026-11-21.
- Load average: 8 to 10 during the runs, from other processes. The machine was not quiet.
- Change: push, body grid and mob intent in the motion pass (`Refs #1067`).

The `components` side drives the items through the ECS motion pass. The `rows` side drives them
through the rows crate. Both sides run the items without a lifetime, so no item expires during a
run. The time is the criterion median of one tick.

| Case | Side | `main` | This change | Delta |
| - | - | - | - | - |
| 100k items at rest | rows | 606 µs | 607 µs | 0% |
| 100k items at rest | components | 903 µs | 992 µs | +10% |
| 100k items in flight | rows | 2.24 ms | 2.31 ms | +3% |
| 100k items in flight | components | 3.12 ms | 3.26 ms | +4% |
| 1M items, 25% awake | rows | 15.08 ms | 15.17 ms | +1% |
| 1M items, 25% awake | components | 15.96 ms | 17.01 ms | +7% |
| spawn 10k items | rows | 1.27 ms | 1.19 ms | -7% |
| spawn 10k items | components | 350 µs | 352 µs | +1% |

Two more back-to-back runs of the `components` cases gave +8% at rest, +4% to +5% in flight
and +4% to +6% for 1M items.

The components side is slower because each item step now does more work. The step checks the
optional mob intent, the grid slot and the living state, and then the item lifetime. A profile
shows the extra time in the step function itself. The new systems take no measurable time
when no entity has a grid slot.

The benchmark spawns the items without `MotionId`. On the server, each replicated item has a
`MotionId`, and `MotionId` requires a `GridSlot`. One run with a `MotionId` on each item was
10% to 18% slower than the table above, at a load average of 20 to 30. The body grid owner
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
