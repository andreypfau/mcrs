# ECS API Patterns (0.19)

What to store, and who owns each projection, is decided by `bevy-ecs`. This file
covers how to express the result with the 0.19 API.

## Components and resources

```rust
use bevy::prelude::*;

#[derive(Component, PartialEq, Clone, Copy)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

#[derive(Component, PartialEq, Clone, Copy)]
pub enum Pose {
    Standing,
    Crouching,
    Swimming,
}

#[derive(Resource, Default)]
pub struct TickCount(pub u64);
```

- `Pose` changes often and is mutually exclusive, so it is one value field, not
  three marker components.
- A marker (`#[derive(Component)] struct Spectator;`) is right when membership
  changes rarely and hot queries filter on it with `With`/`Without`, which skip
  whole archetypes.
- `#[component(storage = "SparseSet")]` makes insert and remove cheaper and
  iteration slower. It still moves the entity to another archetype, so it does
  not make a frequently toggled marker free.
- Derive `PartialEq` on anything written through `set_if_neq`.
- Since 0.19 a resource is a component on a singleton entity
  (`trait Resource: Component`). `Query<Entity>` and `Query<EntityRef>` also
  return resource entities; add `Without<IsResource>` to a broad query.

## Required components

```rust
#[derive(Component)]
#[require(Health { current: 20, max: 20 }, Pose::Standing)]
pub struct Player;
```

Use `#[require]` for an invariant that must hold whenever the component exists.
Spawning `Player` alone then gives a complete entity.

## Queries

| Form | Meaning |
|---|---|
| `Query<&T>` / `Query<&mut T>` | read / write `T` on every match |
| `Query<(&A, &mut B), With<C>>` | `With`/`Without` are archetype filters and skip whole tables |
| `Query<&T, Changed<T>>` | entities whose `T` changed since this system last ran; visits every candidate to check the tick |
| `Query<Ref<T>>` | read with `.is_changed()` / `.is_added()` without filtering |
| `Query<(Entity, Has<T>)>` | presence as a `bool`, no filtering |
| `Single<&T, With<Local>>` | exactly one match, or the system is skipped |
| `Option<Single<..>>` | zero or one match, never skipped |
| `Populated<&T>` | at least one match, or the system is skipped |
| `query.single()` | `Result`; handle it, never `unwrap` |
| `query.get(e)` / `get_mut(e)` | `Result` for a specific entity |
| `query.iter_many(ids)` | iterate a known list of entities, for example a work queue |

Two queries in one system that could alias the same component mutably are
rejected at startup with error B0001. Make them disjoint:

```rust
fn follow(
    target: Single<&Transform, With<Player>>,
    mut camera: Single<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    camera.translation = target.translation;
}
```

When disjointness is impossible, use `ParamSet<(Query<..>, Query<..>)>` and
access one member at a time.

## Commands

`Commands` record structural changes (spawn, despawn, insert, remove, trigger)
and apply them at the next sync point.

```rust
fn despawn_expired(mut commands: Commands, q: Query<(Entity, &Lifetime)>, tick: Res<TickCount>) {
    for (entity, life) in &q {
        if life.ends_at <= tick.0 {
            commands.entity(entity).despawn();
        }
    }
}
```

- `despawn()` also despawns descendants through `linked_spawn` relationships
  such as `Children`. To remove only the children, use `despawn_children()` or
  `despawn_related::<Children>()`. `despawn_descendants` no longer exists.
- Insert several components in one call, `insert((A, B, C))`, to move the entity
  once instead of three times.
- `ParallelCommands` record commands from inside `par_iter`.
- A command that needs `&mut World` is an `EntityCommand` or `Command` queued
  with `commands.queue(..)`. It runs at the sync point, so it does not block
  other systems the way an exclusive system does.

## Messages: buffered temporal facts

```rust
#[derive(Message)]
pub struct ItemPickedUp {
    pub item: Entity,
    pub by: Entity,
}

app.add_message::<ItemPickedUp>();

fn pick_up(mut out: MessageWriter<ItemPickedUp>, /* ... */) {
    out.write(ItemPickedUp { item, by: player });
}

fn play_pickup_sound(mut picked: MessageReader<ItemPickedUp>) {
    for msg in picked.read() {
        // ...
    }
}
```

