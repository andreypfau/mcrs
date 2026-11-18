# Item and Inventory Design

This document fixes the crate map and the rules of the item and inventory system. The behaviour of a stack is in `spec/item-stack.md`; this document refers to its
rules by id (`STK-*`) and does not state them again.

# 2. State classification

| Stored thing | Class | Owner or writer |
|-|-|-|
| `Kind` | authoritative truth, immutable for the life of the stack | the spawn command |
| `Count` | authoritative truth | a system (`Query<&mut Count>`), `consume`, the commit of a request, `give` |
| item component `T` | authoritative truth | a system (`Query<&mut T>`), `set`, `unset`, `reset` |
| `Removed` | authoritative truth | `set`, `unset`, `reset`, the spawn command |
| `InSlot` | authoritative truth of the placement | the placement commands |
| `Dropped` | authoritative truth of the placement | `drop_at`, `give_or_drop` |
| `Selected` | authoritative truth | the system that reads the input of the holder |
| `Slots` | materialized: the index from a slot number to the stack | the hooks of `InSlot` |
| occupancy mask | materialized: one bit for each slot of `Slots` | the hooks of `InSlot` |
| `Equipped` | materialized: the stack in the selected slot | one projection system |
| row hash | derived, computed at each read, not stored | none |
| patch of a stack entity | derived by `to_value` from the row and the prototype | none |
| prototype table | materialized from the data pack, immutable after the bake | the bake, one time for each data load |
| `Given`, `RequestResult` | temporal fact | the command that did the action |
| values of an unloaded container | authoritative truth while no entity exists | the container (later specification) |

Cost models of the materialized things:

- `Slots` and the occupancy mask make "first free slot" and "stack of slot n" one array read. A
  scan of all `InSlot` components is proportional to the population.
- `Equipped` lets a system filter with `With<Equipped>` at the archetype level. The selected slot
  changes at the rate of player input, so the structural change is rare.
- The prototype table makes each prototype value a borrow. A stack without a patch owns no heap
  data.

A stack has exactly one form at a time. A container that is not loaded stores values. Its entities
appear when it opens, or on the first tick that needs them, and the values go away.

# 3. Crate map

The layer numbers are those of `AGENTS.md`.

| Crate | Layer | Bevy | Owns | Depends on |
|-|-|-|-|-|
| `mcrs_minecraft_item_stack` | 3, data | feature `bevy`: derives only | `ItemStack`, `PatchEntry`, `Kind`, `Count`, one type for each component type, the codecs of the spec section 6 | wire, NBT, SNBT, text, registry ids |
| `mcrs_minecraft_item_table` | 3, data | feature `bevy`: one resource derive | `ItemTable`, `Prototype`; the bake from the data pack | `mcrs_minecraft_item_stack`, registries |
| `mcrs_minecraft_inventory_transaction` | 3, rules | none | the planner of an atomic request | nothing (`core` and `alloc`) |
| `mcrs_minecraft_engine_item` | 4, engine | required | the entity form, hooks, commands, read tools, the request command, `Equipped` | the three crates above, `bevy_ecs`, `bevy_app`, `bevy_math` |
| `mcrs_minecraft_recipe` | 3, data | none | `Ingredient`; the keys of the recipe serializers, the recipe book categories, the recipe displays and the slot displays | `mcrs_minecraft_item_stack`, registries, wire |
| `mcrs_minecraft_inventory_menu` | 3, data | none | the slot layout of each menu | `mcrs_minecraft_inventory_player`, `mcrs_minecraft_item_stack`, `mcrs_minecraft_menu_key` |
| `mcrs_minecraft_menu_key` | 3, data | none | `MenuType` and the keys of the menu registry | registries, resource |
| `mcrs_minecraft_item_key` | 3, data | none | the enum `Item` and the item tag keys | registries, resource |
| `mcrs_minecraft_potion` | 3, data | none | the enum `Potion` and the potion tag keys | registries, resource |
| `mcrs_minecraft_map_decoration` | 3, data | none | the enum `MapDecorationType` | registries |
| `mcrs_minecraft_banner_pattern` | 3, data | none | `BannerPattern`, its keys and its tag keys | resource |
| `mcrs_minecraft_decorated_pot_pattern` | 3, data | none | `DecoratedPotPattern` and its keys | resource |
| `mcrs_minecraft_block_transformer` | 3, data | none | `BlockTransformer` and its keys | block predicate, sound, registries, resource |
| `mcrs_minecraft_instrument` | 3, data | none | `Instrument<I>`, its keys and its tag keys | sound, text, resource |
| `mcrs_minecraft_jukebox_song` | 3, data | none | `JukeboxSong<I>` and its keys | sound, text, resource |
| `mcrs_minecraft_painting_variant` | 3, data | none | `PaintingVariant<I>`, its keys and its tag keys | text, resource |
| `mcrs_minecraft_trim` | 3, data | none | `TrimMaterial<I>`, `TrimPattern<I>` and their keys | text, resource |

