# ECS lint: architecture

Design note for `mcrs_lint`, a rustc driver that checks the ECS rules of `AGENTS.md` and of
the `bevy-developer` and `bevy-ecs` skills at compile time. Clippy and `xtask` cannot check
these rules: the rules need the types of system parameters and the schedule of each system.

## 0. Decisions

| Question | Decision | Section |
|----------|----------|---------|
| Engine | a rustc driver with late lint passes, on the toolchain of `rust-toolchain.toml` | §2 |
| Existing linters | not used. `bevy_lint` pins another nightly and has no model of the schedule | §1 |
| Unit of analysis | one model of registrations per crate, built once; each rule reads the model | §3 |
| Facts across crates | each crate writes a facts file; a crate reads the facts files of its dependencies | §4 |
| Diagnostics | real rustc lints in the tool namespace `mcrs`; `#[expect]` with a reason suppresses one | §5 |
| Matching of Bevy items | definition paths, checked at start: a missing anchor is an error | §6 |

## 1. Why a new linter

`bevy_lint` (TheBevyFlock/bevy_cli) is the only maintained Bevy linter. It does not fit:

- It compiles the project with its own pinned nightly. The project uses another nightly, so
  every dependency compiles twice, and a feature newer than that nightly does not compile.
- Each lint finds systems with its own walk over `add_systems`. The walk does not see a system
  that a helper function registers, and it does not know the run conditions of a set.
- Each lint sees one crate. A set gets its run condition in one crate and its systems in
  another crate.
- A Bevy release that moves a type makes a lint stop silently.

## 2. Process

```text
cargo check (or build, test, rust-analyzer)
  .cargo/config.toml: build.rustc-workspace-wrapper = scripts/rustc-wrapper
    scripts/rustc-wrapper, once per workspace crate
      lints/target/release/mcrs-lint-driver if it exists, else rustc
        rustc + the lint pass of mcrs_lint
          reads  <out>/lib<dependency>-<hash>.mcrs-lint.json
          emits  lints
          writes <out>/lib<crate>-<hash>.mcrs-lint.json
```

- The lints are part of every cargo command that compiles a workspace crate. There is no
  separate lint command.
- `build.rustc-workspace-wrapper` applies only to workspace members. Dependencies outside the
  workspace compile with the normal rustc and keep their cache.
- `scripts/rustc-wrapper` runs plain rustc when the driver is not built. A checkout without
  `rustc-dev` compiles as before, without the lints.
- `cargo clippy` sets its own workspace wrapper, so clippy runs without these lints. CI runs
  `cargo check` and `cargo clippy`.
- The driver is linked with an rpath to the sysroot of its toolchain. It compares its build
  toolchain with `RUSTUP_TOOLCHAIN`. On a difference, it writes a warning and runs plain rustc,
  so a toolchain update never mixes two compilers.
- The driver adds itself to the dependency information of each crate. Cargo checks the
  workspace crates again after a new driver build, and only then.
- The driver adds `--cfg mcrs_lint` and registers the tool `mcrs`. Without the driver,
  the attributes in `cfg_attr(mcrs_lint, ...)` are not compiled.
- `lints/` is a separate workspace. The main workspace does not need `rustc-dev`.
  `lints/rust-toolchain.toml` names the same channel and adds `rustc-dev` and `llvm-tools`.

## 3. Model of registrations

The pass walks every body of the crate once. It records these facts:

- **Registration.** A call to `add_systems` on `App`, `SubApp` or `Schedule`. It records the
  schedule label type, the span and the tree of the configuration argument.
- **Configuration tree.** Each node is a system, a tuple, or a configuration method
  (`run_if`, `in_set`, `before`, `after`, `chain`, `ambiguous_with`, and the others) on a
  child node. A leaf is a function path, a closure, or a generic parameter of the enclosing
  function.
- **Helper.** A function that passes its generic parameter into the configuration argument of
  a registration. The helper records the parameter index and the schedule. A call of the
  helper is a registration of its argument.
- **Set configuration.** A call to `configure_sets`. It records which set types get a run
  condition and which set types are children of other sets.

From these facts, each leaf gets a resolved state:

