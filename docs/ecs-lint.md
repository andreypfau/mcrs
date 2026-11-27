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
| Diagnostics | real rustc lints in the tool namespace `mcrs`, at level `deny`; `#[expect]` with a reason suppresses one | §5 |
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
- The `push` trigger of `xtask` runs the check `ecs-lint`, so the pre-push hook runs the lints
  before a push. The `commit` trigger does not run it.
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
  condition and which set types are children of other sets, in which schedule. A run
  condition on a set gates the set only in that schedule.

From these facts, each leaf gets a resolved state:

- the schedule label;
- the run condition: on the leaf, on an ancestor node, on a set of the leaf, or on a parent
  set of that set;
- the parameter types of the system, from `fn_sig` of its `DefId`. A foreign `DefId` has its
  signature in the crate metadata, so a facts file is not necessary for signatures.

A leaf that the model does not resolve gets no rule. With `MCRS_LINT_DEBUG` set, the pass
writes a note at each such leaf, so that the coverage of the model is visible.

## 4. Facts files

A facts file holds only the facts that a dependent crate cannot read from metadata:

- the helpers of the crate;
- the set configurations of the crate;
- the components that each function of the crate writes through `Mut`;
- the components that an ungated tick system of the crate writes on each run;
- the run conditions of the crate that have the attribute `mcrs::event_condition` (§10).

The facts file is next to the metadata file of the crate, with the extension
`.mcrs-lint.json`. The pass finds the files of its dependencies through the paths of their
metadata files. Thus each profile, target and target directory has its own facts. A crate
with no such facts has no facts file. Items are keyed by their
`DefPathHash`, which is the same in every crate.

Cargo compiles a dependency before its dependents, so the facts of a dependency exist when a
dependent crate compiles. A change in a dependency changes its metadata, and cargo compiles
the dependents again.

A set that gets its run condition in a crate that is not a dependency is not visible. Such a
case needs an `#[expect]` with a reason, or a later workspace pass.

## 5. Rules

Each rule is a lint with the name `mcrs::<rule>` and the level `deny`. A lint is emitted at the
`HirId` of the item or of the registration, so `#[expect]` on that item applies. Fix the code
when the fix keeps the behaviour. Otherwise, add an `#[expect]` with a reason that states why the
code is correct. Rust does not accept an attribute on an expression statement. Put the `#[expect]` of an
expression lint on a `let` statement or on the enclosing function.

```rust
#[cfg_attr(mcrs_lint, expect(mcrs::exclusive_system, reason = "drives the dimension sub-apps"))]
fn tick_dimensions(world: &mut World) { /* ... */ }
```

An `#[expect]` of `alloc_in_tick` also states the number of allocation sites of the system. The
number goes in a tool attribute next to the expectation:

```rust
#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "one buffer for each chunk"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(2))]
fn mesh_chunks(chunks: Query<&Chunk>) { /* ... */ }
```

- The driver registers the tool `mcrs` for attributes with `-Zcrate-attr`, so a crate needs no
  `register_tool` of its own.
- The lint `alloc_site_count` is an error when the number differs from the number of sites, or
  when the expectation has no `alloc_sites` attribute. The message reads "system X allocates
  at N sites, the expectation states M", and it labels each site. `#[expect]` of
  `alloc_in_tick` does not suppress `alloc_site_count`.
- Several sites in one macro call count as one site.
- A new allocation in a system with an expectation changes the number. The author of the change
  reads the sites and updates the number.

Rules on systems. A "tick system" is a system in a tick schedule. "Reach" is the body of the
system, its closures, and the local functions that it calls, at any depth. An `async` block and a closure in `observe` or `add_observer` are not in the reach. A task
pool or an event runs them outside the system.

