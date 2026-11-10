# Item and Inventory Design

This document fixes the public API and the crate map of the item and inventory system. It contains
no implementation. The behaviour of a stack is in `spec/item-stack.md`; this document refers to its
rules by id (`STK-*`) and does not state them again.

A line that has the mark **(proposal)** is a choice of this document. All other choices are
decisions of the owner and are fixed.

# 1. Purpose and scope

The system gives one model of an item stack to the server, the client, the protocol, the save
format and a user without Bevy. The model has two forms:

- **Value form**: the type `ItemStack`: a kind, a count and a patch. It has no Bevy type.
- **Entity form**: one entity for each stack. The kind, the count and each item component are ECS
  components.
- **Holder**: an entity that has the component `Slots`: a player, a cursor, a chest, a hopper.
- **Placement**: where a stack is: a slot of a holder (`InSlot`), the world (`Dropped`), or nowhere.
- **Prototype**: the components that each stack of one kind has before a patch (STK-PRO-*).
- **Neutral value**: the one value of a component type that a stack entity does not store
  (section 6).
- **Row**: the kind and all item components of one stack, without the count and the placement.

In scope:

- the value form and its codecs (STK-SAV-*, STK-NET-*, STK-TPL-*, STK-HSH-*);
- the prototype table and the registry of item components;
- the entity form: components, read tools, commands, placement, the atomic request;
- the migration from the crate `mcrs_minecraft_item`.

A later specification defines each of these domains. This document gives only their seams:

- inventory transactions (the meaning of a click, a drag, a quick move);
- the inventory of each holder type (slot layout, the menu);
- the item entity (motion, age, pickup, merge in the world);
- containers (open, close, the unloaded form, the hopper rules).

The empty stack (STK-EMP-001) has no entity. In the value form it is `None` of
`Option<ItemStack>`.

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

All crate names are **(proposal)**. The layer numbers are those of `CLAUDE.md`.

| Crate | Layer | Bevy | Owns | Depends on |
|-|-|-|-|-|
| `mcrs_minecraft_item_stack` | 3, data | feature `bevy`: derives only | `ItemStack`, `PatchEntry`, `Kind`, `Count`, one type for each component type, the traits of section 4.1, the codecs of the spec section 6 | wire, NBT, SNBT, text, registry ids |
| `mcrs_minecraft_item_table` | 3, data | feature `bevy`: one resource derive | `ItemTable`, `Prototype`, `ItemRegistry`; the bake from the data pack | `mcrs_minecraft_item_stack`, registries |
| `mcrs_minecraft_inventory_plan` | 3, rules | none | the planner of an atomic request | nothing (`core` and `alloc`) |
| `mcrs_minecraft_engine_item` | 4, engine | required | the entity form, hooks, commands, read tools, the request command, `Equipped` | the three crates above, `bevy_ecs`, `bevy_app`, `bevy_math` |
| `mcrs_minecraft_item_behaviour` | 5, vanilla | none | what each item does (later specification) | layer 3 crates |
| `mcrs_minecraft_vanilla_item` | 5, vanilla | required | the removable plugin that schedules the behaviour | `mcrs_minecraft_engine_item`, `mcrs_minecraft_item_behaviour` |
| `mcrs_minecraft_recipe` | 3, data | none | `Ingredient`; the keys of the recipe serializers, the recipe book categories, the recipe displays and the slot displays | `mcrs_minecraft_item_stack` (the old crate until step 11), registries, wire |
| `mcrs_minecraft_menu` | 3, data | none | `MenuType` and the keys of the menu registry; later the slot layout of each menu | resource |

- The three layer 3 crates start as `#![no_std]` with `alloc`. If a codec dependency forbids it,
  the crate states which one.
- The protocol crates and the save crate depend on `mcrs_minecraft_item_stack` only.
- A proxy or a tool takes `mcrs_minecraft_item_stack` alone. A bot adds the table and the engine
  crate.
- The Bedrock edge depends on the same crates. The network id of a stack is state of one
  connection in the edge. No crate of this map stores it.
