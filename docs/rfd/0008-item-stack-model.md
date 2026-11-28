# RFD 8: Item stack model: stack, template, view and crate split

- Authors: andreypfau
- State: prediscussion
- Labels: item

This RFD proposes the shape of the one item stack model of mcrs: the value types, the read
interface, the comparisons, the count rules at the edges and the crates. Read it before you change
`mcrs_minecraft_item_stack`, `mcrs_minecraft_item_table`, `mcrs_minecraft_engine_item` or a crate
that reads a stack.

This RFD converts and supersedes `docs/item-design.md`. Issue #1114 removes the first stack model;
this RFD gives the design of the model that stays. Its rules of behaviour are in
`docs/spec/item-stack.md`, and this RFD cites them by id (`STK-*`).

The maintainer gave the direction of each proposal in the section "Proposal". The text marks a
detail that the maintainer did not state as "proposed". No determination is made yet.

## Problem

Many consumers read and write a stack: a slot of a container, the cursor, an item entity,
equipment, a bundle, a recipe result, a trade, a loot table, a command, a predicate, a tooltip, a
model renderer, a proxy and a bot. Each of them needs the same small set of operations. The model
of today gives these operations in three different places, and some of them are missing. The
problems that follow are facts about `origin/main` at commit `c7bdbad0b`.

1. **The value is not in normal form.** STK-PAT-005 requires that the patch of each stack is in
   normal form. `ItemStack::set` and `ItemStack::unset` in
   `crates/mcrs_minecraft_item_stack/src/item_stack.rs` get no prototype, so they cannot drop an
   entry without effect (STK-PAT-004). Only the entity form and the loot work item
   (`crates/mcrs_minecraft_loot_table/src/work_stack.rs`, `set` and `set_kind`) normalize, each in
   its own way.
2. **The merge check is wrong for such a value.** `ItemStack::mergeable` and the derived `==`
   compare the kind and the raw patch. Two stacks with equal effective components and different
   patches compare as different. The entity form compares effective components
   (`crates/mcrs_minecraft_engine_item/src/stack_rows.rs`).
3. **A stack and a template are one type.** STK-TPL-007 states that an item template keeps each
   patch entry that its reader gets, and a stack does not. `BundleContents`, `ChargedProjectiles`
   and `Container` hold `ItemStack`, so no type says which rule applies.
4. **There is no one read interface.** A reader of an effective component (STK-PAT-002) uses
   `ItemTable::effective` for a value, `Stacks` and `Effective` for an entity, and the private
   `ComponentHolder` of `mcrs_minecraft_predicate` for a predicate. A predicate on a stack entity
   needs `Stacks::to_value`, which allocates. The client click prediction does that today
   (`crates/mcrs_minecraft_inventory_client/src/predict_click.rs`).
5. **A read is a linear scan.** `ItemTable::effective` scans the patch, and `Prototype::get`
   scans the prototype (`crates/mcrs_minecraft_item_table/src/prototype.rs`).
6. **The item of a stack cannot change.** Consumers such as crafting, a bucket, smithing and a
   dispenser need an operation that changes the item and keeps the patch. The value has no such
   operation, and only the loot work item has a private one. The entity form sets `Kind` at spawn
   (`crates/mcrs_minecraft_engine_item/src/spawn_stack.rs`), and no command changes it.
7. **A count above 99 has no public type.** `Count` holds `1..=99` (STK-CNT-001). A loot chain
   counts in an `i32` inside the private `WorkStack`. U6 of the specification states a trade
   result with a larger count and asks what mcrs does with it.
8. **The engine row mask has a hard cap.** The engine stores a mask of rows in a `u128`, and
   `MAX_ROWS` is 128 (`crates/mcrs_minecraft_engine_item/src/item_components.rs:17`). See the
   section "What is known" for the numbers.
9. **The value types and the codecs are one crate.** `mcrs_minecraft_item_stack` has 139 files in
   `src/`. The saved codec and the wire codec match on the private fields of some value types
   (for example `Equippable` and `TooltipDisplay`), so no codec can move to its own crate. A
   proxy that needs only the wire frame builds every value type and every codec.

The consumers that pay are the users of the library crates. A bot, a proxy or a map tool cannot
read a stack with the same code as the server, and each consumer writes its own normalization,
merge check and read path.

## What is known

Each number has the command that gives it, run at the root of the repository on `c7bdbad0b`.

