# mcrs

A Minecraft **library**, server **and client** in Rust on Bevy ECS. Not a demo.
The server and the client are two consumers of crates that a proxy, a bot, a
minigame server, an online world map or a model and structure viewer must be
able to take on their own. Assume every subsystem you touch will be read by both
processes, by code outside this repository, and by systems that do not exist
yet.

This file holds what applies everywhere. It is the instruction file for every
coding agent; `CLAUDE.md` is a link to it.

- **Skills** live in `.agents/skills/<name>/SKILL.md`. "Load `<name>`" in this
  file means: invoke that skill, or read its `SKILL.md` if your tool has no
  skills. `.claude/skills/` links to the same folders.
- **Domain rules** live in `.agents/rules/`. Claude Code loads them by the
  `paths` in their header. Other tools: read the rule before you edit a crate of
  its domain.

| Rule | Crates |
| - | - |
| `.agents/rules/rendering.md` | `*render*`, `mesh`, `model`, `client`, `client_gui`, `molang_wgsl` |
| `.agents/rules/codec.md` | registries, NBT/SNBT, data pack and pack files, text, loot, environment, dimension, predicates, worldgen, protocol, wire, save, Anvil |
| `.agents/rules/simulation.md` | `mcrs_engine_*`, `*_engine*`, simulation, physics, brain, goals, replication, light engine, movement, server |
| `.agents/rules/memory.md` | chunk, volume, voxel, block, entity rows, light engine, heightmap, Anvil, engine store and world |

## Layers

Each layer depends only on the layers below it. A user takes any prefix and pays
for nothing above it.

1. **Wire**: ids and positions, NBT, text, keys; codec traits and primitive
   codecs; framing, compression, encryption; the packets whose fields are not
   game data (handshake, status, login, configuration). No Bevy, no tokio, no
   files on disk.
2. **Network**: the connection state machine, login on both sides, Mojang
   authentication, proxy forwarding, raw frame passthrough, and the edge core:
   frames encoded once, subscriptions, per-connection encryption. tokio, no
   Bevy.
3. **Game data and rules**: registries baked from the data pack; blocks,
   items, entities and the other value types with their dense tables and the
   packets that carry them; section storage, volumes and Anvil I/O. Beside
   them, rules crates: kernels and pure models without Bevy (block simulation,
   light propagation, movement, collision, inventory transactions, world
   generation). Rules depend on data, never the reverse. Usable without the
   engine.
4. **Engine**: Bevy plugins for worlds, dimensions, chunk sources, players,
   events and commands, and the generic drivers of the kernels. No vanilla
   behaviour by default.
5. **Vanilla**: behaviour crates without Bevy (what a block, an item or a mob
   does) and the removable plugins that wire that behaviour, gameplay, world
   generation and lighting into the engine.
6. **Rendering**: meshing and pipelines over layer 3 data. Depends on neither
   the network, the client nor the server.
7. **Applications**: the server binary, the client, the examples.

- Logic lives in crates without Bevy; Bevy crates schedule it. A `bevy`
  feature on a crate below layer 4 adds derives, at most one resource and one
  message, is off by default and adds no system.
- Bevy is a feature below layer 4, never a requirement; tokio likewise below
  layer 2.
- Headless is first-class. The client is network, world state and input;
  rendering is a plugin it adds. A bot is the client with rendering off.
- What the server does with a player, a chunk, a command or a packet goes
  through a public message or plugin point a user can replace or cancel.
  Configuration comes through the plugin, never from environment variables or
  files in the working directory.
- **Library code never panics.** No `unwrap`, `expect`, `panic!`,
  `unreachable!`, `todo!` or out-of-bounds indexing outside tests: input a
  caller can get wrong returns `Option` or `Result`, and an internal invariant
  is expressed so that no panicking operation is needed.
- A `pub` item in layers 1–4 is a commitment to users. Keep the rest
  `pub(crate)`, and give errors that cross a public boundary a type a caller can
  match on.
- A crate that is specific to the game is named `mcrs_minecraft_*`. `mcrs_*`
  is for a concept that is proven generic.

## The one rule that outranks the rest

> The reference is the authority on **data**, never on **code shape**.

