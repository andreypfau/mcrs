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
| `mcrs_minecraft_item_table` | 3, data | feature `bevy`: one resource derive | `ItemTable`, `Prototype`; the bake from the data pack | `mcrs_minecraft_item_stack`, registries |
| `mcrs_minecraft_inventory_transaction` | 3, rules | none | the planner of an atomic request | nothing (`core` and `alloc`) |
| `mcrs_minecraft_engine_item` | 4, engine | required | the entity form, hooks, commands, read tools, the request command, `Equipped` | the three crates above, `bevy_ecs`, `bevy_app`, `bevy_math` |
| `mcrs_minecraft_item_behaviour` | 5, vanilla | none | what each item does (later specification) | layer 3 crates |
| `mcrs_minecraft_vanilla_item` | 5, vanilla | required | the removable plugin that schedules the behaviour | `mcrs_minecraft_engine_item`, `mcrs_minecraft_item_behaviour` |
| `mcrs_minecraft_recipe` | 3, data | none | `Ingredient`; the keys of the recipe serializers, the recipe book categories, the recipe displays and the slot displays | `mcrs_minecraft_item_stack` (the old crate until step 11), registries, wire |
| `mcrs_minecraft_inventory_menu` | 3, data | none | `MenuType` and the keys of the menu registry; later the slot layout of each menu | resource |
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
| `mcrs_minecraft_dialog` | 3, data | none | `Dialog` and the keys of the five dialog registries | `mcrs_minecraft_item_stack`, text, NBT, registries |

- Each layer 3 crate starts as `#![no_std]` with `alloc`. If a dependency forbids it, the crate
  states which one.
- `I` is the type parameter of the text (`TextTypes` of the text crate). `mcrs_minecraft_item_stack`
  depends on the five crates from `mcrs_minecraft_banner_pattern` to `mcrs_minecraft_trim`.
  `ItemHover` of the text crate is `I` (section 10.3.4).
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
```

The table has no registry of component operations. The value form compares, hashes and clones a
`PatchEntry` directly. The rows of `fn` pointers over the entity form belong to the engine crate.

## 4.3 `mcrs_minecraft_inventory_transaction`

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
pub struct ItemPlugin { pub table: ItemTable, /* private: the schedule */ }
impl ItemPlugin {
    pub fn new(table: ItemTable) -> Self;          // `ItemSet` in `FixedUpdate`
    pub fn in_schedule(self, schedule: impl ScheduleLabel) -> Self;
}
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

app.add_plugins(ItemPlugin::new(table)).register_item_component::<Soulbound>()?;

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
| 12 | The old crate also holds data that is not an item: key tables of other registries, recipes, dialogs. | Decided: sections 10.1 and 10.2 name the target crate of each group. |
| 13 | A write to an item component and `mergeable` in one system need `ParamSet`. | Accept it. A later `get_mut` on the param removes the limit without a break of the API. |
| 14 | A request holds a fixed number of operations inline. | Keep it, with a `// chisle:` comment. The transaction specification gives the real maximum. |
| 15 | Questions Q1–Q7 and undecided points U1–U5 of `spec/item-stack.md` section 10. | Their answers belong to the specification. Q6 (float equality) changes point 1 and the merge check. |
| 16 | 28 experiments of `spec/item-stack.md` section 11 are not run. | Run them before the codec step is complete. A codec rule without a captured result stays marked in its test. |
| 17 | The item template and the dialog refer to each other. The text of a template names a dialog; a dialog holds texts and item templates. | Decided, and replaced by section 10.3.4: a text holds `ItemHover` and `DialogRef`, and `mcrs_minecraft_text::Text` is the one text type. The dialog lives in the crate `mcrs_minecraft_dialog`, above `mcrs_minecraft_item_stack`. |
| 18 | The groups that section 10.1 keeps in `mcrs_minecraft_item_stack`: enchantment, patterns, instrument, jukebox song, painting variant, trim. | Decided as a rule: many small crates are better than one large crate. Section 10.2 gives each registry its own crate. An id or a type parameter breaks each cycle. Only the three discriminator tables stay, and section 10.2 states the reason. |
| 19 | The order of the hidden types of `TooltipDisplay`. | Decided: the value keeps the input order, because the hashed stack hashes that order. The equality and the hash for the merge check ignore the order. Sub-step 4.12 changes the type. |
| 20 | Public fields of the numeric components against range checks. | Decided: the fields stay public. Each codec checks the range at the edge and returns an error. |
| 21 | The width of the registry id newtypes. | Decided: an id is `u16` where the registry cannot exceed 65,536 entries. Sub-step 4.12 narrows `EnchantmentId`, `AttributeId`, `BlockId`, `BlockTagId` and `ComponentType`. |

# 10. Steps

Each step is one crate or one change, and one pull request. The steps with the same letter can run
in parallel.

1. **(a)** `mcrs_minecraft_inventory_transaction`: the planner and its tests.
2. **(a)** `mcrs_minecraft_item_stack`, types: `Kind`, `Count`, `ItemStack`, `PatchEntry`, the
   traits, the first batch of component types, the canonical order of the patch.
3. **(a)** Move the data that is not an item out of `mcrs_minecraft_item`: the sub-steps 3.1 to
   3.5 of section 10.1.
4. **(b)** The registry groups that are still in the old crate, and the codecs of
   `mcrs_minecraft_item_stack`: the sub-steps 4.1 to 4.19 of section 10.2.
5. **(b)** `mcrs_minecraft_item_table`: the sub-steps 5.1 to 5.4 of section 10.2.
6. `mcrs_minecraft_engine_item`, entity form: the plugin, the registration, the spawn commands,
   `Effective`, `StackRows`, `Stacks`, `set`, `unset`, `reset`, `consume`. Needs steps 2 and 5.
7. `mcrs_minecraft_engine_item`, placement: `Slots`, `InSlot` and its hooks, `Dropped`, `put_in`,
   `put_at`, `swap_with`, `drop_at`, `give`, `give_or_drop`, `load_inventory`, `Given`.
8. `mcrs_minecraft_engine_item`, request: `Request`, the commit, `RequestResult`. Needs step 1.
9. `mcrs_minecraft_engine_item`, `Selected` and the projection of `Equipped`. Parallel with step 8.
10. The Miri run and the test with a counting allocator for the hopper path.
11. **(c)** Migrate the users of the old crate, one group for each pull request. Section 10.3 gives
    the groups, their order and the new home of each type. The key files of section 10.2 that have
    the mark "step 11" move in these pull requests.