| Fact | Value | Command or source |
|-|-|-|
| component types of this game version | 122 | the length of `minecraft:data_component_type` in `assets/mcrs/reports/registries.json` |
| component types with a model | 43; `docs/item-design.md` and #1114 say 44, which counts the undecoded row | `grep -c '=> "' crates/mcrs_minecraft_item_stack/src/for_each_builtin.rs` |
| engine rows in use | 44: 43 typed rows and one row for undecoded entries | `ItemComponents::from_world` |
| engine rows in use when each type has a model | 123 of 128, so 5 rows stay for custom types | 122 + 1 |
| removal bits of the entity form | 32, a `u32` in `Removed`, for the types with a neutral value | `crates/mcrs_minecraft_engine_item/src/removed.rs` |
| crates that depend on `mcrs_minecraft_item` | 28, all as a normal dependency | the `Cargo.toml` files under `crates/` |
| crates that depend on `mcrs_minecraft_item_stack` | 34 | the `Cargo.toml` files under `crates/` |
| crates that name `ProtoStack`, its own crate excluded | 9: `client`, `client_entity`, `engine_registries`, `entity_visual`, `inventory_client`, `inventory_server`, `protocol_game`, `replication`, `vanilla_item_entity` | `grep -rl ProtoStack crates` |

These are facts of our code, read on that commit. The behaviour of the game enters this RFD only
through the rules of `docs/spec/item-stack.md`. A need of a consumer that no rule states yet is
marked as a need, not as a fact about the game.

The bake makes the prototype table once for each data load and leaks it, so a prototype value is a
`'static` borrow (`crates/mcrs_minecraft_item_table/src/item_table.rs`). A heavy component holds a
`Cow<'static, _>`. A stack with an empty patch thus owns no heap data. This holds today and the
proposal keeps it.

The cost of the entity form is not measured. Nobody has measured the cost of one archetype for
each item and component set, or of the change detection of stack entities.

## Proposal

### Two value types: `ItemStack` and `ItemTemplate`

`ItemStack` is always in normal form (STK-PAT-005). The type makes this true by construction:
each constructor and each of `set`, `unset`, `reset` and `with_kind` takes the prototype of the
item. A change keeps the normal form in the same call, as STK-PAT-006 states. No public operation
gives a patch with an entry without effect.

`ItemTemplate` is an item, a count and a raw patch (STK-TPL-001). It keeps each entry that its
reader gets (STK-TPL-007), and its equality and hash include these entries. A value that holds
stacks holds templates: `BundleContents`, `ChargedProjectiles`, `Container`, and the use
remainder and sulfur cube content when they get a model. A recipe result, a trade result and a
loot item entry are templates too.

Three operations connect the two types:

- `normalize` makes a stack from a template. It drops each entry without effect (STK-PAT-004) and
  checks nothing else. A reader of a stack uses it (STK-PAT-005). It does not compare the count
  with the maximum stack size (STK-CNT-002).
- `from_template` makes a stack from a template by `normalize` and then strict validation
  (STK-TPL-006). The result can be the empty stack. A recipe result, a trade result and a loot
  entry use it.
- `to_template` makes a template from a stack by a copy. The copy does not allocate when the patch
  is empty.

A reader of a stack decodes a template first and then calls `normalize`. So the decode of each
format is one code path. A reader that has no prototype table, such as a proxy or a recorder, stops at
the template or at the wire frame. A wire frame keeps its bounds without a table (STK-NET-002).

Proposed: the constructors take `&Prototype`, and `with_kind` takes a host of one small trait:

```rust
pub trait Prototypes {
    fn prototype(&self, kind: Kind) -> Option<&Prototype>;
}

impl ItemStack {
    pub fn new(kind: Kind, count: Count, prototype: &Prototype) -> Self;
    pub fn set<T: ItemComponent>(&mut self, value: T, prototype: &Prototype);
    pub fn unset<T: ItemComponent>(&mut self, prototype: &Prototype);
    pub fn reset<T: ItemComponent>(&mut self);
    pub fn with_kind(self, kind: Kind, table: &impl Prototypes) -> Option<Self>;
}
```

`with_kind` gives `None` for a kind that the table does not have. Today `mcrs_minecraft_item_table`
depends on `mcrs_minecraft_item_stack`. So `Prototype` and the trait `Prototypes` move to
`mcrs_minecraft_item_stack`, and `ItemTable` implements the trait. `ItemTable` keeps the bake from
the data pack and the name tables.

### One read interface: `StackView`

One read trait gives the item, the count and an effective component of a type:

```rust
pub trait StackView {
    fn kind(&self) -> Kind;
    fn count(&self) -> u32;
    fn get<T: ItemComponent>(&self) -> Option<&T>;
}
```

These four owners implement it:

