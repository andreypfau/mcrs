---
paths:
  - "crates/mcrs_engine_*/**"
  - "crates/mcrs_minecraft_engine*/**"
  - "crates/mcrs_minecraft_{simulation,physics,brain,goals,goal_world,mob_control,replication}/**"
  - "crates/mcrs_minecraft_{light_engine,vanilla_light,move_predict,move_validate,entity_rows}/**"
  - "crates/mcrs_minecraft_server/**"
---

# Simulation: determinism, shards, dimensions

## Dimension sub-apps

Each dimension runs as a Bevy `SubApp` with its own `World`, held by `SubAppWorld<DimInput, DimOutput>`.
Data from the main app to a sub-app is a field of `DimInput`. Data from a sub-app to the main app
is a field of `DimOutput`. Nothing else crosses. That is how `WorldClocks` reaches dimension systems.

## Determinism

- **Exact, as the reference:** world generation; the arithmetic of the player movement model;
  anything rolled from a seed stored in the save; the protocol and the save format.
- **Semantic, ready to become exact:** neighbour updates, scheduled ticks, block events,
  redstone, pistons and fluids. The gameplay result matches the reference; the order within a
  tick need not, for now. Every queue keeps the fields the reference orders by (due tick,
  priority, insertion sequence), and the order inside a shard is decided in one replaceable
  ordering policy per domain, so moving to tick-for-tick parity changes that policy, not the
  design. Tick-for-tick fixtures from the game are still recorded as its target.
- **Independent of the scheduler:** everything else. The same inputs give the same tick
  regardless of which thread ran first; a conflict between entities is decided by a stable key
  such as arrival order or entity id. Matching the reference's random sequence is not required,
  only its distributions.
- **Free:** the order of entities and systems that share no written data.
- **Intentional deviations are listed.** A subsystem that departs from the reference on purpose
  says so in its specification or design note, and a differential test consults that list before
  calling a difference a bug.

Simulation schedules are tested with ambiguity detection set to error.

## Ticks and budgets

- **Generation and light results apply at the start of a tick, never between ticks.** Applying
  them at a wall-clock moment makes the starting world depend on thread timing.
- **Syncing an unchanged inventory costs at most 3 us per player.** The sync compares slots in
  place and builds a stack value only to send it.