12. Delete `mcrs_minecraft_item` and its entry in the workspace manifest, in the pull request that
    moves its last user.
13. Measure the budgets of section 8 with the real data pack and write them to `PERF.md`.
14. Add component types in the order of section 4.1.

This is one migration. The old path and the new path exist together only from step 6 to step 12.

## 10.1 Step 3: the data that is not an item

A group moves out of the old crate when it obeys these two conditions:

- No file of the group uses a stack type, an item component or the item template.
- A referrer that is not an item component exists, or no item component names the registry.

Section 10.2 gives the target crate of each other group.

| Group | Files | Target crate | Crate | Moves | Users outside the old crate |
|-|-|-|-|-|-|
| Loot | `loot.rs`, `loot_table_keys.rs`, `context_key_set_keys.rs`, `context_int_provider_keys.rs`, `context_float_provider_keys.rs` and the two `*_provider_type_keys.rs` | `mcrs_minecraft_loot` | existing | before | `block_entity`, `worldgen_feature_place`, `worldgen_structure_place`, `registry_catalog` |
| Entity domain | `damage_type.rs`, `damage_type_keys.rs`, `damage_type_tag_keys.rs`, `mob_effect_keys.rs` | `mcrs_minecraft_entity` | existing | before | `engine_brain`, `server`, `game_registries`, `registry_catalog`, `mcrs_minecraft_client_recorder` |
| Recipe | `recipe/ingredient.rs`, `recipe_serializer_keys.rs`, `recipe_book_category_keys.rs`, `recipe_display_keys.rs`, `slot_display_keys.rs` | `mcrs_minecraft_recipe` | new | before | `protocol_game`, `registry_catalog` |
| Menu | `menu_keys.rs` | `mcrs_minecraft_inventory_menu` | new | before | `protocol_game`, `registry_catalog` |
- The two new crates are layer 3 data crates. They have no Bevy and no `bevy` feature.
- `mcrs_minecraft_inventory_menu` starts as `#![no_std]`. `mcrs_minecraft_recipe` states the dependency that
  prevents `#![no_std]`, if one does.
- Before step 11, `mcrs_minecraft_recipe` depends on the old crate for the item id and the item
  reference trait. The old crate keeps the type `Recipe` until step 11.
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
- **3.4 Menu.** Target: the new crate `mcrs_minecraft_inventory_menu`. Move `menu_keys.rs`. Add the crate to
  the workspace manifest, to `NO_BEVY_PACKAGES` and to the dependencies of `registry_catalog` and
  `protocol_game`. Edit one import of `protocol_game`. Check: `-p mcrs_minecraft_inventory_menu
  -p mcrs_minecraft_protocol_game -p mcrs_minecraft_registry_catalog -p mcrs_minecraft_update`.
- **3.5 Unused dependency.** `vanilla_worldgen` names the old crate in its manifest and uses it in
  no source file. Remove the line. Check: `-p mcrs_minecraft_vanilla_worldgen`.

Order: 3.1, 3.2, 3.3, 3.4 run one after the other. They share `owners.rs`, the `lib.rs` and the
`registry_keys.rs` of the old crate and the generated file of the catalog. Sub-step 3.5 shares no
file with them and runs in parallel. All sub-steps run in parallel with steps 1 and 2; only 3.3 and
3.4 share the workspace manifest with step 2.

After step 3 the old crate holds the item stack files and the groups of section 10.2.

## 10.2 Steps 4 and 5: the sub-steps

`mcrs_minecraft_item_stack` names a registry entry by a numeric id newtype, and a codec gets the
names through a lookup parameter. Thus no item component needs a key file, and each group below
leaves the item stack. Each target crate is a layer 3 data crate without Bevy. For the wire forms,
the decision of section 10.3.3 replaces this lookup parameter: the wire forms use no table.

| Group | Files of the old crate | Target crate | What breaks the cycle | Dependents |
|-|-|-|-|-|
| Item ids | `item_keys.rs`, `item_tag_keys.rs` | `mcrs_minecraft_item_key`, new | none needed: the stack holds `Kind` | 13 crates, the update tool |
| Potion | `potion_keys.rs`, `potion_tag_keys.rs` | `mcrs_minecraft_potion`, new | numeric id | `registry_catalog` |
| Map decoration | `map_decoration_type_keys.rs` | `mcrs_minecraft_map_decoration`, new | numeric id | `registry_catalog` |
| Banner pattern | `BannerPattern` of `component/banner.rs`, two key files | `mcrs_minecraft_banner_pattern`, new | none needed: the value holds no text | `game_registries` |
| Decorated pot pattern | `decorated_pot_pattern.rs`, its key file | `mcrs_minecraft_decorated_pot_pattern`, new | numeric id | `game_registries` |
| Block transformer | `block_transformer.rs`, its key file, `tests/it/block_transformer.rs` | `mcrs_minecraft_block_transformer`, new | none needed: no file names an item | `game_registries` |
| Discriminators | `data_component_type_keys.rs`, `data_component_predicate_type_keys.rs`, `consume_effect_type_keys.rs` | `mcrs_minecraft_item_stack` | none: the group stays | 7 crates |
| Instrument | the registry value of `component/instrument.rs`, two key files | `mcrs_minecraft_instrument`, new | type parameter `I` for the text | `game_registries` |
| Jukebox song | `JukeboxSong` of `component/instrument.rs`, one key file | `mcrs_minecraft_jukebox_song`, new | type parameter `I` | `client` |
| Painting variant | the registry value of `component/painting.rs`, two key files | `mcrs_minecraft_painting_variant`, new | type parameter `I` | `game_registries`, `protocol_game` |
| Trim | `TrimMaterial`, `TrimPattern` of `component/trim.rs`, two key files | `mcrs_minecraft_trim`, new | type parameter `I` | `model`, `protocol_game` |
| Enchantment | `enchantment.rs`, two key files | `mcrs_minecraft_enchantment`, existing | numeric id `EnchantmentId` | `protocol_game` |
| Recipe type | `recipe.rs` | `mcrs_minecraft_recipe`, existing | none needed: the recipes component holds names | none |
| Dialog | `dialog.rs`, six key files | `mcrs_minecraft_dialog`, new | none needed: a text names a dialog by `DialogRef` (section 10.3.4) | `game_registries`, `registry_catalog`, `chat_client`, `client_gui`, `client_world`, `server` |