- The planner knows no edition. It gets slot ids and stack ids as type parameters.

# 4. Public names

Each item below is `pub`. All other items are `pub(crate)`.

## 4.1 `mcrs_minecraft_item_stack`

```rust
pub struct Kind(pub u16);                  // the item id (spec section 0.1)
pub struct Count(/* private, non-zero */);
impl Count {
    pub fn new(count: u8) -> Option<Self>;
    pub fn get(self) -> u8;
}
pub struct ItemStack { pub kind: Kind, pub count: Count, /* private patch */ }
impl ItemStack {
    // No check of the kind. For a stack of air, the wire writer writes the empty stack and the
    // saved writer gives an error.
    pub fn new(kind: Kind, count: Count) -> Self;
    pub fn set<T: ItemComponent>(self, value: T) -> Self;
    pub fn unset<T: ItemComponent>(self) -> Self;
    pub fn patch(&self) -> &[PatchEntry];
}
pub enum PatchEntry { Value(Builtin), Removal(BuiltinKey), Custom(CustomValue) }
pub enum Builtin { /* one variant for each built-in component type */ }
pub enum BuiltinKey { /* one variant for each built-in component type */ }
pub trait ItemComponent: Clone + PartialEq + Hash { /* sealed; hidden methods */ }
pub trait Neutral: ItemComponent { const NEUTRAL: &'static Self; }
pub trait CustomComponent: Clone + PartialEq + Hash {
    const KEY: &'static str;
    fn encode(&self, bytes: &mut Vec<u8>);
    fn decode(bytes: &[u8]) -> Option<Self>;
}
pub struct CustomValue { pub key: &'static str, pub bytes: Box<[u8]> }
```

- The user does not implement `ItemComponent`. Each built-in type has it, and each
  `CustomComponent` gets it.
- With the feature `bevy`, `Kind`, `Count` and each component type derive `Component`.
- A heavy component type holds `Cow<'static, _>`. Section 6 states where the borrow comes from.
- The codec functions follow the formats of the spec section 6. Each one returns `Result` with an
  error enum that the caller can match on. Their names come with the codec step (section 10).

The first batch of component types is: `MaxStackSize`, `MaxDamage`, `Damage`, `Unbreakable`,
`ItemModel`, `ItemName`, `CustomName`, `Rarity`, `Lore`, `Enchantments`, `AttributeModifiers`,
`RepairCost`, `TooltipDisplay`, `Food`, `Tool`. The order rule for the other types is:

1. the types that a rule of `spec/item-stack.md` section 7 reads;
2. the types that each vanilla prototype carries;
3. the types that a behaviour in this repository reads;
4. the other types, by the number of prototypes that carry them.

A type that has no Rust type yet travels as its encoded bytes in the patch. Thus a round trip
through the value form loses no data (STK-TYP-003).

## 4.2 `mcrs_minecraft_item_table`

```rust
pub struct Prototype { /* private */ }
impl Prototype {
    pub fn get<T: ItemComponent>(&self) -> Option<&T>;
}
pub struct ItemTable { /* private: &'static [Prototype] */ }   // Copy; a Resource with `bevy`
impl ItemTable {
    pub fn leak(prototypes: impl IntoIterator<Item = Vec<Builtin>>) -> Self;
    pub fn prototype(&self, kind: Kind) -> Option<&'static Prototype>;
}
pub struct ItemRegistry { /* private: one row of fn pointers for each component type */ }
pub enum RowHashing { Included, Excluded }
```

`ItemRegistry` is not generic. Each row holds `fn` pointers for equality, hash, clone and the
conversion to and from the value form. It has no `dyn`.

## 4.3 `mcrs_minecraft_inventory_plan`

