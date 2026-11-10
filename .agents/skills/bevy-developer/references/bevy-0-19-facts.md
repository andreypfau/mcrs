# Bevy 0.19 Facts

Checked against the `bevy_ecs`, `bevy_app`, `bevy_time`, `bevy_state`,
`bevy_tasks`, `bevy_asset` and `bevy_diagnostic` 0.19.1 sources. When an answer
comes from memory of 0.13–0.16, check it against this table first.

## Gone or renamed

| Do not write | Write |
|---|---|
| `EventReader<E>` / `EventWriter<E>` / `add_event` | `MessageReader<M>` / `MessageWriter<M>` / `add_message` (messages arrived in 0.17) |
| `#[derive(Event)]` for a buffered stream | `#[derive(Message)]` |
| `on_event::<E>` | `on_message::<M>` |
| `Trigger<E>` | `On<E>` |
| `trigger.target()` | `trigger.entity` (the `EntityEvent` field), or `event_target()` |
| `commands.trigger_targets(ev, e)` | `#[derive(EntityEvent)]` + `commands.trigger(Ev { entity: e, .. })` or `commands.entity(e).trigger(..)` |
| `Replace` / `on_replace` / `OnReplace` | `Discard` / `on_discard` |
| `despawn_recursive()` / `despawn_descendants()` | `despawn()` (recursive through `linked_spawn`) / `despawn_children()` |
| `Parent` | `ChildOf` |
| `TransformSystem::TransformPropagate` | `TransformSystems::Propagate` |
| `bevy::ecs::system::CommandQueue` | `bevy::ecs::world::CommandQueue` |
| `bevy::ecs::component::ComponentHooks` | `bevy::ecs::lifecycle::ComponentHooks` |
| `Time::<Fixed>::set_max_delta` | `Time::<Virtual>::set_max_delta` |
| `OnTransition { from, to }` | `OnTransition { exited, entered }` |
| `ExecutorKind::SingleThreaded` | `schedule.set_executor(SingleThreadedExecutor::new())` |
| `.and(c)` / `.or(c)` / `.nand(c)` / `.nor(c)` on conditions | `.and_then(c)` / `.or_else(c)` / `.nand_then(c)` / `.nor_else(c)`, or `*_eager` (deprecated since 0.19.0) |
| `nondeterministic_system_order` | `ScheduleBuildSettings { ambiguity_detection: LogLevel::Error, .. }` |
| `query.single()` returning the item | `query.single()` returns `Result`; or use `Single<D, F>` |

## Behaviour that changed

- **Resources are components.** `trait Resource: Component`. Each resource lives
  on its own entity, marked with `IsResource`. `DefaultQueryFilters` filters only
  `Disabled`, so `Query<Entity>`, `Query<EntityRef>` and `Query<EntityMut>` see
  resource entities too. Add `Without<IsResource>`. A broad `EntityMut` query
  also conflicts with every `Res`/`ResMut` in the same system.
- **Inserting a resource component on a second entity** removes the new value
  with a warning. The original entity keeps the resource.
- **`NextState::set(s)` always transitions**, including when `s` equals the
  current state. `NextState::set_if_neq(s)` keeps the old no-op behaviour.
- **`Single<D, F>` and `Populated<D, F>`** skip the system when the query does
  not match. They do not panic and do not return an error. `Option<Single<..>>`
  runs with `None`.
- **Run conditions are read-only systems** (`SystemCondition` requires
  `ReadOnlySystem`) and are written without call parentheses:
  `run_if(on_message::<M>)`, `run_if(resource_exists::<R>)`.
- **Ambiguity detection defaults to `LogLevel::Ignore`.** Nothing reports an
  unordered conflicting pair unless you turn it on.
- **`Single<D>` requires `D: IterQueryData`**; nested queries use
  `SingleEntityQueryData`. `ExactSizeIterator` on a query iterator holds only
  with archetype filters (`With`, `Without`), not with `Changed`/`Added`.
- **`RemovedComponents<T>`** is message-backed and keeps entries for two
  updates.
- **`set_if_neq`** needs `PartialEq` on the target type. `map_unchanged` narrows
  a `Mut<T>` to one field without marking it changed.

## Small things that save a search

- `bevy_tasks::futures::check_ready(&mut task)` polls a task once without
  blocking. Dropping a `Task` cancels it; `.detach()` lets it finish unobserved.
- `TimeUpdateStrategy::FixedTimesteps(n)` makes each `App::update` run the fixed
  schedules exactly `n` times. `ManualDuration(d)` advances by `d`.
- `DiagnosticPath::const_new("a/b")` builds a path in a `const`.
  `app.register_diagnostic(Diagnostic::new(PATH))` registers it.
- `Commands::run_system(id)` runs a one-shot system registered with
  `register_system`; `run_system_cached(f)` registers on first use.
- `AssetLoader` requires `TypePath` on the loader type.
- `AppLabel` + `SubApp::new()` + `set_extract(|main, sub| ..)` +
  `app.insert_sub_app(label, sub)` builds a sub-app.
- `App::ignore_ambiguity(schedule, a, b)` and `.ambiguous_with(x)` document an
  accepted conflict.