- The discriminators stay because each one selects a shape that the item stack crate owns. No
  value refers to one of them as a registry entry. They add the dependency on
  `mcrs_minecraft_registries` to the crate.
- `mcrs_minecraft_item_key` does not depend on the stack: a user of the item ids does not pay for
  the stack.
  The conversion from `Item` to `Kind` arrives in `mcrs_minecraft_item_table` with its first caller.
- The trim materials and the trim patterns are one crate. One component reads the two registries
  together, and neither has a different user.
- A text-bearing value of the old crate takes its text from the old template. Thus the four crates
  with `I` and the dialog crate get new types now, and their key files move in step 11.
- The enchantment crate and the recipe crate depend on the old crate until step 11. Thus their
  marker types and key files also move in step 11.

Each sub-step is one pull request. The check command is `cargo nextest run` with the `-p` list of
the sub-step. A sub-step with the mark **K** edits `OWNERS`, runs the key tool as in section 10.1,
and adds `-p mcrs_minecraft_item -p mcrs_minecraft_registry_catalog -p mcrs_minecraft_update` to
its check. A new crate goes into the workspace manifest and into `NO_BEVY_PACKAGES`.

Registry moves. The old crate depends on each new crate and imports from it.

- **4.1 Item ids (K).** Move the two files to `mcrs_minecraft_item_key`. Edit the imports of the 13
  users. Needs: none. Check: `-p mcrs_minecraft_item_key` and the 13 users.
- **4.2 Potion (K).** Move the two files to `mcrs_minecraft_potion`. Check: `-p mcrs_minecraft_potion`.
- **4.3 Map decoration (K).** Move the file to `mcrs_minecraft_map_decoration`. Check:
  `-p mcrs_minecraft_map_decoration`.
- **4.4 Banner pattern (K).** Move `BannerPattern` and the two key files. Check:
  `-p mcrs_minecraft_banner_pattern -p mcrs_minecraft_game_registries`.
- **4.5 Decorated pot pattern (K).** Move the two files. Check:
  `-p mcrs_minecraft_decorated_pot_pattern -p mcrs_minecraft_game_registries`.
- **4.6 Block transformer (K).** Move the two files and the test. Check:
  `-p mcrs_minecraft_block_transformer -p mcrs_minecraft_game_registries`.
- **4.7 Discriminators (K).** Deferred to step 11. The old crate has methods and trait
  implementations on the three generated enums, and the flag table of a component type comes from
  the old component list. A move before the old crate goes breaks the orphan rule. The move runs
  in the pull request that deletes the old crate; the flag table is written again from the
  specification in `mcrs_minecraft_item_stack`.
- **4.8 to 4.11 Instrument, jukebox song, painting variant, trim.** One new crate for each
  sub-step, with the value type over `I` and no key file. Needs: none for the types; 4.18 for a
  codec of the description. Check: `-p` the new crate.

Codecs of `mcrs_minecraft_item_stack`. Each format is one file; the per-type functions of a format
are in that file.

- **4.12 Corrections of the types.** Apply open points 19 and 21. Needs: none. Check:
  `-p mcrs_minecraft_item_stack`.
- **4.13 Hash operations.** Write the hash of a structured value (spec section 6.7) in a new file.
  Add the crate as a second output of the fixture `hash_ops`. Needs: none. Check: as 4.12.
- **4.14 Saved form.** Spec section 6.1: the frame, the error enum, the name lookup parameter, the
  bytes of a type without a Rust type, and the 12 types without text. Needs: 4.12. Check: as 4.12.
- **4.15 Wire form.** Spec sections 6.3 to 6.5 for the same 12 types. Needs: 4.14. Check: as 4.12.
- **4.16 Item template.** Spec section 6.6. The template has no type parameter: a text names a
  dialog by `DialogRef` (section 10.3.4). Needs: 4.14.
  Check: as 4.12.
- **4.17 Hashed stack.** Spec section 6.7 for the same 12 types. Needs: 4.13 and 4.15. Check: as
  4.12.
- **4.18 Text-bearing types.** `CustomName`, `ItemName` and `Lore` in the four formats. Needs: 4.14
  to 4.17, and a specification of the text component (`spec/item-stack.md`, Q8). Check: as 4.12 and
  `-p mcrs_minecraft_text`.
- **4.19 Dialog.** New crate `mcrs_minecraft_dialog` with `Dialog`, which has one form and holds
  `Text` and `ItemTemplate`. Its reader gets the lookup through `DeserializeSeed`. No key file.
  Needs: 4.16 and the text specification.
  Check: `-p mcrs_minecraft_dialog`.

The item table.

- **5.1 Table.** New crate with `Prototype` and `ItemTable`. Needs:
  4.1. Check: `-p mcrs_minecraft_item_table`.
- **5.2 Registry.** Removed: the engine crate holds the only rows of component operations.
- **5.3 Bake.** Read `assets/mcrs/item_definition/` into typed shapes and build the table. A type
  with no codec yet is counted and skipped. Needs: 5.1, 4.14. Check: as 5.1.
- **5.4 Complete bake.** A skipped type becomes a load error. Run the check of open point 9 against
  the pinned pack. Needs: 5.3, 4.18, because each vanilla prototype carries an item name. Check: as
  5.1.

Order and parallel work:

- 4.1 to 4.7 run one after the other; they share `owners.rs` and the generated catalog.
- 4.8 to 4.11 run in parallel with each other and with all other sub-steps.
- 4.12 to 4.18 form one chain in one crate. 4.13 runs in parallel with 4.12 and 4.14. 4.15 and
  4.16 run in parallel. 4.7 and the chain share `lib.rs`; merge 4.7 before 4.14.
- 5.1 and 5.2 run in parallel with the codec chain. 4.19 and 5.3 run in parallel.

Fixtures:

- The goldens that the game wrote are data, and the new tests read them: `hash_ops`, `item_plain`,
  `item_nested` and `inventory_packets` of `tools/fixtures.json`. A sub-step adds its crate as an
  output of the fixture; no person copies a golden.
- A rule with no golden gets a case from `spec/item-stack.md` section 12, or an experiment of
  section 11 (open point 16).
- `harness.rs` of the old crate holds sample values of the old types as Rust code. It is not data,
  and the new crate does not reuse it. Step 12 deletes it.

## 10.3 Step 11: the order and the types