| Lint | Rule | Check |
|------|------|-------|
| `exclusive_system` | "No system takes `&mut World`" | a registered system with a `&mut World` parameter at any position |
| `exclusive_observer` | Bevy panics on an exclusive observer | an observer with a `&mut World` parameter, from `add_observer` or `observe` |
| `ungated_system` | "Systems are gated by run conditions" | a tick system with no run condition, and with no `Single`, `Populated`, `If` or `PopulatedMessageReader` parameter |
| `blocking_in_system` | "The tick never waits" | the reach of a tick system calls `Instant::now`, `SystemTime::now`, `thread::sleep`, a `std::fs` function, a `File` or `TcpStream` method, `Receiver::recv`, `JoinHandle::join` without an `is_finished` check, or a `block_on` |
| `alloc_in_tick` | "No allocation in a steady-state tick" | the reach of a tick system has a call that returns `Vec`, `String`, `Box`, a `std`, `alloc`, `hashbrown` or `indexmap` collection; `mem::take`, `mem::replace`, `mem::swap` and a `new` or `default` with no arguments do not count. A `let` binding that holds a `new` or `default` of such a type is a site when the reach grows it with `push`, `extend`, `insert`, `push_str`, `append`, `resize` or `reserve`, or passes `&mut` of it to a call; the site is the `let` initializer. A move of the binding by value into an assignment, a struct field, a `return` or an argument ends the tracking. A site inside an external macro, for example `format!`, is reported at the macro call. A system with a run condition that fires on an event (§10) is not checked. A call on an event path does not count: in a `for` loop over messages, removed components or a query with `Changed` or `Added`, in an `if` on them, or in an `if` on `is_changed` or `is_added` |
| `invalid_event_condition` | `mcrs::event_condition` marks a run condition (§10) | the attribute is on an item that is not a function, on a function that does not return `bool`, or on a private function that no `run_if` uses |
| `alloc_site_count` | An expectation states what it covers | a system with `#[expect(mcrs::alloc_in_tick)]` has another number of sites than its `mcrs::alloc_sites(N)` states, or has no such attribute |
| `per_frame_insert_resource` | Do not insert a resource on each run | the body of a tick system calls `insert_resource` outside an `if`, a `match`, a loop or a comparing early exit, and no run condition of the system fires on an event (§10) |
| `marker_toggle` | "State that flips often is a value, not a marker" | the body of a tick system inserts and removes the same unit component |
| `time_clock_mismatch` | Read the clock of the schedule | a system in a `Fixed*` schedule reads `Res<Time<Real>>` or `Res<Time<Virtual>>` |
| `always_changed_filter` | "`Changed<T>` still visits every candidate" | a `Changed<T>` in a query, when a tick system with no run condition dereferences `Mut<T>` mutably outside an `if` or a `match`; the facts files carry the written components to the dependent crates |

Rules on any function:

| Lint | Rule | Check |
|------|------|-------|
| `missed_set_if_neq` | "Write through `set_if_neq`" | an assignment through `Mut`, `ResMut` or `NonSendMut` of a `PartialEq` value, outside a guard. A guard is an `if` with a comparison, a `!`, `is_none` or `is_some` in its condition, or such an `if` that ends with `return`, `continue` or `break` before the assignment. `x = !x`, `x = x + n`, `x = x - n` and `wrapping_add` or `wrapping_sub` with a literal that is not zero, and a write after `mem::take`, `mem::replace` or `mem::swap` of the same place are not checked |
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

Integration tests and benchmarks are not checked for the rules on systems and for
`missed_set_if_neq`: their schedules do not run in the game tick. Cargo sets
`CARGO_TARGET_TMPDIR` only for these targets, and the driver reads it. The other rules apply to
all targets. The same holds for code under `#[cfg(test)]`: rustc keeps a trace of the `cfg` in
the HIR, and the driver reads it. A registration under `#[cfg(test)]` does not count as a use of the system. Thus a test app
that runs a system without its run condition does not make the system ungated.

## 6. Matching of Bevy items

The pass keeps a table of anchors: public paths of Bevy items, for example
`bevy_ecs::system::Query`. At the start of a crate that depends on `bevy_ecs`, the pass
resolves each anchor to a `DefId` through the public module tree. Then it compares `DefId`s.
An anchor that does not resolve is an error, so a Bevy upgrade cannot make a rule stop
without a message.

## 7. CI

The `lint` job runs the check `ecs-lint` of `xtask`:

1. It installs the toolchain of `lints/rust-toolchain.toml` and builds the driver.
2. It runs `cargo check --all-targets` in `lints/fixture`.
3. It runs `cargo check --workspace --all-targets` with `RUSTC_WORKSPACE_WRAPPER` set to the
   driver.

Steps 2 and 3 use `build.warnings = "deny"`, so an unfulfilled expectation also fails. Step 3
does not use `scripts/rustc-wrapper`: a different wrapper path changes the fingerprint of the
workspace crates. Thus cargo checks the crates again, also when the build cache has results
from a compilation without the driver. A new rule comes with its fixes or expectations in the
same pull request.

## 8. Use