| Owner | Implementation |
|-|-|
| a stack and the prototype table | the patch, then the prototype (STK-PAT-002) |
| a row of a stack entity | the columns of the entity, with no copy to a value |
| a template and the prototype table | the raw patch, then the prototype |
| a loot draft (see below) | its template and the prototype table |

A raw patch and its normal form give the same effective component for each type: an entry
without effect is equal to the prototype by STK-PAT-004. So a template and a draft can implement
the view without a normalization.

Predicates, tooltips, the render state of a stack and the loot conditions take `impl StackView`.
The `ComponentHolder` and `ItemSubject` of `mcrs_minecraft_predicate` go away. Issue #1108 sets
the convention for a predicate that reads the world; this RFD only supplies the view.

The prototype gets one dense slot for each component type in place of the linear scan of today.
Proposed: the slot is an index by the component type id into the components of the prototype.
The patch of a stack stays sorted by key, so a read of the patch is a binary search.

A `count` of `u32` lets the draft give its count above 99. A stack and a template give a value in
`1..=99`.

### Named comparisons

The comparisons are named functions. No caller compares raw patches.

| Function | Meaning | Rule |
|-|-|-|
| `same_item` | the same item | none; the count and the components have no part |
| `same_components` | the same item and components | STK-EQ-001 |
| `exact` | the same item, components and count | STK-EQ-002 |
| `exact_ignoring(flags)` | as `exact`, but each component type with one of the flags has no part | the flags of `DataComponentType`; no rule yet |
| `subset` | the same item, and each component of a given list is equal | no rule yet |

A merge uses `same_components` and then the rule of its caller (STK-EQ-004). Because a stack is in
normal form, `same_components` on two stacks compares the two patches directly. On a template or
an entity row it compares through the view. The hashed comparison of a click stays as it is
(STK-HSH-003).

Consumers need `exact_ignoring` for the equip animation of the client, and `subset` for the buy
cost of a trade and for the exact part of an item predicate. Each needs a rule in a specification
before its code. `ItemStack::mergeable` goes away in the pull request that adds
`same_components` and moves its callers.

### Copies and snapshots

- No shared pointer. No `Arc` and no `Rc` in a stack, a template or a prototype, as #566
  requires.
- A stack with an empty patch does not allocate when it is copied.
- A snapshot, for example the last stack sent to a viewer or the stack before a use, compares in
  place with `exact` when it can borrow the stack. Otherwise it keeps a 64-bit digest of the
  stack. Proposed: the digest is a hash over the normal form. A digest that is equal for two
  different stacks makes a change not visible. The specification of each snapshot states whether
  that is acceptable.
- Each copy and each comparison that claims no allocation has a test with the counting allocator
  of `mcrs_test_alloc`.

### Row mask

`RowMask([u64; 4])` replaces the `u128` row mask and the cap `MAX_ROWS = 128`. It gives 256 rows.
That holds the 123 rows of a full model and leaves 133 rows for custom types.

Proposed: `Removed` uses the same mask. Today `Removed` is a `u32`, and a test requires that each
type with a neutral value has a row index below 32. That cap is a second limit beside `MAX_ROWS`.
A stack has `Removed` only when its patch has a removal, so the larger component is rare.

### Change of the item

`ItemStack::with_kind(kind, &table)` changes the item and keeps the patch. It then brings the
patch to normal form against the new prototype (STK-PAT-004). A component that the new prototype
gives with an equal value drops out of the patch.

The entity form gets one transmute command. It changes `Kind` and does the same normalization on
the rows of the entity. The entity keeps its placement, so a holder sees one change of a slot and
not a despawn and a spawn.

### Count and the draft

`Count` stays `1..=99` (STK-CNT-001). A count above 99 lives only in a public draft:

```rust
pub struct StackDraft {
    pub template: ItemTemplate,
    pub count: u32,
}

impl StackDraft {
    pub fn split(&self, max: u8, table: &impl Prototypes, out: &mut Vec<ItemStack>);
}
```

The count of the draft is the count of the result. The count of its template has no effect while
the template is in a draft. A loot function that gives a count of 0 or less gives the count 0.
The draft then keeps its item and its patch, and it reads as the empty stack (LOOT-MOD-011 of
`docs/spec/loot-functions.md`). A later function can make the count larger than 0 again. Each
function reads a count of 0 or less as 0, so a `u32` gives the same result as the `i32` of
`WorkStack`. A function on a draft writes the raw patch of the template. `split` normalizes each output stack and divides the count into stacks of at
most `max`. A draft with the count 0 gives no stack. That is the split step of a loot table
(STK-CNT-007, LOOT-OUT-001 of `docs/spec/loot.md`).