- the schedule label;
- the run condition: on the leaf, on an ancestor node, on a set of the leaf, or on a parent
  set of that set;
- the parameter types of the system, from `fn_sig` of its `DefId`. A foreign `DefId` has its
  signature in the crate metadata, so a facts file is not necessary for signatures.

A leaf that is a generic parameter and that no helper resolves is counted as unresolved. The
report shows the count, so that the coverage of the model is visible.

## 4. Facts files

A facts file holds only the facts that a dependent crate cannot read from metadata:

- the helpers of the crate;
- the set configurations of the crate.

The facts file is next to the metadata file of the crate, with the extension
`.mcrs-lint.json`. The pass finds the files of its dependencies through the paths of their
metadata files. Thus each profile, target and target directory has its own facts. A crate
with no helpers and no set configurations has no facts file. Items are keyed by their
`DefPathHash`, which is the same in every crate.

Cargo compiles a dependency before its dependents, so the facts of a dependency exist when a
dependent crate compiles. A change in a dependency changes its metadata, and cargo compiles
the dependents again.

A set that gets its run condition in a crate that is not a dependency is not visible. Such a
case needs an `#[expect]` with a reason, or a later workspace pass.

## 5. Rules

Each rule is a lint with the name `mcrs::<rule>`. A lint is emitted at the `HirId` of the item
or of the registration, so `#[expect]` on that item applies.

```rust
#[cfg_attr(mcrs_lint, expect(mcrs::exclusive_system, reason = "drives the dimension sub-apps"))]
fn tick_dimensions(world: &mut World) { /* ... */ }
```

Rules on systems. A "tick system" is a system in a tick schedule. "Reach" is the body of the
system, its closures, and the local functions that it calls, at any depth.

| Lint | Rule | Check |
|------|------|-------|
| `exclusive_system` | "No system takes `&mut World`" | a registered system with a `&mut World` parameter at any position |
| `exclusive_observer` | Bevy panics on an exclusive observer | an observer with a `&mut World` parameter, from `add_observer` or `observe` |
| `ungated_system` | "Systems are gated by run conditions" | a tick system with no run condition, and with no `Single`, `Populated` or `If` parameter |
| `blocking_in_system` | "The tick never waits" | the reach of a tick system calls `Instant::now`, `SystemTime::now`, `thread::sleep`, a `std::fs` function, a `File` or `TcpStream` method, `Receiver::recv`, `JoinHandle::join` or a `block_on` |
| `alloc_in_tick` | "No allocation in a steady-state tick" | the reach of a tick system has a call that returns `Vec`, `String`, `Box`, a `std`, `alloc`, `hashbrown` or `indexmap` collection; `mem::take`, `mem::replace` and `mem::swap` do not count |
| `per_frame_insert_resource` | Do not insert a resource on each run | the body of a tick system calls `insert_resource` outside an `if`, a `match` or a loop |
| `marker_toggle` | "State that flips often is a value, not a marker" | the body of a tick system inserts and removes the same unit component |
| `time_clock_mismatch` | Read the clock of the schedule | a system in a `Fixed*` schedule reads `Res<Time<Real>>` or `Res<Time<Virtual>>` |
| `always_changed_filter` | "`Changed<T>` still visits every candidate" | a `Changed<T>` in a query, when a tick system with no run condition dereferences `Mut<T>` mutably outside an `if` or a `match`; the facts files carry the written components to the dependent crates |

Rules on any function:

| Lint | Rule | Check |
|------|------|-------|
| `missed_set_if_neq` | "Write through `set_if_neq`" | an assignment through `Mut`, `ResMut` or `NonSendMut` of a `PartialEq` value, outside an `if` that compares with `==` or `!=` |
| `borrowed_reborrowable` | Pass a reborrow, not `&mut` | a parameter `&mut` of `Commands`, `EntityCommands`, `Query`, `DeferredWorld`, `EntityMut`, `Mut`, `ResMut` or `NonSendMut`, if the return type does not use its lifetime; `self` and trait methods are not checked |
| `zst_query` | Filter on a marker | `Query` data that fetches `&T` of a unit component |
| `insert_message_resource` | Register messages with `add_message` | `init_resource` or `insert_resource` of `Messages<T>` |
| `unit_in_bundle` | A bundle inserts components | a `()` in a type argument that has a `Bundle` bound |
| `too_many_arguments` | At most seven parameters | a function or a trait method with more than seven parameters that do not implement `SystemParam`; it replaces `clippy::too_many_arguments`, which the workspace allows. It runs in every crate, also in a crate without Bevy |