This section gives the order of step 11. The facts are those of the commit `bf3f9d028`. Each new
type and each new home in this section is a **(proposal)**. A line with the mark **(owner)** needs a
decision of the owner before its group starts.

### 10.3.1 What the code has today

27 crates name the old crate in their manifest. `mcrs_minecraft_save` names it as a development
dependency and imports it in no file. 63 crates depend on the old crate, directly or through
other crates.

The table gives the items that each crate imports from the old crate. "Through" gives the items
that a crate imports through `mcrs_minecraft_protocol_game::item`, which exports the whole old
crate again.

| Crate | Files | Items of the old crate |
|-|-|-|
| `block_entity` | 5 | `ItemStackWithSlot`, `ItemStackValue`, `ComponentPatch` |
| `chat_client` | 1 | `ItemHoverMap` |
| `client` | 2 | `Items`, `ProtoStack`, `JukeboxSong`, `JukeboxPlayable`, `EquipmentSlot` |
| `client_entity` | 2 | `ProtoStack` |
| `client_gui` | 6 | `Items`, `ItemStack`, `StackRevision`, `Held`, `SlotTable`, `SelectedHotbarSlot`, `slots`, `Template`, `ItemHoverMap`, `DataComponentType`, `value`, the damage functions. Through: 11 component types |
| `client_world` | 4 | `Items`, `ItemStack`, `StackRevision`, `Held`, `SlotTable`, `SelectedHotbarSlot`, `slots`, `ProtoStack`, `ItemHoverMap`, `DataComponentType`, `value`. Through: `ItemStackValue`, `WireLookup` |
| `enchantment` | 2 | `Text`, `EquipmentSlotGroup`, the marker `Enchantment` |
| `engine` | 1 | `ItemStack` |
| `engine_registries` | 4 | `Items`, `ItemDefinitions`, `CORPUS_DIRECTORY` |
| `engine_world` | 1 | `ItemStack` |
| `entity_render` | 1 | `Items`, `Equippable`, `EquipmentSlot` |
| `entity_visual` | 1 | `ProtoStack`, `ItemComponentValue`, `DataComponentType`, `EquipmentSlot` |
| `environment` | 2 | `Text` |
| `game_registries` | 7 | `Template`, the dialog types, `ItemDefinitions`, `ItemEntry`, `ItemTableError`, `ItemDefinitionFile`, `ComponentMap`, `InstrumentValue`, `PaintingVariantValue`, `JukeboxSong`, `TrimMaterial`, `TrimPattern`, 12 component types in tests |
| `light_color` | 2 | `Items`, `ItemDefinitions` |
| `light_render` | 1 | `Items` |
| `loot_table` | 5 | the marker `Enchantment` |
| `model` | 2 | `Items`, `ItemStack`, `ItemComponentValue`, `ComponentPredicate`, the two discriminator enums, `RgbInt`, `DyeColor`, `TrimMaterial`, 13 component types, the effective value functions |
| `particle` | 2 | `Template`, `RgbInt`, `ArgbInt` |
| `protocol_game` | 7, and 15 of tests | `ProtoStack`, `Template`, `Text`, `DelimitedStack`, `HashedStack`, `ComponentMap`, `DataComponentType`, `PaintingVariantValue`, `TrimPatternRef`, `GlobalPos`, `DyeColor`, `EquipmentSlot`, the marker `Enchantment`, and the export of the whole crate |
| `recipe` | 1 | `ItemRef` |
| `registry_catalog` | 1 | the entries and the registry keys of seven discriminator and dialog registries |
| `save` | 0 | none: the manifest entry has no user |
| `server` | 9 | `ItemHoverMap`, `ItemStack`, `Items`, `Template` (eight files are tests) |
| `vanilla_block_place` | 1 | `Items` |
| `vanilla_falling_block` | 2 | `Items` |
| `worldgen_structure` | 1 | `MinMaxBounds`, which the old crate exports again from `mcrs_minecraft_value_provider` |

`mcrs_minecraft_vanilla_inventory` does not name the old crate. Six of its files use old types
through `protocol_game::item`: `HashedStack`, `ItemStackValue` and `ItemStackWithSlot`.

The dependencies between the 27 crates follow. An arrow points to the dependency.

```text
protocol_game  -> particle, recipe
environment    -> particle
registry_catalog -> enchantment, environment, particle, recipe
game_registries  -> enchantment, environment, loot_table, registry_catalog
engine_registries -> environment, game_registries
engine_world   -> block_entity
worldgen_structure -> block_entity
engine         -> block_entity, engine_registries, engine_world, environment, game_registries,
                  protocol_game, save
save           -> environment, game_registries, protocol_game
vanilla_block_place, vanilla_falling_block -> engine, engine_world, protocol_game
vanilla_falling_block -> vanilla_block_place
server         -> engine, engine_world, loot_table, vanilla_block_place, vanilla_falling_block
chat_client    -> protocol_game
client_world   -> block_entity, chat_client, engine_registries, engine_world, entity_visual,
                  environment, game_registries, protocol_game
client_entity  -> client_world, entity_visual, protocol_game
entity_render  -> entity_visual, game_registries, model
light_render   -> light_color
client_gui     -> block_entity, chat_client, client_world, engine_world, entity_render,
                  entity_visual, model, protocol_game
client         -> each client crate, engine, server, save, protocol_game
```

`block_entity`, `enchantment`, `entity_visual`, `light_color`, `loot_table`, `model`, `particle`,
`recipe` depend on no other crate of the set.

Five facts fix the order.

1. **A type flow decides a group, not a crate.** A public type of a crate changes together with
   each user of that type. Thus `protocol_game` and `save` cannot go first: `protocol_game` gets
   the old template from `particle` and the old item reference from `recipe`.
2. **The old model has three forms of each value.** The type parameter `M` of
   `mcrs_minecraft_registries` selects the form: `Named` is the form of a file, `Wire` is the form
   of a packet, `Resolved` is the checked form. `ItemParticle<M>`, `SlotDisplay<M>`,
   `DisplayInfo<M>`, `MerchantOffer<M>` and `ItemCost<M>` hold an old stack type in each form. The
   new stack has one form, which holds numeric ids.
3. **The new wire reader needs a lookup.** `read_wire`, `write_wire` and `read_hashed` take a
   `NameLookup`. The old `ProtoStack<Wire>` decodes with no lookup, because the old crate has the
   component types as a generated enum. That difference is the cause of four byte round trips:
   `vanilla_inventory/src/sent_stack.rs:23`, `decode_click.rs:64`,
   `set_creative_mode_slot.rs:57` and `vanilla_item_entity/src/wire_stack.rs:25`.