```rust
pub const MAX_OPS: usize = 128;
pub enum Op<I> {
    Split { from: I, to: I, count: u8 },
    Merge { from: I, to: I },
    Swap { left: I, right: I },
    Drop { from: I, count: u8 },
}
pub enum Reason {
    TooManyOps, NoSuchSlot, SameSlot, BadCount, EmptySource,
    NotEnough, TakeDenied, NotEqual, PlaceDenied, NoRoom,
}
pub struct Rejected { pub op: usize, pub reason: Reason }
pub struct Planned<T> { pub stack: T, pub kind: u16, pub count: u8, pub max: u8 }
pub enum Filter { Permits, Refuses }
pub struct Loaded<T> { pub stack: Option<Planned<T>>, pub limit: u8, pub take: Filter }
pub struct Entry<I, T> { pub slot: I, pub before: Option<Planned<T>>, pub after: Option<Planned<T>> }
pub struct Plan<I, T> { /* inline scratch: 256 slots, 128 dropped stacks */ }
impl<I, T> Plan<I, T> {
    pub const fn new() -> Self;
    pub fn changes(&self) -> impl Iterator<Item = Entry<I, T>> + '_;
    pub fn dropped(&self) -> impl Iterator<Item = Planned<T>> + '_;
}
pub fn plan<I: Copy + PartialEq, T: Copy + PartialEq>(
    ops: impl IntoIterator<Item = Op<I>>,
    scratch: &mut Plan<I, T>,
    load: impl FnMut(I) -> Option<Loaded<T>>,
    place: impl FnMut(I, T) -> Filter,
    equal: impl FnMut(T, T) -> bool,
) -> Result<(), Rejected>;
```

The operations, the reasons and the order of the checks are the rules `ITX-*` of
`spec/inventory-transactions.md`. The crate links no `alloc`. A `load` that returns `None` gives
`NoSuchSlot`. Two stacks of different `kind` are never equal; the planner calls `equal` only for the
same `kind`. A slot limit out of range is an error of the caller. An operation that makes or
destroys items is not part of a request.

The caller owns `scratch` and uses it again for the next request.

## 4.4 `mcrs_minecraft_engine_item`