`StackDraft` replaces `WorkStack` of `mcrs_minecraft_loot_table`. The normalization that
`WorkStack::set` and `WorkStack::set_kind` do today moves to `split` and to `with_kind`.

A trade result with a count above 99 is a draft. Proposed answer to U6: the edge that puts the
draft into a slot or into a packet limits the count to 99. This needs a rule of the specification of
trades by its author before code.

### Crates

| Crate | Layer | Owns | Depends on |
|-|-|-|-|
| `mcrs_minecraft_item_component` | 3, data | each component value type with `pub` fields, the enum `Builtin` of the values, the component type enum `DataComponentType`, `Kind`, `Count`, the patch, `ItemTemplate` | the registry value crates (sound, trim, banner pattern), text, NBT, static registry |
| `mcrs_minecraft_item_stack` | 3, data | `ItemStack`, `Prototype`, `Prototypes`, `StackView`, the comparisons, `StackDraft` | `mcrs_minecraft_item_component` |
| `mcrs_minecraft_item_stack_saved` | 3, data | the saved form and the command form of a stack and a template, and the trait of a saved codec | `mcrs_minecraft_item_stack`, NBT, SNBT |
| `mcrs_minecraft_item_stack_wire` | 3, data | the wire frames, the scan of the bounds, the wire codec of each type and its trait | `mcrs_minecraft_item_stack`, wire |
| `mcrs_minecraft_item_stack_hash` | 3, data | the hashed stack, the hash of a component and its trait | `mcrs_minecraft_item_stack` |
| `mcrs_minecraft_item_table` | 3, data | `ItemTable` and the bake from the data pack | `mcrs_minecraft_item_stack`, `mcrs_minecraft_item_stack_saved` |
| `mcrs_minecraft_engine_item` | 4, engine | the entity form, `RowMask`, the transmute command | as today |

Proposed: `ItemTemplate`, the patch, `Kind` and `Count` live in the component crate. A nested
value such as `BundleContents` holds templates, and a template holds a patch of `Builtin` values.
So these types are one cycle of types and must be in one crate.

Each codec crate owns its trait and implements it for each value type. The fields of the value
types are `pub`, so a codec does not need to live in the crate of the type. A type with an
invariant keeps its fields private and gets a checked constructor and public accessors. For
example, `Container` holds at most 256 slots and drops the empty slots at its end. A plugin that adds a
component type implements the trait of each form that it needs.

Each new crate starts as `#![no_std]` with `alloc`. Each one has a `bevy` feature only for
derives, off by default.

What a user takes:

- A proxy takes `mcrs_minecraft_item_stack_wire` and copies frames. It still builds each value
  type, but not the saved codec or the hash.
- A bot adds `mcrs_minecraft_item_table` and reads stacks through `StackView`.
- A map tool or a model viewer takes the stack crate, the saved crate and the table.

### The open question: the form of a live stack

A live stack is a stack in a loaded slot, in an item entity or in equipment. Today it is an ECS
entity, and each effective component that is not neutral is an ECS component
(`docs/item-design.md`, section 6). The other option is a value in a component of its holder.

The rest of this RFD holds for each option: the two value types, the view, the comparisons, the
draft, the row mask (only for the entity form) and the crates. A measurement decides. It runs
these four cases with each option:

1. a hopper scan: many hoppers, and each one scans the slots of a neighbour container for a
   merge;
2. an item farm: thousands of item entities that try to merge;
3. a chest of 27 stacks that loads, opens, changes and saves;
4. a mob army: thousands of mobs, and a system compares the equipment of each mob with a
   snapshot.

The metrics are the time of one tick, the allocations of one tick and the memory of one stack.
No threshold is set. The maintainer sets it before the bench runs.

## Migration

The work is steps of #1114. It is not a new migration. Each step is one pull request and ends
with the old path deleted.

1. Move the value types, `Builtin` and `DataComponentType` to `mcrs_minecraft_item_component`, and
   make their fields `pub`. This is a `refactor` and changes no behaviour.
2. Add `ItemTemplate`, the normal `ItemStack`, `Prototypes`, `StackView` and the comparisons.
   Nested values hold `ItemTemplate`. Delete `ItemStack::mergeable` and `ItemTable::effective`.
   This changes the merge check (problem 2), so it is a `fix`.
3. Move the codecs to the three codec crates, one pull request for each.
4. In `mcrs_minecraft_engine_item`, replace the `u128` and `MAX_ROWS` with `RowMask`. This step
   can run in parallel with the others.
5. Step 4 of #1114 moves `mcrs_minecraft_block_entity`, `mcrs_minecraft_vanilla_inventory` and the
   loot tables onto the new types. Its loot pull request adds `StackDraft` and deletes
   `WorkStack`.