4. **The new crate reads 15 of the 122 component types.** The bare wire form has no length prefix.
   Thus `read_wire` gives an error for each of the other 107 types. The old crate reads all 122.
5. **The old item entry holds more than components.** `ItemEntry` holds the placed block, the
   number of container slots and the crafting remainder. `ItemTable` holds prototypes only.

### 10.3.2 The type mapping

"Exists" means that the type is in the workspace today.

Stack values:

| Old type | New home | State |
|-|-|-|
| `ProtoStack` (`Resolved`) | `Option<ItemStack>` of `mcrs_minecraft_item_stack` | exists |
| `ProtoStack<Wire>` | `WireStack`, the bare wire form, in `mcrs_minecraft_item_stack` | proposal |
| `DelimitedStack` | `PrefixedStack`, the wire form with length prefix, in `mcrs_minecraft_item_stack` | proposal |
| `stack::HashedStack`, `HashedPatchMap` | `HashedStack` of `mcrs_minecraft_item_stack`, with `Encode` and `Decode` | exists; the two traits are a proposal |
| `ItemStackValue`, `ComponentPatch` | `ItemStack`, `PatchEntry` | exists |
| `ItemStackWithSlot` | `SlotStack` | exists |
| `Template` (`Resolved`) | `ItemTemplate` in `mcrs_minecraft_item_stack` (sub-step 4.16) | proposal |
| `Template<Wire>` | `WireTemplate` in `mcrs_minecraft_item_stack` | proposal |
| `Template<Named>`, and the item of a hover | `ItemHover` of `mcrs_minecraft_text` | exists |
| `ComponentMap` of a prototype | `Prototype` of `mcrs_minecraft_item_table` | exists |
| `ComponentMap` of `ItemCost` | `WireComponents`, a list of `Builtin`, in `mcrs_minecraft_item_stack` | proposal |
| `ItemComponentValue`, `ItemDataComponent`, `for_each_data_component` | `Builtin`, `ItemComponent`, `for_each_builtin` | exists |
| `DataComponentType` | `ComponentType` and `BuiltinKey`; the generated key file moves to `mcrs_minecraft_item_stack` (sub-step 4.7) | exists; the move is step 11 |
| A component type | the type of the same name in `mcrs_minecraft_item_stack` | 15 exist; step 14 adds the others |
| `ItemPredicate`, `ComponentPredicate`, `BlockPredicate`, `AdventureModePredicate`, `CanPlaceOn`, `DataComponentPredicateType` | `mcrs_minecraft_item_stack`; the gate W2 adds them | proposal |
| `hash_ops` | `hash_stack`, `matches_hashed` and the private hash files of `mcrs_minecraft_item_stack` | exists |
| `harness` | none: step 12 deletes it | |

Text and registry values:

| Old type | New home | State |
|-|-|-|
| `Text<M>` | `Text` of `mcrs_minecraft_text`, which is `Text<ItemHover>` | exists |
| `ItemHoverMap` | none: the two text types are one type | |
| `dialog::DialogRef<M>` | `DialogRef` of `mcrs_minecraft_text` | exists |
| `Dialog`, `DialogBody`, `Input`, `Action`, the six dialog key files | `mcrs_minecraft_dialog` (sub-step 4.19) | proposal |
| `InstrumentValue` | `Instrument<ItemHover>` of `mcrs_minecraft_instrument` | exists |
| `JukeboxSong` | `JukeboxSong<ItemHover>` of `mcrs_minecraft_jukebox_song` | exists |
| `PaintingVariantValue` | `PaintingVariant<ItemHover>` of `mcrs_minecraft_painting_variant` | exists |
| `TrimMaterial`, `TrimPattern` | the types of `mcrs_minecraft_trim` over `ItemHover` | exists |
| `TrimPatternRef` | `TrimPatternRef` in `mcrs_minecraft_trim` | proposal |
| the marker `Enchantment`, its two key files | `mcrs_minecraft_enchantment` | section 10.2 |

Small values:

| Old type | New home | State |
|-|-|-|
| `RgbInt`, `ArgbInt` | `mcrs_minecraft_core` | proposal |
| `DyeColor` | `mcrs_minecraft_core` | proposal |
| `EquipmentSlot` | `mcrs_minecraft_core` | proposal |
| `EquipmentSlotGroup` | `SlotGroup` of `mcrs_minecraft_item_stack`, with a serde implementation | exists; serde is a proposal |
| `GlobalPos` | `mcrs_minecraft_core`, with the dimension as `ResourceLocationBuf` | proposal |
| `MinMaxBounds` | `mcrs_minecraft_value_provider` | exists |
| `stack::ItemRef` | `mcrs_minecraft_item_key` | proposal |

Item definitions and the entity form:

| Old type | New home | State |
|-|-|-|
| `ItemDefinitions`, `ItemEntry`, `Items`, `ItemTableError`, `ItemDefinitionFile` | `ItemTable`, `Prototype`, `BakedItems`, `BakeError`, `DefinitionFile` of `mcrs_minecraft_item_table` | exists |
| `ItemEntry::block_placer`, `container_slots`, `crafting_remainder` | `ItemLinks`, a second dense table by `Kind`, in `mcrs_minecraft_item_table` | proposal **(owner)** |
| `ItemStack` (the old component), `StackRevision`, `Held`, `Holds`, `SlotTable`, `SelectedHotbarSlot`, `slots`, the modules `value` and `effective` | the entity form of `mcrs_minecraft_engine_item` (section 4.4) | exists |
| `ItemStack` in the block drop messages of `engine_world` | `ItemStack` of `mcrs_minecraft_item_stack`; the messages lose `Copy` | proposal |

### 10.3.3 A stack in a game packet

`mcrs_minecraft_registries` states the rule that each other packet value obeys (`wire.rs:91`): the
wire form encodes and decodes with no registry, and one later step checks the value against the
registries. The dispatcher of the network layer decodes each frame before it raises the event
(`mcrs_minecraft_network/src/event.rs:52`), and it has no lookup.