Take from the game: JSON layouts, default values, numeric constants, semantics,
ordering, and parity of behaviour. Do **not** take its object graph. A manager
object with a mutable map and a post-construction `init()`, a sampler that
memoizes against a tick id and is poked from outside, a probe that
double-buffers values and is ticked externally: each is the object-oriented
answer to a problem Bevy solves with a resource, a system and change detection.
If your design needs an `update_*`, `invalidate_*` or `apply_to_*` method that
pushes values into somebody else's state, that shape has leaked in.

## Clean room

`docs/clean-room.md` is the rule; read it before you write a specification or
implement one.

- A domain's behaviour is written down before it is built, as a specification in
  `docs/spec/` in the style of `docs/spec/README.md`. A domain without a
  specification gets one before its code.
- The author of a specification can read the reference; the implementer reads
  the specification, its design note and the fixtures, never the reference. A
  gap in a specification is a question to its author.
- No class, method or field name of the game's code, and no path into it,
  appears in code, comments, tests, specifications, notes, commit messages or
  issues. Never decompile.
- The reference is not in this repository. Each specification author keeps a
  local copy and never writes its location into a tracked file. Experiments
  against the game run through the tools that `docs/spec/README.md` lists in
  its section "Experiments".

## Version

Target the **26.4.x** branch, one snapshot at a time. The single source of truth
is `assets/minecraft/version.json`, read through `mcrs_minecraft_core::VERSION`;
a test fails when a Rust, TOML or Gradle file states one of its values. There is
no backward compatibility and no datafixer. A `DataVersion` mismatch is a loud
error. The data pack and the resource pack come from the pinned client jar, not
from the repository; generated reports and definitions are written by
`cargo run -p mcrs_minecraft_update`, never by hand. See
`tools/mcrs_minecraft_update/README.md`.

## Data-driven, not hardcoded

If it ships as JSON, read it from the asset. Derive sets from the registry and
the loaded assets instead of hand-writing lists: a data pack that adds a track,
a biome or a clock is picked up without a code change. A shape is a `Serialize`
/ `Deserialize` implementation, never a function over `serde_json::Value`, and
it is validated at load.

## ECS

**Load `bevy-ecs` and `bevy-developer` before designing anything.** Classify
every value you are about to store: authoritative truth, derived state,
materialized derived state, or temporal fact. Store the truth, derive the rest,
and give any materialization exactly one owner and a stated cost model.

- **A system is one transformation applied to every entity that has the
  components it names, never "tick this entity".**
- **A block is not an entity and its behaviour is not an object.** Block state
  properties live in dense tables indexed by numeric id; tags are bitsets. No
  `HashMap`, string or `dyn` lookup inside a tick loop.
- **Work is proportional to change, not to population.** `Changed<T>` still
  visits every candidate, so sparse changes over a large population take an
  explicit work queue.
- **The tick runs in phases with few sync points:** ingest packets; simulate in
  parallel, each entity writing only itself and emitting messages; resolve
  cross-entity effects and world writes serially in sorted order; project
  derived state; encode packets in parallel per connection.
- **No system takes `&mut World`.** `&mut World` belongs to the seams only:
  extraction between worlds, a queued command, driving a sub-app, a test fixture.
- **No structural change on a hot path.** State that flips often is a value, not
  a marker.
- **No allocation in a steady-state tick.** Buffers live in a `Local` or a
  resource; a broadcast packet is encoded once and shared.
- **The tick never waits.** No sleep, no blocking receive, no wall clock in a
  kernel. Disk, generation and decompression run on the task pool and the tick
  collects what is ready.
- **The empty tick has a budget.** Systems are gated by run conditions.
- Global mutable state has one writer in the main app and reaches dimension
  sub-apps as a read-only snapshot.
- A simulation partition is a **shard**, never a thread and never a "region". A
  shard step is a library call on a kernel without Bevy.

An exception to any of these is justified by a number in `PERF.md`.

## Ownership, locks, dispatch and allocation

The direction is #566. New library code in layers 1 to 4 follows it from the
first commit; existing code moves when it is touched for another reason. Never
add a new use of what is listed here.

- **No shared ownership.** No `Arc` or `Rc`. Data has one owner, and the owner
  lends it. Data that crosses a world or a thread is moved, or copied when its
  version changes.
- **No interior mutability or locks.** No `Mutex`, `RwLock`, `RefCell`, `Cell`,
  `OnceCell` holding mutable state, or atomics used as shared state.
