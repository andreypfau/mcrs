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
| `.agents/rules/rendering.md` | `mcrs_minecraft_{render,render_controller,render_probe,entity_render,light_render,mesh,model,client,client_gui}`, `mcrs_molang_wgsl` |
| `.agents/rules/codec.md` | registries, NBT/SNBT, data pack and pack files, text, loot, environment, dimension, predicates, worldgen, protocol, wire, save, Anvil |
| `.agents/rules/simulation.md` | `mcrs_engine_*`, `*_engine*`, simulation, physics, brain, goals, replication, light engine, movement, server |
| `.agents/rules/memory.md` | chunk, volume, voxel, block, entity motion, light engine, heightmap, Anvil, engine store and world |

## Who you work for

Before you open a pull request, get the login of your user with
`gh api user --jq .login`.

- **A code owner** is a login in `.github/CODEOWNERS`. `CONTRIBUTING.md` does
  not apply to a code owner. A code owner can override any rule of this file
  for a task.
- **Any other user** is an outside contributor. Follow `CONTRIBUTING.md` before
  you write code. If the task breaks one of its rules, tell the user which rule
  and do not open the pull request. Typical cases: no issue, an issue that is
  taken, more than 1000 added lines, or a new crate or dependency without a
  published RFD.
  In the section "Models" of the pull request body, list your model and what
  you did, and each other model that took part.

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
  the specification, its RFD and the fixtures, never the reference. A
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
- A fact that the server and the client both hold is one component in the shared
  crate. The client writes that component from packets and input. A client crate
  adds only what the server has no use for: prediction state, last-sent values,
  interface state. A system that copies a client type into a shared component is
  a defect. `mcrs_minecraft_inventory_client` follows this rule with `Slots`.
- A simulation partition is a **shard**, never a thread and never a "region". A
  shard step is a library call on a kernel without Bevy.

An exception to any of these is justified by a measurement in the pull request. Put the measurement in the pull request body. Do not add a tracked file for it.

## Crossing a boundary

Each boundary has one mechanism. Do not add another.

- Main app to a dimension sub-app, and back: a field of `DimInput` and a field of
  `DimOutput` (`SubAppWorld<DimInput, DimOutput>`). Nothing else crosses.
- Main world to render world: a small `Copy` setting uses Bevy `ExtractResource`.
  A buffer moves in an extract system that takes `ResMut<MainWorld>`, as
  `extract_entities` does in `mcrs_minecraft_entity_render`.
- Background work to a world: a task that a system polls with `check_ready`, as
  `mcrs_minecraft_client_world` does for columns. A stage that runs inline or on a
  thread uses `Link` of `mcrs_engine_link`.
- A new `Arc<Mutex<..>>` queue between worlds is not allowed. A real cross-thread
  seam, such as a GPU map callback or a thread of another runtime, stays behind one
  named type with `#[expect(..., reason = "...")]`.

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
  parameter or an enum. A callback that the code stores is a `fn` pointer or an id the host resolves.
- **How a kernel reads the world.** A kernel takes `&W`, where `W` is bound by the
  smallest traits it uses. One trait covers one aspect. Only the methods that
  define the aspect are required; every other method has a default body that
  returns `None`, `false` or empty. The crate that owns the trait owns the one
  empty host. A closure that a caller passes for one call is `impl Fn`, never `&dyn`.
  `BodyView` and `NoBodies` in `mcrs_minecraft_physics` show a small trait with its
  one empty host.
- **No allocation on a hot path.** Scratch buffers belong to the caller. Borrow
  the input. An error is data, rendered to text at the edge. A hot path that
  claims zero allocations has a test with a counting allocator.
- **`no_std` with `alloc` where the dependencies allow it.** A new library crate
  starts as `#![no_std]` unless a dependency forbids it, and its commit message
  says which one.