| Option | What it is | Cost | Rules |
|-|-|-|-|
| (a) Bytes in the packet | The packet field borrows the bytes of the stack from the frame. The edge calls `read_wire` with the table. | No allocation and no change of `mcrs_minecraft_item_stack`. A decoder cannot find the end of a bare stack, because that form has no length prefix. Thus the option fails for a list of stacks and for a stack before another field. A test cannot compare two packets as values. | Breaks `.agents/rules/codec.md`: raw bytes are not a substitute for a symmetric codec. Obeys each layer rule. |
| (b) A context parameter | `Decode` and `Encode` get a second form that takes a lookup. | The derive, the packet table and the dispatcher of each packet change. A proxy cannot decode a packet without game data. The lookup must reach each place that decodes a frame. | Breaks the rule of layer 1 and layer 2: a frame decodes with no game data. |
| (c) A wire form with no table | The reader takes the component types from the generated key table of the pinned version. It keeps each other id as a number. The edge then checks the value with the lookup. | The signatures of sub-step 4.15 change. The key table of the component types comes to `mcrs_minecraft_item_stack` before the old crate goes. The decode allocates the patch of a stack that has one. | Obeys each layer rule, the codec rule and the rule of `wire.rs:91`. |

**Decided: option (c).** It replaces the lookup parameter of section 10.2 for the wire formats.

- `WireStack`, `PrefixedStack` and `WireTemplate` implement `Encode` and `Decode`. Each one holds
  the stack that the frame gives, not checked. A packet field for the encode direction borrows the
  stack, as `ClientboundSectionBlocksUpdate` borrows its blocks. Thus a send allocates nothing.
- `HashedStack` implements `Encode` and `Decode`. Its reader needs the key table only.
- The component type ids come from `data_component_type_keys.rs`. The key tool writes that file
  to `mcrs_minecraft_item_stack` as a second output until step 12 deletes the old one.
- `ItemStack` and `ItemTemplate` implement `FromWire` and `Resolve` of
  `mcrs_minecraft_registries`. `FromWire` is the one check of a decoded stack: the item exists, a
  stack of air becomes the empty stack, and STK-NET-009 holds. The edge calls it, as for each other
  packet value. No second check function exists. The saved formats keep the lookup parameter,
  because they hold names.
- The `Wire` form is `WireStack` or `WireTemplate`. The `Named` form is `ItemHover`. Then
  `ItemParticle<M>`, `SlotDisplay<M>`, `DisplayInfo<M>` and `MerchantOffer<M>` keep their derives
  and hold `M::Value<ItemTemplate>` or `M::Value<ItemStack>`. `ParticleOptions<Named>` of
  `mcrs_minecraft_environment` needs the `Named` form.
- These implementations add the dependency `mcrs_minecraft_item_stack -> mcrs_minecraft_registries`.
  The discriminators of section 10.2 add the same dependency.
- The implementations read the names through `NameLookup`. `mcrs_minecraft_item_stack` implements
  `NameLookup` for each `Resolver + Namer`, as `engine_registries/src/registry_names.rs` does for
  `Registries` today: `NameTable` gives the name of its registry, and `Resolver` and `Namer` get
  one method each that takes a registry name, as `Registries::id_of` and `names_of` do. That code
  moves down, and `RegistryNames` goes.
- Option (a) stays in one place, which exists: a component with no Rust type in the form with
  length prefix (`UndecodedWireValue`).

### 10.3.4 The item of a hover and the dialog

| Option | What it is | Cost |
|-|-|-|
| (a) `ItemHover` and `DialogRef` | A text holds the hover item as the wire holds it: the name, the count and each component as its tag. A text names a dialog by its id or holds it as a compound. | A text decodes with no table and has no form parameter. A bad hover item is not a decode error of the text. |
| (b) `Text<ItemTemplate>` | A text holds the typed template, as open point 17 decided. | A hover holds names and the template holds numeric ids. Thus each text codec needs a lookup, also in a chat packet. This is the problem of section 10.3.3 for each text. |
| (c) A `Named` template type | The new crate gets a template with names, as the old crate has. | The three forms that the new model removed come back. |

**Decided: option (a).** It replaces the answer of open point 17.

- `mcrs_minecraft_text::Text` is the one text type. `CustomName`, `ItemName` and `Lore` of
  `mcrs_minecraft_item_stack` hold it today. `protocol_game::Text` becomes the same type. No
  code maps a whole text between two text types, and `ItemHoverMap` goes with the old crate.
- The four registry crates with the parameter `I` take `ItemHover` as `I`.
- `mcrs_minecraft_item_stack` gives one function that reads an `ItemHover` into an `ItemTemplate`
  with a `NameLookup`. A loader of registry data calls it for each hover of each text, so a bad
  hover item fails at load. A client calls it before it draws the hover.
- `Dialog` of `mcrs_minecraft_dialog` has no type parameter and one form. It holds `Text` and,
  for an item body, `ItemTemplate`. Its reader gets the lookup through `DeserializeSeed`.
  `mcrs_minecraft_text::DialogRef` holds an inline dialog as a compound; the same reader makes a
  `Dialog` from that compound at the edge.
- `mcrs_minecraft_registries/src/text.rs` maps a text between forms. The old crate is its last
  user, and group 10 deletes it.

### 10.3.5 The groups

Each group is one pull request that builds alone. The numbers give the order. A gate is work in
the new crates that a different step of section 10 owns.

Gates:

- **W1**: the decision of section 10.3.3 and its code in `mcrs_minecraft_item_stack`.
- **W2**: the wire codec of each of the 122 component types (step 14, wire format). The types of
  the gate C1 come first.
- **T1**: `ItemTemplate` and its codecs (sub-step 4.16), and the `Resolve` and `FromWire`
  implementations of section 10.3.3.
- **D1**: `mcrs_minecraft_dialog` (sub-step 4.19).
- **L1**: the table `ItemLinks` of `mcrs_minecraft_item_table`.
- **C1**: a Rust type in `mcrs_minecraft_item_stack` for each component type that the client reads:
  `Equippable`, `DyedColor`, `Trim`, `CustomModelData`, `PotionContents`, `FireworkExplosion`,
  `Bees`, `BlockState`, `EnchantmentGlintOverride`, `JukeboxPlayable`.