6. Move the predicate crate onto `StackView` with #1108. That pull request deletes
   `ComponentHolder` and `ItemSubject`.
7. The bench of the live stack runs beside these steps.

The server half of the open steps of #1114 finishes on the new types: the server crates of step
3e (`protocol_game`, `replication`, `inventory_server`, `vanilla_item_entity`,
`engine_registries`), then 3f and 3g. The client half of step 3e (`client`, `client_entity`,
`entity_visual`, `inventory_client`) waits for the rewrite of the client GUI in pull request #1159.

- The pull request of the client half of step 3e deletes `ProtoStack`, `DelimitedStack`, the old
  hashed stack and the last bytes bridges.
- Step 5 of #1114 deletes the first item model of `mcrs_minecraft_item`: the component types,
  `patch.rs` and `stack.rs`. It needs the client half of step 3e and #1108, because
  `mcrs_minecraft_predicate` still depends on `mcrs_minecraft_item`.
- `mcrs_minecraft_item::hash_ops` goes in the pull request that gives the last type without a
  model a reader in the stack crates and moves its recorded hash vectors there, as #1114 states.

### What happens to `docs/item-design.md`

The pull request that publishes this RFD deletes `docs/item-design.md`. It moves each part that is
still in force:

| Section of the note | Destination |
|-|-|
| 2, state classification | a new page "Items" in the section "Concepts" of the site |
| 3, crate map | this RFD, section "Crates" |
| 5, usage | the new page "Items" |
| 6, storage contract and the write rule | the new page "Items", while the entity form stays |
| 7, writers, the hooks of `InSlot`, what does not panic | the new page "Items" |
| 8, removal of the first model | this RFD, section "Migration", and #1114 |
| 8.1, how a packet carries a stack | this RFD; the frames move to `mcrs_minecraft_item_stack_wire` |

The same pull request changes the references in `docs/README.md`, `docs/inventory-design.md` and
`docs/spec/item-entity.md`.

## Options

These options were considered.

- **Keep one `ItemStack` for stacks and templates, with a flag.** It is less code. But each
  operation must check the flag, and a template with entries without effect can reach a slot. A
  type cannot state the rule of STK-TPL-007.
- **Normalize at the edges only**, on read and on write. Today's `set` keeps working. But each
  comparison must normalize first or compare through the prototype, and that is the defect of
  problem 2.
- **Unresolved entries inside `ItemStack`** for a reader with no table. They make an invalid state
  possible. `docs/item-design.md` rejected them, and this RFD keeps that.
- **A shared pointer to the patch**, so that a copy is cheap for each patch. It breaks #566. A
  snapshot compares in place or by digest instead.
- **Raise `MAX_ROWS` to 256 with a `u256` from a crate.** It adds a dependency for one array of
  four words.
- **A count of `u32` in `ItemStack`.** It makes a count above 99 possible in a slot, and that has
  no saved form (STK-CNT-004).
- **One codec trait in the stack crate for all forms.** A proxy then builds the saved and the
  hash code. Each codec crate owning its trait keeps each form optional.

## Open questions

1. The form of a live stack: an ECS entity or a value. The bench of the section above decides.
   The maintainer sets its threshold.
2. The place of `Prototype` and `Prototypes` in `mcrs_minecraft_item_stack`, and of
   `ItemTemplate` in `mcrs_minecraft_item_component`, is proposed, not decided.
3. Does a draft hold an `ItemTemplate` with a count that has no effect, or a type with an item and
   a patch only? The first reuses the template codecs. The second has no unused field.
4. Does `Removed` move to `RowMask` in the same step as the row mask?
5. Is a 64-bit digest acceptable for each snapshot that cannot borrow its stack?
6. The rules of `exact_ignoring` and `subset` need a specification before code. Which document
   owns them?
7. U6: is the proposed limit to 99 at the edge of a slot the answer for trades?

## What would reopen this

- The bench shows that the entity form costs more than the threshold. The model stays, and the
  entity form and `RowMask` go.
- A consumer needs a template in a slot. The two-type split then is wrong.
- A normalization on each `set` shows up in a profile of a hot path, for example the damage of a
  tool on each use.

## Objections

The review pass of RFD 1 has not run on this text. Its objections go here before the state moves
to discussion.

## References

- `docs/spec/item-stack.md`: the rules of a stack.
- `docs/item-design.md`: the note that this RFD supersedes.
- Issue #1114: one item stack model.
- Issue #566: library code without shared ownership, locks, trait objects or hot-path allocation.
- Issue #1108: one convention for a kernel that reads the world.
- Pull request #1159: the rewrite of the client GUI.