```rust
pub struct ItemPlugin { pub table: ItemTable }
pub struct ItemSet;                                // the set of the projection of `Equipped`
pub trait ItemAppExt {                             // for `App`
    fn register_item_component<T: CustomComponent + Component>(&mut self)
        -> Result<&mut Self, RegisterError>;
}
pub enum RegisterError { DuplicateKey, MaskFull, UnsupportedStorage }

// Components
pub struct Removed { /* private */ }
impl Removed { pub fn contains<T: ItemComponent>(&self) -> bool; }
pub struct InSlot { /* private; immutable component; no public constructor */ }
impl InSlot { pub fn holder(&self) -> Entity; pub fn index(&self) -> u16; }
pub struct Slots { /* private */ }
impl Slots {
    pub fn new(size: usize) -> Self;
    pub fn len(&self) -> usize;
    pub fn get(&self, index: usize) -> Option<Entity>;
    pub fn iter(&self) -> impl Iterator<Item = Option<Entity>> + '_;
    pub fn occupied(&self) -> impl Iterator<Item = Entity> + '_;
    pub fn first_free(&self) -> Option<u16>;
    pub fn first_occupied(&self) -> Option<Occupant>;
}
pub struct Occupant { pub index: u16, pub stack: Entity }
pub struct Dropped { pub pos: DVec3 }
pub struct Selected(pub u8);
pub struct Equipped;                               // marker; read only for the user

// Read tools
#[derive(QueryData)]
pub struct Effective<T: ItemComponent + Component> { /* reads T, Removed, Kind */ }
// on the item of the query:
    pub fn get(&self) -> Option<&'w T>;
    pub fn or_neutral(&self) -> &'w T where T: Neutral;
    pub fn state(&self, table: &ItemTable) -> ComponentState<'w, T>;
pub fn effective<'a, T: ItemComponent>(stored: Option<&'a T>, removed: Option<&Removed>)
    -> Option<&'a T>;
pub enum ComponentState<'a, T> { Default(&'a T), Overridden(&'a T), Removed, Absent }
#[derive(SystemParam)]
pub struct StackRows { /* reads each item component and Kind; not Count; not placement */ }
impl StackRows<'_, '_> {
    pub fn kind(&self, stack: Entity) -> Option<Kind>;
    pub fn mergeable(&mut self, left: Entity, right: Entity) -> bool;
    pub fn row_hash(&mut self, stack: Entity) -> Option<u64>;
}
#[derive(SystemParam)]
pub struct Stacks { /* reads each item component, Kind, Count, Removed, ItemTable */ }
impl<'w> Stacks<'w, '_> {
    pub fn kind(&self, stack: Entity) -> Option<Kind>;
    pub fn count(&self, stack: Entity) -> Option<Count>;
    pub fn get<T: ItemComponent>(&mut self, stack: Entity) -> Option<&'w T>;
    pub fn get_or_neutral<T: Neutral>(&mut self, stack: Entity) -> &'w T;
    pub fn state<T: ItemComponent>(&mut self, stack: Entity) -> ComponentState<'w, T>;
    pub fn to_value(&mut self, stack: Entity) -> Option<ItemStack>;
    pub fn mergeable(&mut self, left: Entity, right: Entity) -> bool;
    pub fn row_hash(&mut self, stack: Entity) -> Option<u64>;
}

// Commands
pub trait ItemCommandsExt {                        // for `Commands`
    fn spawn_item(&mut self, kind: Kind) -> EntityCommands<'_>;
    fn spawn_stack(&mut self, value: ItemStack) -> EntityCommands<'_>;
}
pub trait StackCommandsExt {                       // for `EntityCommands` of a stack
    fn set<T: ItemComponent>(&mut self, value: T) -> &mut Self;
    fn unset<T: ItemComponent>(&mut self) -> &mut Self;
    fn reset<T: ItemComponent>(&mut self) -> &mut Self;
    fn consume(&mut self, count: u8) -> &mut Self;
    fn put_in(&mut self, holder: Entity) -> &mut Self;
    fn put_at(&mut self, holder: Entity, index: u16) -> &mut Self;
    fn swap_with(&mut self, other: Entity) -> &mut Self;
    fn drop_at(&mut self, pos: DVec3) -> &mut Self;
}
pub trait HolderCommandsExt {                      // for `EntityCommands` of a holder
    fn give(&mut self, gift: impl Into<Gift>) -> &mut Self;
    fn give_or_drop(&mut self, gift: impl Into<Gift>, pos: DVec3) -> &mut Self;
    fn request(&mut self, request: Request) -> &mut Self;
    fn load_inventory(&mut self, values: Vec<Option<ItemStack>>) -> &mut Self;
}
pub enum Gift { Stack(Entity), Value(ItemStack) }  // From<Entity>, From<ItemStack>

// Request, messages and events
pub struct SlotRef { pub holder: Entity, pub index: u16 }
pub struct RequestId(pub u32);
pub struct Request { /* private */ }
impl Request {
    pub fn new(id: RequestId) -> Self;
    pub fn split(self, from: SlotRef, to: SlotRef, count: u8) -> Self;
    pub fn merge(self, from: SlotRef, to: SlotRef) -> Self;
    pub fn swap(self, left: SlotRef, right: SlotRef) -> Self;
}
#[derive(Message, EntityEvent)]
pub struct RequestResult { pub holder: Entity, pub id: RequestId, pub outcome: Result<(), Rejected> }
#[derive(Message)]
pub struct Given { pub holder: Entity, pub stack: Option<Entity>, pub remainder: Option<Entity> }
```

`Reason` and `Rejected` come from the planner crate. The engine crate exports them again.

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

Atomic request with its result. A system that handles many holders reads
`MessageReader<RequestResult>` as an alternative to the observer:

```rust
let (from, to) = (SlotRef { holder: player, index: 0 }, SlotRef { holder: player, index: 9 });
commands.entity(player).request(Request::new(RequestId(7)).split(from, to, 16).swap(to, from));

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

app.add_plugins(ItemPlugin { table }).register_item_component::<Soulbound>()?;

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

Only the engine crate can construct `InSlot`, so no user code can break guarantee 1.

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
reads use `unsafe`. Each block states its condition, and section 9 lists the check that they need.

# 8. Budgets

All numbers are synthetic: 1,000 kinds, 10% of the stacks have a patch, one machine. They come from
the evidence of section 11. Each number is a budget until a measurement with the real data pack
replaces it in `PERF.md`. Where one number covers the two columns, the measured population mixes
the two cases.

| Measure | Regular case: a stack with no patch | Rare case: a stack with a patch |
|-|-|-|
| memory for each stack | about 178–209 bytes, mixed population | same measurement |
| heap data that the stack owns | none; each heavy value borrows from the table | only the values that the patch wrote |
| hopper tick, 10,000 hoppers | about 95 µs, mixed population | same measurement |
| allocations in the hopper tick | 0 | 0.005 for each tick; all from the split of a patched stack |
| merge check, stack to stack | 30–37 ns, mixed population | same measurement |
| valid request | about 200 ns | not measured as a different case |
| rejected request | about 80 ns | not measured as a different case |
| load of 270,000 stacks | about 31–40 ms, mixed population | same measurement |
| archetype move | none after the spawn | one for a `set` of a component that the stack does not store |

The design keeps the cost of the rare case out of the regular case: no stack without a patch has
`Removed`, an owned value or an archetype of its own.

# 9. Open points

| # | Point | Recommended answer |
|-|-|-|
| 1 | Equality of the patch of `ItemStack` depends on the order of its entries. | Keep the patch in a canonical order (by type id, one entry for each type). Then `==` agrees with STK-EQ-001. |
| 2 | `SlotRef` and `InSlot` hold the same pair. | One public type for the pair: `SlotRef`. `InSlot` holds one `SlotRef` in a private field and gives it through `slot()`. |
| 3 | `mergeable` and `row_hash` exist on `StackRows` and on `Stacks`. | Keep them on `StackRows` only, and give `Stacks` a method `rows()` that returns it. |
| 4 | The item mask and `Removed` have 64 bits; the game has 122 types and users add more. | Two words (128 bits) for the two. A registration past the limit gives `RegisterError::MaskFull`. |
| 5 | An item component with `SparseSet` storage has no table column. | Reject it at registration (`UnsupportedStorage`) until a user needs it. |
| 6 | The `unsafe` blocks for untyped column reads. | Keep them in one module. Run the tests of the engine crate under Miri in CI before the first user migrates. |
| 7 | A leaked table is never freed; owned values are not shared between stacks. | Keep the two for now. Measure the owned values on a real world before a dedup design. |
| 8 | `Damage(0)` against an absent `Damage` in the merge check. | Treat `Damage` as a type with the neutral value 0 only if the game data confirms it (STK-DMG-*, STK-EQ-003). No answer yet. |
| 9 | Does each vanilla prototype carry all types that have a neutral value? | Check it in the bake against the pinned data pack. The answer fixes the list of section 6. |
| 10 | A custom component has a codec that the user writes by hand. | Accept it for the first version. A derive is a later convenience. |
| 11 | The width of the count. STK-CNT-003 lets a wire reader accept a count above 255. | Decided: `Count` holds 8 bits. A decoder rejects a larger count with an error. `spec/item-stack.md` records this as a deviation. |
| 12 | The old crate also holds data that is not an item: key tables of other registries, recipes, dialogs. | Decided: section 10.1 names the target crate of each group and the groups that stay with the item stack. |
| 13 | A write to an item component and `mergeable` in one system need `ParamSet`. | Accept it. A later `get_mut` on the param removes the limit without a break of the API. |
| 14 | A request holds a fixed number of operations inline. | Keep it, with a `// chisle:` comment. The transaction specification gives the real maximum. |
| 15 | Questions Q1–Q7 and undecided points U1–U5 of `spec/item-stack.md` section 10. | Their answers belong to the specification. Q6 (float equality) changes point 1 and the merge check. |
| 16 | 28 experiments of `spec/item-stack.md` section 11 are not run. | Run them before the codec step is complete. A codec rule without a captured result stays marked in its test. |
| 17 | The item template and the dialog refer to each other. The text of a template names a dialog; a dialog holds texts and item templates. | Decided: the template holds the dialog as a type parameter. The dialog lives in the crate `mcrs_minecraft_dialog`, above `mcrs_minecraft_item_stack`. |
| 18 | The groups that section 10.1 keeps in `mcrs_minecraft_item_stack`: enchantment, patterns, instrument, jukebox song, painting variant, trim. | Decided as a rule: many small crates are better than one large crate. Step 4 gives each registry its own crate where a type parameter or an id breaks the cycle. A group stays only when no such cut exists; step 4 states the reason. |