| # | Group | Crates that change | Old types that go | Crates that drop the old crate from their manifest | Gate | Files |
|-|-|-|-|-|-|-|
| 1 | Values move down | the old crate, `core`, `item_key`, `recipe`, `worldgen_structure`, `save`, `particle`, `protocol_game`, `model`, `client`, `entity_render`, `entity_visual` | `RgbInt`, `ArgbInt`, `DyeColor`, `EquipmentSlot`, `GlobalPos`, `ItemRef`, the export of `MinMaxBounds` | `recipe`, `worldgen_structure`, `save` | none | 20 |
| 2 | Text | `environment`, `enchantment`, `game_registries`, `protocol_game`, `chat_client`, `client_world`, `client_gui`, `client_entity`, `server` | `Text<M>` outside the old crate, each use of `ItemHoverMap` on a whole text | `environment`, `chat_client` | none | 40 |
| 3 | Enchantment (K) | `enchantment`, `loot_table`, `protocol_game`, the old crate | the marker `Enchantment` and its key files, `EquipmentSlotGroup` outside the old crate | `enchantment`, `loot_table` | group 2 | 16 |
| 4 | Stacks of serverbound packets | `protocol_game`, `vanilla_inventory`, `server` (tests) | none: `stack::HashedStack`, `HashedPatchMap` and `DelimitedStack` leave the packets | none | W1 | 10 |
| 5 | Item links | `item_table`, `engine_registries`, `vanilla_block_place`, `vanilla_falling_block`, `light_color`, `light_render` | `Items` in these crates | `vanilla_block_place`, `vanilla_falling_block`, `light_color`, `light_render` | L1 | 14 |
| 6 | Drops and block entities | `engine`, `engine_world`, `block_entity`, the readers of the drop messages | the old component `ItemStack` in messages, `ItemStackWithSlot`, `ItemStackValue` | `engine`, `engine_world`, `block_entity` | none | 15 |
| 7 | Registry values (K) | `game_registries`, `registry_catalog`, `protocol_game`, `client` (tests), `model`, the four crates with `I`, `dialog`, the old crate | the dialog types, `InstrumentValue`, `PaintingVariantValue`, `JukeboxSong`, `TrimMaterial`, `TrimPattern`, their key files | none | D1, group 2 | 30 |
| 8 | Client state and the stacks and templates of clientbound packets | `client_world`, `client_entity`, `client_gui`, `client`, `model` (`EntityStack`, `Evaluator`), `entity_visual`, `particle`, `protocol_game`, `vanilla_inventory`, `vanilla_item_entity`, `replication`, `engine`, `server` (tests) | the old entity form, `ProtoStack<M>`, `stack::HashedStack`, `HashedPatchMap`, `DelimitedStack`, `TrimPatternRef`, `DataComponentType` in packets, the export `protocol_game::item`; `Template<M>` and `ComponentMap` leave the packets | `particle`, `protocol_game`, `client_world`, `client_entity`, `client_gui`, `entity_visual` | W1, W2, T1, C1 | 65 |
| 9 | Model and render | `model`, `entity_render`, `client` | `ItemComponentValue`, the old component types, `Items` on the client | `model`, `entity_render`, `client` | C1, group 8 | 8 |
| 10 | The end (K), step 12 | `item_stack`, `registry_catalog`, `engine_registries`, `game_registries`, `server` | the discriminator enums, `Items`, `ItemDefinitions`, `Template`, `ComponentMap`, `harness` | each crate that is left; the old crate is deleted | each group | 15, and the old crate |

Before group 4, one small pull request fixes the lookup of the edge (open point 8 of section
10.3.8). `sent_stack.rs` and `vanilla_item_entity/src/wire_stack.rs` pass to `write_wire` one
`NameLookup` that joins `ItemTable` for the item names and `RegistryNames` of
`mcrs_minecraft_engine_registries` for the other tables. `RegistryNames` becomes public. A test at
the edge sends a stack with a component patch and reads it back with its patch.

Notes for each group:

- **Group 1.** Each type moves to a crate below the old crate, and the old crate imports it, as
  in step 3. Each other crate of the group changes its imports only. No codec changes, so each
  test of today proves the bytes.
- **Group 2.** The new type of each field is `mcrs_minecraft_text::Text`. The proof of the bytes
  is the fixture `text_vanilla`, the chat packet goldens and `tests/it/item_hover.rs` of the old
  crate. That test shows that the two forms give one tag.
- **Group 3.** The old crate depends on `mcrs_minecraft_enchantment` after this group. The proof is
  the enchantment corpus test of `game_registries` and the loot tests.
- **Group 4.** `ServerboundContainerClick` holds `HashedStack` of the new crate, and
  `ServerboundSetCreativeModeSlot` holds `PrefixedStack`. No client crate builds these packets. The
  proof is the fixture `inventory_packets`. The old types stay for the tests `golden.rs` and
  `scaffold.rs` of `protocol_game` until group 8 deletes them.
- **Group 5.** `engine_registries` inserts `Items` and `ItemTable` until group 10. The proof is
  the item corpus test: `ItemLinks` and `ItemDefinitions` give the same block for each item.
- **Group 6.** A block entity file holds stacks with names. Its reader gets the lookup through a
  `DeserializeSeed` and calls `read_saved_with_slot`. A component with no Rust type stays as its
  saved bytes. The proof is the block entity goldens of `block_entity`.
- **Group 7.** The key tool moves each key file. `protocol_game` (`entity.rs`), the tests of
  `client` and `model` (`item_model_asset.rs`) import `PaintingVariantValue`, `JukeboxSong` and
  `TrimMaterial`, so they change their imports in this group. The proof is the registry corpus
  tests of `game_registries` and the snapshot of the registry data packet.
- **Group 8.** Decided: client state and the clientbound packets are one pull request. The client
  holds its stacks in the entity form of `mcrs_minecraft_engine_item` and reads them from the new
  packet types. No temporary byte seam exists on the client. The proof is the fixtures
  `inventory_packets`, `item_plain` and `item_nested`, and the goldens of the recipe, advancement,
  particle and entity packets. This group moves the tests of section 10.3.6. The pull request is
  under the limit of 100 files of the review bot only if the estimate of 65 files holds. If it does
  not, split off the templates first: `Template<M>` in `particle` and in the recipe, advancement
  and particle packets, with their tests. That part holds no client stack.
- **Group 9.** `model` takes `ComponentPredicate` and `DataComponentPredicateType` from
  `mcrs_minecraft_item_stack`. No byte changes. The proof is the model and render tests of today.
- **Group 10.** Sub-step 4.7 runs here. No byte changes.