The tick schedules are the schedules of the Bevy main loop (`First` to `Last`, the fixed
schedules, `ExtractSchedule`, `Render`) and the schedules in
`[env] MCRS_LINT_TICK_SCHEDULES`. Other schedules, for example `Startup` or a schedule that a
system runs on demand, are not checked.

`Time<Fixed>` in a frame schedule is not a mismatch: interpolation reads the overstep of the
fixed clock there.

## 6. Matching of Bevy items

The pass compares the definition path of a `DefId` (for example
`bevy_ecs::system::query::Query`) with a table of anchors. At the start of a crate that
depends on `bevy_ecs`, the pass resolves every anchor. An anchor that does not resolve is an
error, so a Bevy upgrade cannot make a rule stop without a message.

## 7. Migration path

1. The driver, the model, `exclusive_system` and `ungated_system`. The existing violations get
   an `#[expect]` with a reason or a fix.
2. CI builds the driver before `cargo check`, so that a missing driver cannot skip the lints.
3. Each next rule comes with its fixes or expectations, one rule per pull request.

## 8. Use

```sh
cargo build --release --manifest-path lints/Cargo.toml   # once, and after each change of lints/
cargo check --workspace                                  # the lints run as part of the check
MCRS_LINT_DEBUG=1 cargo check -p <crate>                 # shows each configuration that the model does not resolve
```

- `[env] MCRS_LINT_TICK_SCHEDULES` in `.cargo/config.toml` lists the project tick schedules.
  The driver records this variable and `MCRS_LINT_DEBUG` in the dependency information, so a
  new value checks the crates again.
- `lints/fixture` is the test of the linter. Each expected lint has an `#[expect]`, so
  `RUSTFLAGS="-D warnings" cargo check` in `lints/fixture` fails on a missing lint and on an
  extra lint. The fixture has two crates, so it also tests the facts files.

## 9. Cost

Measured on the workspace (207 crates, Apple M-series). The pass takes 0.9 s of CPU time for
all crates together: 0.5 s for the model of registrations, 0.1 s for the scan of bodies,
0.2 s for the rules on systems, 0.1 s for the rules on signatures. A cold check of the
workspace crates takes 15 to 18 s with and without the driver.

| Case | Without the driver | With the driver |
|------|--------------------|-----------------|
| One edit in `mcrs_minecraft_engine` | 2.1 s | 2.3 s |
| No change | 0.6 s | 0.6 s |

The model looks at a call only when the callee is in the local crate or in a crate with
helpers. Other calls (`Iterator::map`, `Clone::clone` and the others) cannot forward a system,
so the model does not resolve them.

## 10. Known limits

- A configuration that is not a function, a closure, a generic parameter or an `impl Trait`
  return is not resolved. Examples: a `ScheduleConfigs` value from a function, a system in a
  local variable of type `ScheduleConfigs`.
- A helper in a trait whose impl is in another crate than the trait is not found, because the
  pass selects calls by the crate of the callee.
- A helper argument with its own configuration, for example `helper(app, f.run_if(c))`, is
  not resolved.
- A run condition that a crate outside the dependencies of the registration adds to a set is
  not visible (§4).
- Observers are checked only for `exclusive_observer`. The rules on tick systems do not apply
  to observers.
- The reach of a system does not include functions of other crates, trait methods or calls
  through a function pointer.
- `missed_set_if_neq` accepts a write inside any `if` that compares, also when the comparison
  is about another value.
- `always_changed_filter` sees a writer only in the crate that registers it or in a
  dependency of the crate of the filter. A loop is not a guard; an `if` or a `match` is.
- Without the driver, `cfg_attr(mcrs_lint, expect(mcrs::too_many_arguments, ..))` is not
  compiled and nothing checks the number of parameters.
- The set graph does not separate schedules: a run condition on a set in one schedule gates
  the set in all schedules.