# 10. Steps

Each step is one crate or one change, and one pull request. The steps with the same letter can run
in parallel.

1. **(a)** `mcrs_minecraft_inventory_plan`: the planner and its tests.
2. **(a)** `mcrs_minecraft_item_stack`, types: `Kind`, `Count`, `ItemStack`, `PatchEntry`, the
   traits, the first batch of component types, the canonical order of the patch.
3. **(a)** Move the data that is not an item out of `mcrs_minecraft_item`: the sub-steps 3.1 to
   3.5 of section 10.1.
4. **(b)** `mcrs_minecraft_item_stack`, codecs: the saved form, the wire form, the item template,
   the hashed stack, each against the fixtures.
5. **(b)** `mcrs_minecraft_item_table`: `Prototype`, `ItemTable`, `ItemRegistry`, the bake, the
   check of open point 9.
6. `mcrs_minecraft_engine_item`, entity form: the plugin, the registration, the spawn commands,
   `Effective`, `StackRows`, `Stacks`, `set`, `unset`, `reset`, `consume`. Needs steps 2 and 5.
7. `mcrs_minecraft_engine_item`, placement: `Slots`, `InSlot` and its hooks, `Dropped`, `put_in`,
   `put_at`, `swap_with`, `drop_at`, `give`, `give_or_drop`, `load_inventory`, `Given`.
8. `mcrs_minecraft_engine_item`, request: `Request`, the commit, `RequestResult`. Needs step 1.
9. `mcrs_minecraft_engine_item`, `Selected` and the projection of `Equipped`. Parallel with step 8.
10. The Miri run and the test with a counting allocator for the hopper path.
11. **(c)** Migrate the users of the old crate, one group for each pull request: the protocol and
    save crates; the engine and server crates; the client crates; the worldgen, registry, recipe and
    tool crates.
12. Delete `mcrs_minecraft_item` and its entry in the workspace manifest, in the pull request that
    moves its last user.
13. Measure the budgets of section 8 with the real data pack and write them to `PERF.md`.
14. Add component types in the order of section 4.1.

This is one migration. The old path and the new path exist together only from step 6 to step 12.

Migration surface: the crates that depend on `mcrs_minecraft_item` today.

```text
crates/  block_entity  chat_client  client  client_entity  client_gui  client_world  enchantment
         engine  engine_brain  engine_physics  engine_registries  engine_world  entity_render
         environment  game_registries  light_color  light_render  model  particle  protocol_game
         recipe  registry_catalog  save  server  worldgen_builtin
         worldgen_feature_place  worldgen_structure  worldgen_structure_place
tools/   mcrs_minecraft_client_recorder  mcrs_minecraft_update
```

Each name in `crates/` has the prefix `mcrs_minecraft_`. Step 3 removes four names from the
surface: `worldgen_structure_place`, `engine_brain`, `vanilla_worldgen` and
`mcrs_minecraft_client_recorder`. Each other crate uses the item ids or a stack type, and step 11
moves it.

## 10.1 Step 3: the data that is not an item

A group moves out of the old crate when it obeys these two conditions:

- No file of the group uses a stack type, an item component or the item template.
- A referrer that is not an item component exists, or no item component names the registry.

Each other group is part of the item stack. Thus the item components of
`mcrs_minecraft_item_stack` name its ids with no dependency on a higher crate.