Parallel work: groups 1, 2, 4, 5 and 6 share no type. Group 7 runs in parallel with groups 1, 4,
5 and 6 after its gate.
Groups 3 and 7 share `owners.rs` and run one after the other. All groups are one migration.

### 10.3.6 The tests of the component codecs

`protocol_game/tests/it/item/` holds 4,897 lines in 17 files. They test the old crate through the
export `protocol_game::item`.

| Files | Lines | What happens |
|-|-|-|
| `advancement.rs`, `particle.rs`, `recipe_packets.rs` | 976 | They test packets. They stay in `protocol_game`, and group 8 writes them over the new types with the same goldens. |
| `plain.rs`, `nested.rs`, `golden.rs`, `records.rs`, `registry_refs.rs`, `holders.rs`, `slotted.rs`, `symmetry.rs`, `table.rs` | 2,455 | They test component codecs. They do not move. Each pull request of step 14 adds its types to the table tests of `mcrs_minecraft_item_stack/tests/it/`, which read the goldens `item_plain` and `item_nested`. |
| `predicate.rs` | 335 | It moves with the predicate types to `mcrs_minecraft_item_stack`. |
| `corpus.rs` | 224 | The bake test of `mcrs_minecraft_item_table` replaces it when sub-step 5.4 is complete. |
| `scaffold.rs`, `harness.rs`, `mod.rs` | 907 | Test support of the old types. Group 8 deletes them. |

- The old tests run against the old crate until group 8. They stay the proof of the old codecs
  while a client uses them.
- Group 8 deletes a file of the second row only when the new table test reads each case of the
  golden. The new test fails for a case of a type that has no codec. Thus the gate W2 is a test,
  not a list.
- `tools/fixtures.json` gets `mcrs_minecraft_item_stack` as an output of `item_plain` and
  `item_nested`. Group 8 removes the output to `protocol_game`.

### 10.3.7 The byte round trips

Four round trips exist, two for each direction.

- **Serverbound**: `decode_click.rs:64` and `set_creative_mode_slot.rs:57`. Group 4 removes them.
  It is the smallest group: about 10 files. It needs the gate W1 only, because the hashed form and
  the form with length prefix state their lengths. It can come first.
- **Clientbound**: `sent_stack.rs:23` and `vanilla_item_entity/src/wire_stack.rs:25`. Group 8
  removes them. It cannot come early. A client must read each stack that a server sends, and the
  bare form needs the codec of each component type (gate W2).

One early variant exists for the clientbound direction, and open point 4 of section 10.3.8
rejects it.
The packet field holds the bytes of the stack, and its decoder finds the end with the old reader.
The server then sends with no parse. Each user of the packets changes two times: at the variant
and at group 8.

### 10.3.8 Open points of step 11

| # | Point | Recommended answer |
|-|-|-|
| 1 | The wire form of a stack in a packet. | Decided: option (c) of section 10.3.3. |
| 2 | The hover item and the dialog. This replaces the answer of open point 17. | Decided: option (a) of section 10.3.4. |
| 3 | `Resolver` and `Namer` select a registry by a marker type. `NameLookup` selects it by `NameTable`. `mcrs_minecraft_item_stack` cannot name the markers of the block and the enchantment crates. | Decided in section 10.3.3: the code of `engine_registries/src/registry_names.rs`, which implements `NameLookup` over `Registries`, moves down as the implementation for each `Resolver + Namer`. |
| 4 | The clientbound packets need the gate W2, which is all of step 14 for the wire format. | Decided: group 8 waits for the wire codecs of all 122 component types. Step 14 gives the types of the gate C1 first, then the other types. The early variant of section 10.3.7 is not used. |
| 5 | A separate group for the client state adds one byte round trip on the client. | Decided: the client state and the clientbound packets are one pull request, group 8, with about 65 files. No temporary byte seam exists on the client. |
| 6 | **(owner)** The home of the placed block, the container slots and the crafting remainder. | `ItemLinks` in `mcrs_minecraft_item_table`, baked from the same files as the prototypes. |
| 7 | The home of the item predicates. | `mcrs_minecraft_item_stack`. `CanPlaceOn`, `CanBreak` and `Lock` are component types of the stack, and `ComponentPredicate` matches the component types of the stack. Thus a crate below the stack cannot hold them, and a crate above cannot give them to the stack. The gate W2 adds them; group 9 moves `model` to them. |
| 8 | Verified: `ItemTable` is the lookup of the edge today, and it answers `NameTable::Item` only (`item_table.rs:80-93`). `sent_stack.rs:30` passes it to `write_wire`. For a stack with one patch entry, `type_id` finds no component type (`wire.rs:313`), and the edge sends the empty stack. No test at the edge covers it; `item_stack/tests/it/wire.rs:461` tests the kernel half only. | Fix it before group 4 in its own small pull request (section 10.3.5): the edge passes `ItemTable` joined with `RegistryNames` of `engine_registries`, with a test at the edge. Option (c) then removes the cause for the wire. |
| 9 | The homes of `RgbInt`, `ArgbInt`, `DyeColor`, `EquipmentSlot` and `GlobalPos`. | `mcrs_minecraft_core` for each one (section 10.3.2). `Equippable` holds `EquipmentSlot` and `lodestone_tracker` holds `GlobalPos`, so their home is below `mcrs_minecraft_item_stack`. `mcrs_minecraft_entity` and `mcrs_minecraft_dimension` pull the biome, the sound and the block crates into the stack; `core` adds no dependency. A new crate for one enum is not necessary. |
| 10 | `ItemHover` becomes the form with names of each item template, not only of a hover. | Keep the name until group 8. Then decide one new name with the owner; a new name earlier touches each text user two times. |
| 11 | The drop messages of `engine_world` lose `Copy`. | Accept it. A stack with an empty patch owns no heap memory. |
| 12 | The list "Migration surface" of section 10 names crates that step 3 removed and misses four crates. | Done: the table of section 10.3.1 replaces it, and section 10 no longer has the list. |

# 11. Evidence

A throwaway crate proved the API of section 4.4 and gave the numbers of section 8: branch
`spike/item-s1h`, commit `0eddd2c76`, on Bevy 0.20.1. That code is not a base for the
implementation. The implementation takes the API surface from this document and the behaviour from
the specification.

The design uses component hooks, immutable components, entity commands, derived `QueryData` and
`SystemParam`, messages and entity events. Bevy 0.19.1 and 0.20.1 have all of them. The upgrade of
the workspace to 0.20.1 is a different task, and this design does not depend on it.