- `MessageReader`/`MessageWriter` replace `EventReader`/`EventWriter`, which do
  not exist in 0.19.
- Messages are double-buffered and live for two `Messages::update` calls. A
  reader that does not run in that window loses them.
- A message is right for a temporal fact ("X picked up this item") and for a
  work queue of touched ids. It is wrong for "recompute something you can read
  from current state".

## Observers: immediate reactions

```rust
#[derive(EntityEvent)]
pub struct Damaged {
    pub entity: Entity,
    pub amount: i32,
}

fn on_damaged(damaged: On<Damaged>, mut q: Query<&mut Health>) {
    if let Ok(mut hp) = q.get_mut(damaged.entity) {
        hp.current = (hp.current - damaged.amount).max(0);
    }
}

app.add_observer(on_damaged);
commands.trigger(Damaged { entity, amount: 3 });
```

- `#[derive(Event)]` is a global event; `#[derive(EntityEvent)]` targets an
  entity named by a field called `entity` or marked `#[event_target]`. Only an
  `EntityEvent` reaches observers attached to that entity
  (`commands.entity(e).observe(..)`).
- `On<E>` dereferences to `E`. Methods on `On`: `event()`, `observer()`,
  `original_event_target()`, `propagate(bool)`. `trigger.target()` and
  `commands.trigger_targets` are gone.
- `world.trigger(..)` runs observers immediately. `commands.trigger(..)` runs
  them when the command queue is applied.
- Lifecycle observers: `On<Add, T>`, `On<Insert, T>`, `On<Discard, T>`,
  `On<Remove, T>`, `On<Despawn, T>` (`bevy_ecs::lifecycle`). `Discard` was
  `Replace` before 0.19. Component hooks (`on_add`, `on_insert`, `on_discard`,
  `on_remove`, `on_despawn`) fire on the same structural events. Nothing fires
  on a mutation through `&mut T`.
- An observer runs once per trigger. Thousands of triggers per frame cost
  thousands of observer runs; a message read by one system is cheaper there.

## Change detection

- `Changed<T>` is true after any `&mut T` deref, even when the value is equal.
  Write with `set_if_neq` or compare first.
- To write one field without touching the others' change state, narrow the
  `Mut` with `map_unchanged(|v| &mut v.field)` and call `set_if_neq` on it.
- `Changed<T>` checks a tick per candidate entity. Over a large population with
  few changes, have the producer write the touched ids into a `Message` or a
  dirty set and iterate those with `iter_many`.
- `RemovedComponents<T>` is backed by messages and keeps entries for two
  updates. A consumer that runs less often misses removals; prefer an
  `On<Remove, T>` observer or a message written by the remover.
- `ResMut::set_if_neq` and `Res::is_changed()` behave the same for resources.

## Relationships

```rust
#[derive(Component)]
#[relationship(relationship_target = Members)]
pub struct MemberOf(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = MemberOf)]
pub struct Members(Vec<Entity>);
```

- Bevy maintains the target side (`Members`) when `MemberOf` is inserted,
  replaced or removed. A hand-maintained reverse index with its own update
  system is the duplicate that this replaces.
- `ChildOf`/`Children` is the built-in hierarchy. `children![..]` and
  `related!(..)` spawn related entities in one bundle.
- `linked_spawn` on the target makes despawning the source despawn the related
  entities.
- A relationship is a structural change on every edit. A link that changes every
  frame (for example "the grid cell this entity is in") belongs in a value plus an
  index with one owner, not a relationship.

## SystemParam

```rust
use bevy_ecs::system::SystemParam;

#[derive(SystemParam)]
pub struct Sessions<'w, 's> {
    by_id: Res<'w, SessionIndex>,
    sessions: Query<'w, 's, &'static Session>,
}
```

Group parameters that always travel together. The scheduler sees the fields
exactly as if they were written inline.

## Anti-patterns

- `query.single().unwrap()`.
- A frequently flipped state stored as markers, or as several `bool`s that
  encode one mutually exclusive state.
- A `HashMap<_, Entity>` kept beside a query when an index resource with one
  owner, `EntityHashMap`, or a relationship already answers the lookup.
- `query.iter().find(|x| x.id == id)` per incoming message when an id → entity
  index exists.
- `Mutex` inside a system. Shared mutable state has one writer; others send it
  messages.