```sh
cargo build --release --manifest-path lints/Cargo.toml --target-dir lints/target   # once, and after each change of lints/
cargo check --workspace                                  # the lints run as part of the check
MCRS_LINT_DEBUG=1 cargo check -p <crate>                 # shows each configuration that the model does not resolve
```

- `[env] MCRS_LINT_TICK_SCHEDULES` in `.cargo/config.toml` lists the project tick schedules.
  The driver records this variable and `MCRS_LINT_DEBUG` in the dependency information, so a
  new value checks the crates again.
- `lints/fixture` is the test of the linter. Each expected lint has an `#[expect]`, so
  `cargo check --all-targets --config 'build.warnings="deny"'` in `lints/fixture` fails on a missing lint and on an
  extra lint. The fixture has two crates, so it also tests the facts files.

## 9. Cost

Measured on the workspace (207 crates, Apple M-series). The pass takes 0.9 s of CPU time for
all crates together. The parts are 0.5 s for the model of registrations, 0.1 s for the scan of
bodies, 0.2 s for the rules on systems, 0.1 s for the rules on signatures. A cold check of the
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
- The pass does not find a helper in a trait when the impl is in another crate than the trait.
The pass selects calls by the crate of the callee.
- A helper argument with its own configuration, for example `helper(app, f.run_if(c))`, is
  not resolved.
- A run condition that a crate outside the dependencies of the registration adds to a set is
  not visible (§4).
- Observers are checked only for `exclusive_observer`. The rules on tick systems do not apply
  to observers.
- The reach of a system does not include functions of other crates, trait methods or calls
  through a function pointer.
- `missed_set_if_neq` accepts a write after any guard that compares, also when the comparison
  is about another value.
- `alloc_in_tick` and `per_frame_insert_resource` know the Bevy run conditions that fire on
  an event: `on_message`, `resource_changed`, `resource_added`, `resource_removed`,
  `resource_exists_and_changed`, `resource_changed_or_removed`, `run_once`,
  `any_component_removed`, `state_changed`, `input_just_pressed`, `input_just_released`,
  `any_match_filter` with a `Changed` or `Added` filter, and `not(resource_exists)`. A `Single`
  or `Populated` parameter with a `Changed` or `Added` filter also counts as such a condition.
- A project run condition is an event condition when its function has the attribute
  `#[cfg_attr(mcrs_lint, mcrs::event_condition)]`. Mark a function only when it returns `true`
  on a change, on an event or on pending work that the gated system consumes. Do not mark a
  condition that is `true` on every frame for a long time.
- The facts file of a crate lists its marked functions, so a system in another crate that uses
  the condition is also an event system.
- A marked function must return `bool`. The lint `invalid_event_condition` is an error when it
  does not. It is also an error when a private marked function is not an argument of a `run_if`
  of its crate. A `pub` marked function has no such check, because a dependent crate can use it.
- Combinators follow these rules: `a.and_then(b)` and `a.and_eager(b)` are events when `a` or `b`
  is an event. `a.or_else(b)` and `a.or_eager(b)` are events when `a` and `b` are events.
  `not(a)` is an event only for `resource_exists`; `not` of a marked function is not an event.
- A condition that is a closure, or a function that returns a condition, is not an event
  condition, also when it calls a marked function.
- An `if` on `is_changed` or `is_added` of a Bevy value, or on `Ref::is_changed`, is an event
  path in the body of a system. A negated check is an event path when the code after an early
  `return`, `continue` or `break` on it runs. `a && b` is an event path when `a` or `b` is a
  change check. `a || b` is an event path when `a` and `b` are change checks.
- `always_changed_filter` sees a writer only in the crate that registers it or in a
  dependency of the crate of the filter. A loop is not a guard; an `if` or a `match` is.
- Without the driver, `cfg_attr(mcrs_lint, expect(mcrs::too_many_arguments, ..))` is not
  compiled and nothing checks the number of parameters.
- After the first build of the driver, cargo does not check again a crate that it checked
  without the driver. Touch the crate, or run `cargo clean -p <crate>`, to see its lints.
- `.cargo/config.toml` names `scripts/rustc-wrapper`, a shell script. Cargo cannot run it on
  Windows. The project builds on Unix hosts only.
- `scripts/rustc-wrapper` looks for the driver in `lints/target`. Build the driver with
  `--target-dir lints/target`, also when `CARGO_TARGET_DIR` is set.