- Each layer 3 crate starts as `#![no_std]` with `alloc`. If a dependency forbids it, the crate
  states which one.
- `I` is the type parameter of the text (`TextTypes` of the text crate). `mcrs_minecraft_item_stack`
  depends on the five crates from `mcrs_minecraft_banner_pattern` to `mcrs_minecraft_trim`.
- The protocol crates and the save crate depend on `mcrs_minecraft_item_stack` only.
- A proxy or a tool takes `mcrs_minecraft_item_stack` alone. A bot adds the table and the engine
  crate.
- The network id of a stack is state of one connection in the edge. No crate of this map stores it.
- The planner knows no edition. It gets slot ids and stack ids as type parameters.

# 5. Usage

Create, give and move. Each line is a command and applies at the next sync point:

```rust
commands.entity(player).give(ItemStack::new(SWORD, ONE).set(Damage(12)));
let icon = commands.spawn_item(SWORD).id();    // a stack without placement
commands.entity(stack).put_in(chest);          // merge, then the first free slot
commands.entity(stack).put_at(player, 4);      // one slot
commands.entity(stack).swap_with(other);
commands.entity(stack).drop_at(pos);
```

Read, and write in place. A system that also writes item components reads through `Effective`:

```rust
fn wear(mut tools: Query<(Entity, Effective<MaxDamage>, &mut Damage)>, mut commands: Commands) {
    for (stack, max, mut damage) in &mut tools {
        let Some(max) = max.get() else { continue };
        damage.0 += 1;
        if damage.0 >= max.0 {
            commands.entity(stack).consume(1);
        }
    }
}
```

Write by command, when the stack possibly does not store the component (section 6):

```rust
fn sign(mut stacks: Query<(Entity, Option<&mut Lore>), With<InSlot>>, mut commands: Commands) {
    for (stack, lore) in &mut stacks {
        match lore {
            Some(mut lore) => lore.lines.to_mut().push("signed".into()),
            None => {
                commands.entity(stack).set(Lore { lines: vec!["signed".into()].into() });
            }
        }
    }
}
```

A system that writes no item data, such as a network or save system, uses `Stacks`:

This example reacts to a change of the count only. A change of the placement or of a component
needs its own change source.

```rust
fn send_slots(mut stacks: Stacks, changed: Query<(Entity, &InSlot), Changed<Count>>) {
    for (stack, slot) in &changed {
        let Some(value) = stacks.to_value(stack) else { continue };
        // encode `value` for the connection of `slot.holder()`
    }
}
```

Atomic request with its result. The `source` comes from `app.allocate_request_source()` in the plugin build. A system that handles many holders reads
`MessageReader<RequestResult>` as an alternative to the observer:

```rust
let (from, to) = (SlotRef { holder: player, index: 0 }, SlotRef { holder: player, index: 9 });
let id = RequestId { source, serial: 7 };
commands.entity(player).request(Request::new(id).split(from, to, 16).swap(to, from));

fn on_result(result: On<RequestResult>) {          // app.add_observer(on_result)
    if let Err(rejected) = result.outcome {
        // send the slots of `result.holder` to its client again
    }
}
```

Custom component, and the value form without Bevy:

```rust
#[derive(Component, Clone, PartialEq, Hash)]
struct Soulbound(u64);

impl CustomComponent for Soulbound {
    const KEY: &'static str = "example:soulbound";
    fn encode(&self, bytes: &mut Vec<u8>) { bytes.extend(self.0.to_le_bytes()); }
    fn decode(bytes: &[u8]) -> Option<Self> { Some(Self(u64::from_le_bytes(bytes.try_into().ok()?))) }
}

app.insert_resource(table).add_plugins(ItemPlugin::default()).register_item_component::<Soulbound>()?;

let stack = ItemStack::new(kind, count).set(CustomName("Sting".into())).unset::<Lore>();
let bytes = encode_wire(Some(&stack))?;            // the codec names come with the codec step
```

# 6. Storage-class contract and the write rule

A stack entity stores each effective component (STK-PAT-002) as an ECS component, with two
exceptions:

1. A component whose value is the neutral value of its type is not stored.
2. A component that a patch removed is not stored. If its type has a neutral value, the removal
   is one bit in the component `Removed`.

Only a stack that has such a removal has `Removed`. That case is rare.