| Group | Files | Target crate | Crate | Moves | Users outside the old crate |
|-|-|-|-|-|-|
| Loot | `loot.rs`, `loot_table_keys.rs`, `context_key_set_keys.rs`, `context_int_provider_keys.rs`, `context_float_provider_keys.rs` and the two `*_provider_type_keys.rs` | `mcrs_minecraft_loot` | existing | before | `block_entity`, `worldgen_feature_place`, `worldgen_structure_place`, `registry_catalog` |
| Entity domain | `damage_type.rs`, `damage_type_keys.rs`, `damage_type_tag_keys.rs`, `mob_effect_keys.rs` | `mcrs_minecraft_entity` | existing | before | `engine_brain`, `server`, `game_registries`, `registry_catalog`, `mcrs_minecraft_client_recorder` |
| Recipe | `recipe/ingredient.rs`, `recipe_serializer_keys.rs`, `recipe_book_category_keys.rs`, `recipe_display_keys.rs`, `slot_display_keys.rs` | `mcrs_minecraft_recipe` | new | before | `protocol_game`, `registry_catalog` |
| Menu | `menu_keys.rs` | `mcrs_minecraft_menu` | new | before | `protocol_game`, `registry_catalog` |
| Item ids | `item_keys.rs`, `item_tag_keys.rs`, `data_component_type_keys.rs`, `data_component_predicate_type_keys.rs`, `consume_effect_type_keys.rs`, `map_decoration_type_keys.rs`, `potion_keys.rs`, `potion_tag_keys.rs` | `mcrs_minecraft_item_stack` | new | with steps 2 and 4 | 12 crates use `item_keys.rs` |
| Values in `component/` | the keys and the tag keys of `banner_pattern`, `instrument`, `jukebox_song`, `painting_variant`, `trim_material`, `trim_pattern` | `mcrs_minecraft_item_stack` | new | with step 4 | `game_registries`, `client`, `model`, `protocol_game` |
| Ids of a component | `enchantment.rs` and its two key files, `decorated_pot_pattern.rs`, `block_transformer.rs` and their key files, the type `Recipe` | `mcrs_minecraft_item_stack` | new | with step 4 | `enchantment`, `protocol_game`, `game_registries` |
| Dialog | `dialog.rs`, `dialog_keys.rs`, `dialog_tag_keys.rs`, the three `dialog_*_type_keys.rs` files, `input_control_type_keys.rs`, `item_hover_map.rs` | `mcrs_minecraft_dialog` | new, layer 3, no Bevy | with step 4 | `game_registries`, `chat_client`, `client_gui`, `client_world`, `server` |

- The two new crates are layer 3 data crates. They have no Bevy and no `bevy` feature.
- `mcrs_minecraft_menu` starts as `#![no_std]`. `mcrs_minecraft_recipe` states the dependency that
  prevents `#![no_std]`, if one does.
- Before step 11, `mcrs_minecraft_recipe` depends on the old crate for the item id and the item
  reference trait. The old crate keeps the type `Recipe`, because the recipes component names it.
- A value of the group "Values in `component/`" holds a text, and that text holds the item template.
  An item component holds the value inline on the wire. Thus the value cannot be in a different crate.
- `mcrs_minecraft_enchantment` depends on the item id and on an item component. Thus it is above
  the item stack and cannot own the enchantment id that the enchantments component names.
- `mcrs_minecraft_block_entity` depends on the stack. Thus it cannot own the decorated pot pattern.
- `mcrs_minecraft_damage` depends on the physics crate. Thus it does not get the damage type.

The sub-steps follow. Each sub-step is one pull request, and the workspace stays green after it.
Each sub-step edits `OWNERS` in `tools/mcrs_minecraft_update/src/owners.rs`, then runs
`cargo run -p mcrs_minecraft_update -- keys`. The tool writes each key file at its new path and
writes the smaller `registry_keys.rs` of the old crate and the catalog again. No person edits a
generated file. Each sub-step deletes the moved files and their `mod` lines from the old crate.

