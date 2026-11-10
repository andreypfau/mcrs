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

Each dimension runs as a Bevy `SubApp` with its own `World`. Immutable registries are cloned in
once at spawn; mutable per-tick state is pushed every tick through `sub_app.set_extract(...)`.
That is how `Time<…>` and `WorldClocks` reach dimension systems.

Global mutable state has **one writer in the main app** and reaches sub-worlds as a read-only
snapshot. A sub-app that advances its own copy diverges silently and per dimension.

## Determinism

- **Exact, as the reference:** world generation; the arithmetic of the player movement model;
  anything rolled from a seed stored in the save; the protocol and the save format.
- **Semantic, ready to become exact:** neighbour updates, scheduled ticks, block events,
  redstone, pistons and fluids. The gameplay result matches the reference; the order within a
  tick need not, for now. Every queue keeps the fields the reference orders by (due tick,
  priority, insertion sequence), and the order inside a shard is decided in one replaceable
  ordering policy per domain, so moving to tick-for-tick parity changes that policy, not the
  design. Tick-for-tick fixtures from the game are still recorded as its target.
- **Shards, not regions.** A simulation partition is a shard: one domain's ownership unit
  (block updates, redstone, fluids, entities), formed from causally connected work, merged and
  split dynamically, never a thread. Sections index space; cross-shard effects are messages
  applied in a stable order. "Region" means only the Anvil region file.
- **A shard step is a library call, not a Bevy system.** Each domain is a kernel without Bevy:
  shard state, a world view through a trait the host implements, and inbound messages in; writes
  and outbound messages out, into caller-owned buffers. The Bevy driver runs every shard of a
  domain in parallel; a loop, a test or a mod over the game's own chunks drives the same kernel.
  One shard over the whole world with the reference ordering policy is the exact-parity mode, on
  the same code path as the sharded one.
- **Independent of the scheduler:** everything else. The same inputs give the same tick
  regardless of which thread ran first; a conflict between entities is decided by a stable key
  such as arrival order or entity id. Matching the reference's random sequence is not required,
  only its distributions.
- **Free:** the order of entities and systems that share no written data.
- **Intentional deviations are listed.** A subsystem that departs from the reference on purpose
  says so in its specification or design note, and a differential test consults that list before
  calling a difference a bug.

Simulation schedules are tested with ambiguity detection set to error.

## Time

- The tick never waits: no `thread::sleep`, no blocking receive, no lock held across a stage.
- A kernel does not read the wall clock. A deadline that depends on `Instant` makes the result
  depend on the machine; budget work in items, not in time.