Out of scope: code written against Bevy's or tokio's own API. When a rule cannot
be met, keep the use and state the reason in an `#[expect(..., reason = "...")]`
or in the commit message; do not work around the rule with an equivalent in
disguise.

## Rust style and comments

**Load `rust-style` before writing, splitting or reviewing Rust.** On a conflict
this file wins.

- **Prefer small crates.** A crate is one abstraction; its files sit flat in
  `src/`. `lib.rs` and every module with submodules hold only `mod` lines and
  re-exports. A file is named after its primary item.
- A bool parameter that reads as a bare `true` at the call site is an enum. A
  returned tuple that is not a pair by nature is a struct.
- **No comments outside `examples/`**, except `// SAFETY:`. The commit check of
  `xtask` (`cargo run -p xtask -- commit`) rejects a comment;
  `cargo run -p xtask -- strip-comments` removes them all.

## Working discipline

- The rules in this file outrank any generic "shortest diff" guidance. Minimal
  means no speculative code, never less than the data and the reference
  behaviour require.
- Do the item you were asked for. No rename and no cleanup outside it.
- One issue is one pull request, and its body closes the issue with
  `Closes #N`. A pull request into another branch closes nothing; it names the
  issue with `Refs #N`. An umbrella issue lists its steps as issues.
- No code before the issue's Decision is settled and its specification has no
  open question.
- A published RFD (`docs/rfd/0001-requests-for-discussion.md`) with the crate
  names, the layers and the migration path comes before the first code pull
  request of a new crate, dependency or subsystem.
- Split a pull request above about 3k added lines, generated data excluded, or
  across more than one crate group (crates of one domain prefix, such as
  `mcrs_minecraft_worldgen_*`). A draft never merges.
- A `refactor` pull request changes no behaviour.
- Never delete or weaken a test to go green. Test when the change is done; if
  something fails, check that single test at `HEAD` to see whether you broke it.
- Report outcomes with the actual output. If something did not work, say so.
- Verify what a command or a test can decide yourself; do not ask the owner to.
- One way to do one thing. A migration ends with the old path deleted, in the PR
  that moves its last caller. At most three migrations are open at once.
- An RFD that adds a second model beside a first one names the pull request
  that deletes the first.
- Before you write a helper, a value type or a test fixture, read `docs/crates.md`
  and search the workspace for it. Use the one that exists. When the canonical
  crate lacks it, add it there, never a local copy. Delete a duplicate that you
  find in code you touch in the same change.
- Commit messages are plain, domain-focused Conventional Commits. No task name
  and no private planning id in code, tests, comments or commits.

## Testing

**Load `efficient-testing` before you add, change or review a test.** Do not write tests by default.

- Run `cargo nextest run -p <crate>` for the crates you touched. Never use `cargo test`.
- Run `cargo nextest run --workspace` once, before a push.
- Sweeps live in a `mod exhaustive`; the default profile skips `exhaustive::` and `--profile full` runs it.
- One integration-test binary per crate: `tests/it/main.rs` plus modules.
- Every sweep keeps a small case in the default tier.
- **A test must be able to fail because of our code.** No test that greps
  source text, restates a constant or a derive, exercises std/Bevy/serde,
  re-implements the formula under test, or repeats another test. A system under
  test is the production system.
- **Cases that differ only in data are one test over a table.**
- **Share expensive fixtures** through a `LazyLock`.

## Agents and context

Load `orchestrate` before you start a second agent.

## Shipping

- Merge only with `scripts/agent/pr-land <pr> [<reviewed head sha>]`. It waits for the checks
  and the review bot, requires zero open review threads and squash merges. For a pull request
  from a fork whose workflows wait for approval, give the head SHA after you reviewed that
  head; the script approves the runs only for that head.
- A pull request that changes a crate `*client*`, `*render*`, `mesh` or `model` needs a run of
  the skill `run-client` before you report it as done.
- A release, a tag and a version commit belong to the owner.