- **3.1 Loot.** Target: `mcrs_minecraft_loot`. Move the three types of `loot.rs` and the six key
  files. Add the dependency on `mcrs_minecraft_loot` to the old crate; `component/common.rs` and
  `component/nbt_wire.rs` import from it. Edit the imports of `block_entity`,
  `worldgen_feature_place` and `worldgen_structure_place`. Remove the old dependency from
  `worldgen_structure_place`. Check: `cargo nextest run -p mcrs_minecraft_loot -p mcrs_minecraft_item
  -p mcrs_minecraft_block_entity -p mcrs_minecraft_worldgen_feature_place
  -p mcrs_minecraft_worldgen_structure_place -p mcrs_minecraft_registry_catalog
  -p mcrs_minecraft_update`.
- **3.2 Entity domain.** Target: `mcrs_minecraft_entity`. Move `damage_type.rs`, its `resolve_as_is!`
  line and the three key files. Edit the imports of five files in `component/`, of `harness.rs`, and
  of `engine_brain`, `server`, `game_registries` and the client recorder. Remove the old dependency
  from `engine_brain` and from the client recorder. Check: `-p mcrs_minecraft_entity
  -p mcrs_minecraft_item -p mcrs_minecraft_engine_brain -p mcrs_minecraft_server
  -p mcrs_minecraft_game_registries -p mcrs_minecraft_registry_catalog
  -p mcrs_minecraft_client_recorder -p mcrs_minecraft_update`.
- **3.3 Recipe.** Target: the new crate `mcrs_minecraft_recipe`. Move `recipe/ingredient.rs` as
  `ingredient.rs` and the four key files. The file `recipe/mod.rs` becomes `recipe.rs` and keeps only
  the type `Recipe`. Add the crate to the workspace manifest, to `NO_BEVY_PACKAGES` and to the
  dependencies of `registry_catalog` and `protocol_game`. Edit the imports of `protocol_game`. Check:
  `-p mcrs_minecraft_recipe -p mcrs_minecraft_item -p mcrs_minecraft_protocol_game
  -p mcrs_minecraft_registry_catalog -p mcrs_minecraft_update`.
- **3.4 Menu.** Target: the new crate `mcrs_minecraft_menu`. Move `menu_keys.rs`. Add the crate to
  the workspace manifest, to `NO_BEVY_PACKAGES` and to the dependencies of `registry_catalog` and
  `protocol_game`. Edit one import of `protocol_game`. Check: `-p mcrs_minecraft_menu
  -p mcrs_minecraft_protocol_game -p mcrs_minecraft_registry_catalog -p mcrs_minecraft_update`.
- **3.5 Unused dependency.** `vanilla_worldgen` names the old crate in its manifest and uses it in
  no source file. Remove the line. Check: `-p mcrs_minecraft_vanilla_worldgen`.

Order: 3.1, 3.2, 3.3, 3.4 run one after the other. They share `owners.rs`, the `lib.rs` and the
`registry_keys.rs` of the old crate and the generated file of the catalog. Sub-step 3.5 shares no
file with them and runs in parallel. All sub-steps run in parallel with steps 1 and 2; only 3.3 and
3.4 share the workspace manifest with step 2.

After step 3 the old crate holds the item stack files and the four groups of the table that have no
"before". Steps 2 and 4 name `mcrs_minecraft_item_stack` in `OWNERS` for the registries of these
groups. Thus step 12 deletes a crate that holds only the item stack.

# 11. Evidence

A throwaway crate proved the API of section 4.4 and gave the numbers of section 8: branch
`spike/item-s1h`, commit `0eddd2c76`, on Bevy 0.20.1. That code is not a base for the
implementation. The implementation takes the API surface from this document and the behaviour from
the specification.

The design uses component hooks, immutable components, entity commands, derived `QueryData` and
`SystemParam`, messages and entity events. Bevy 0.19.1 and 0.20.1 have all of them. The upgrade of
the workspace to 0.20.1 is a different task, and this design does not depend on it.