- **No trait objects.** No `dyn` and no `Any`. A heterogeneous set is a generic
  parameter or an enum. A callback is a `fn` pointer or an id the host resolves.
- **No allocation on a hot path.** Scratch buffers belong to the caller. Borrow
  the input. An error is data, rendered to text at the edge. A hot path that
  claims zero allocations has a test with a counting allocator.
- **`no_std` with `alloc` where the dependencies allow it.** A new library crate
  starts as `#![no_std]` unless a dependency forbids it, and says which one.

Out of scope: code written against Bevy's or tokio's own API. When a rule cannot
be met, keep the use and state the reason next to it; do not work around the
rule with an equivalent in disguise.

## Rust style and comments

**Load `rust-style` before writing, splitting or reviewing Rust.** On a conflict
this file wins.

- **Prefer small crates.** A crate is one abstraction; its files sit flat in
  `src/`. `lib.rs` and every module with submodules hold only `mod` lines and
  re-exports. A file is named after its primary item.
- A bool parameter that reads as a bare `true` at the call site is an enum. A
  returned tuple that is not a pair by nature is a struct.
- **Default to zero comments.** Choose a name instead. A comment is for a
  non-obvious *why*, in one or two lines. No module-header essays, no restating
  the code, no story of the task; that belongs in the commit message.
- A deliberate shortcut with a known ceiling gets a `// chisle:` comment naming
  the ceiling and what lifts it.

## Working discipline

- The rules in this file outrank any generic "shortest diff" guidance. Minimal
  means no speculative code, never less than the data and the reference
  behaviour require.
- Do the item you were asked for. No rename and no cleanup outside it.
- Never delete or weaken a test to go green. Test when the change is done; if
  something fails, check that single test at `HEAD` to see whether you broke it.
- Report outcomes with the actual output. If something did not work, say so.
- Verify what a command or a test can decide yourself; do not ask the owner to.
- One way to do one thing. A migration ends with the old path deleted, in the PR
  that moves its last caller. At most three migrations are open at once.
- Commit messages are plain, domain-focused Conventional Commits. No task name
  and no private planning id in code, tests, comments or commits.

## Testing

**Load `efficient-testing` before you add, change or review a test.** Do not
write tests by default.

- **Run tests with `cargo nextest run -p <crate>`** for the crates you touched
  and their direct dependents. Do not use `cargo test`: it ignores
  `.config/nextest.toml`, and most crates set `doctest = false`. A Claude Code
  hook rejects it. The whole workspace runs once, as the final check before
  a push: `cargo nextest run --workspace`.
- Never run the same command twice without an edit in between.
- **Two tiers.** The default profile skips every test whose path contains
  `exhaustive::` and kills any test that runs past a minute. Sweeps live in a
  `mod exhaustive` and run with `cargo nextest run --profile full`. Every sweep
  keeps a small case in the default tier.
- **One integration-test binary per crate:** `tests/it/main.rs` plus modules.
- **One agent per worktree and target directory.**
- **A test must be able to fail because of our code.** No test that greps
  source text, restates a constant or a derive, exercises std/Bevy/serde,
  re-implements the formula under test, or repeats another test. A system under
  test is the production system.
- **Cases that differ only in data are one test over a table.**
- **Share expensive fixtures** through a `LazyLock`.
- An agent hook that runs on each command or edit stays under a few seconds.
  The `pre-push` checks are the one slow gate; do not add another without the
  owner's word.

## Agents and context

The cost of a call is the size of its context. Load `orchestrate` before you
start a second agent or continue one.

- One agent, one item, with the files, the check and the stop condition in the
  brief. The roles are `implementer`, `spec-author` and `reviewer`; Claude Code
  has them as agent types in `.claude/agents/`, and other tools take the role
  text from there into the brief.
- A new item goes to a new agent with a hand-off. Continue an agent only to
  answer its question.
- At 200k tokens of context, an agent reports and stops, and a main session
  writes a prompt for a new session (`handoff`). Do not resume a large context
  after an idle hour.
- Show the owner the crate map, the public names and the step list before
  implementers start in parallel.
- Review once per pull request, in a new agent (`review-pr`).

## Shipping

An issue or a pull request with no other instruction means the whole path to
`main`: fix, scoped tests, one review, the pull request, CI, the merge. Load
`ship` and follow it without stopping between the stages. Releases, tags and
version commits belong to the owner.