Types that can be absent because of exception 1, in the first batch: `Rarity`, `RepairCost`,
`TooltipDisplay`, `Lore`, `Enchantments`, `AttributeModifiers`. Each of these types implements
`Neutral`. The list for all 122 types (STK-TYP-001) comes from the game data and is an open point.
The list is part of the public contract: a type does not move in or out of it without a notice to
users.

Consequences that the user must know:

- `Query<&T>` and `Changed<T>` do not match a stack that has the neutral value of `T`.
- `Effective<T>`, `Stacks::get` and `effective` give the neutral value for such a stack.
- A stored value that is equal to the neutral value is legal. The merge check and the row hash
  treat it the same as an absent value.

**The write rule.** Write in place (`Query<&mut T>`) when the stack stores `T`. Use the command
`set` when the stack possibly does not store `T`. The two writes give the same result for the merge
check, the row hash and the value form. `set` on a stack that does not store `T` moves the entity
to a different archetype; this is the rare case.

Borrowed data. A heavy component holds `Cow<'static, _>`. A value that comes from the prototype
borrows from the prototype table. A value that a system writes is owned. The bake leaks the table,
so the borrow is `'static` and a stack without a patch owns no heap data. The ceiling is one table
in memory for each data load. Two designs lift the ceiling. In the first, the worlds of one load
borrow the table for a scoped lifetime. In the second, a generation counter frees a table when no
stack borrows from it.

Rejected shapes: slots that hold values with a projection to entities; immutable components with
a variant id; wrapper types `Interned<T>` or `Pool<T>` around a component.

# 7. Ownership and invariants

## 7.1 Writers

Section 2 names the writer of each component. These rules add to it:

- Placement and structure change through commands only. A system does not insert or remove
  `InSlot`, `Dropped`, `Removed`, `Kind` or `Equipped`.
- A stack has at most one placement. `InSlot` and `Dropped` are never together on one entity.
- `Count` is never zero. `consume` of the full count despawns the stack.
- A move keeps the entity. `put_in`, `put_at`, `swap_with`, `drop_at` and a `Swap` of a request
  change placement components only.
- A merge moves a count into the target and despawns a source that becomes empty. A split of a
  request spawns one new entity. These are the only cases where an entity ends or starts.
- The remainder of `give(stack_entity)` keeps the placement that it had. The remainder of
  `give(value)` is a new stack without placement, and `Given::remainder` names it. Its owner
  places it or despawns it.
- The cursor is a child entity of the player and is a holder with one slot. The slots of a player
  are on the player entity.
- Item components are registered in the plugin build only. A registration after the application
  starts is not possible.

A command applies at the next sync point. A system that must see the result runs after that point,
with an explicit order.

## 7.2 The hook contract of `InSlot`

`InSlot` is an immutable component. A change of the slot is a new insert. The hooks give these
guarantees:

1. After each command flush, `Slots::get(i)` of a holder is `Some(stack)` if and only if that
   stack has `InSlot` with that holder and the index `i`.
2. The occupancy mask has the bit `i` if and only if `Slots::get(i)` is `Some`.
3. An insert that names an occupied slot gives the slot to the new stack. The previous stack
   loses its `InSlot` and stays alive without placement.
4. An insert that names a slot that does not exist removes the `InSlot` again.
5. An insert of `InSlot` removes `Dropped`. An insert of `Dropped` removes `InSlot`.
6. The removal of `Slots` from a holder, and the despawn of a holder, despawn its stacks.
7. The despawn of a stack clears its slot.

Only the engine crate can construct `InSlot`. `Slots` has no method that writes a slot, but it is a
mutable component: a user system that replaces the value of a `Slots` in place breaks guarantee 1.
A holder gets new slots through an insert of `Slots`, never through an assignment.

## 7.3 What does not panic

| Input that can be wrong | Result |
|-|-|
| an entity that is not a stack, in a read tool | `None`, or `false` for `mergeable` |
| a kind that the table does not have | `ItemTable::prototype` gives `None`; the spawn command spawns nothing |
| a slot index out of range, a holder without `Slots` | `put_at` changes nothing; a request gives `Reason::NoSuchSlot` |
| a count of zero | `Count::new` gives `None` |
| a request with too many operations | `Reason::TooManyOps`; nothing applies |
| a command on a despawned entity | the command does nothing |
| bytes that do not decode | the codec gives `Err`; `CustomComponent::decode` gives `None` |
| a registration that cannot work | `RegisterError` |

An error is data. The edge renders it to text.

The engine crate reads untyped columns for the merge check, the row hash and `to_value`. These
reads use `unsafe`. Each block states its condition.
