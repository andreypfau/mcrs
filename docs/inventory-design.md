# Player Inventory Design

This document fixes the crate map and the public API of the player inventory. It contains no
implementation. The behaviour is in `spec/inventory.md` (`INV-*`), `spec/inventory-transactions.md`
(`ITX-*`) and `spec/item-stack.md` (`STK-*`). This document refers to the rules by id and does not
state them again. `docs/item-design.md` fixes the stack model; this document builds on it.

A line that has the mark **(proposal)** is a choice of this document. A line that has the mark
**(decided)** is a decision of the owner and is fixed.

# 1. Purpose and scope

The player inventory works today for one consumer: the server with the Java Edition sync. This
document makes each part a crate that a user takes alone. The users are:

| User | Needs |
|-|-|
| proxy | the wire codecs of a stack and of the inventory packets; no Bevy |
| bot (headless client) | local slots, the prediction of a click, the click encoder |
| minigame server | kits, a virtual chest menu with button entries, locked slots, snapshot and restore, no craft |
| world map, save viewer | the inventory of a player file; no Bevy |
| anti-cheat, analytics | the intents of a player and the result of each request, read only |
| Bedrock edge | stack requests as `Request`; stack network ids as state of one connection |

Terms. Each term has one meaning in this document.

- **Holder**: an entity that has `Slots` (`docs/item-design.md`).
- **Player holders**: the four holders of INV-SLT-002: the player inventory, the cursor, the craft
  grid and the craft result.
- **Menu layout**: a value that maps each menu entry to a slot of a menu part.
- **Menu part**: one holder of a menu. The inventory menu has three parts.
- **Menu**: an entity that has a menu layout, its menu parts and one viewer.
- **Open menu**: the last menu of a viewer. Each player always has the inventory menu.
- **Viewer**: the player that sees a menu.
- **Edge**: the code that maps one edition of the protocol to requests and intents.
- **Client view**: what the edge knows about the stacks that the client holds (INV-SYN-002).
- **Gate**: the take filter or the place filter of one slot (ITX-LIM-004, ITX-LIM-005).

Decisions of the owner **(decided)**:

- The cursor is a slot of the player. It is a holder with one slot (ITX-MOD-002).
- The `clear` command empties the cursor slot also.
- The Java data pack is the canonical model. The Bedrock edge maps to it; no crate below the
  Bedrock edge knows Bedrock.
- The game rule `mcrs:spectators_open_menus` has the default value true.
- The off-hand slot has no take filter and no place filter in each edition (INV-SLT-005).

In scope: the change source of a slot, the split of `mcrs_minecraft_vanilla_inventory`, the slot
policy and the cancel point, the menu as data, the player file crate, the request ids, and the
gameplay gaps. Out of scope: the craft (#208 to #212), the menu of each block (#63),
the death drop, the `set_equipment` packet. Each has a later specification.

# 2. State classification

| Stored thing | Class | Owner or writer |
|-|-|-|
| `Slots`, `InSlot`, `Count`, item components | as `docs/item-design.md` section 2 | as that section |
| `SlotPolicy` of a holder | authoritative truth | the plugin that owns the holder; one writer for each slot entry |
| gate table (`SlotGates`) | authoritative truth, immutable after the plugin build | `register_slot_gate` |
| request source counter | authoritative truth, immutable after the plugin build | `allocate_request_source` |
| `PlayerHolders` | authoritative truth: the three child holders of a player | the system that gives a new player its holders |
| `Selected` | authoritative truth | the `select_slot` command; a system of a plugin |
| `SlotChanged` | temporal fact: a slot got a different content | the `InSlot` hooks and the engine commands |
| `RequestCheck`, `RequestResult`, `RequestDrop`, `Given` | temporal fact | the command that does the action |
| `InventoryInput` | temporal fact: one decoded packet | the packet observers of the Java edge |
| `MenuSync` of a menu: state id and client view | authoritative truth of the Java edge; no other state can give it | the Java edge only |
| container id of a menu | authoritative truth of the Java edge | the Java edge only |
| sent selected slot | authoritative truth of the Java edge: the slot that the client has | the Java edge only |
| `MenuDrag` of a menu: the drag in progress | authoritative truth of the click driver | `MenuClicks` of `engine_inventory_click`; `MenuClickPlugin` inserts and resets it |
| stack network ids | authoritative truth of the Bedrock edge, for one connection | the Bedrock edge only |
| `PlayerItems` | a value; a snapshot that its owner keeps | the reader that makes it |
| values of a player file | authoritative truth while the player has no entity | the save |

Cost models of the materialized things:

- No reverse index from a holder to its menus exists. Section 6 states why the recommended sync
  does not need it.

`SlotChanged` is a change list, not a second copy of state. A consumer that has no stored baseline
needs it: equipment broadcast, analytics, a comparator, a save mark. A consumer that has a baseline
(the client view) compares and does not need it.

# 3. Crate map

The layer numbers are those of `CLAUDE.md`. Each name has the prefix `mcrs_minecraft_`.

| Crate | Layer | Bevy | Status | Owns |
|-|-|-|-|-|
| `inventory_transaction` | 3, rules, `no_std` | none | no change | the planner of an atomic request |
| `inventory_player` | 3, rules, `no_std` | none | changed | the 41 slots, the vanilla slot rules as data, the insert rule, the close moves |
| `inventory_click` | 3, rules, `no_std` | none | no change | Java click kinds to operations; the traits `Menu` and `Slots` |
| `inventory_menu` | 3, data | feature `bevy`: derives only | changed | `MenuEntry`, the layout of the inventory menu |
| `menu_key` | 3, data | none | new | `MenuType` and the keys of the menu registry |
| `inventory_client_view` | 3 | none | changed | `StateId`, `ClientView` |
| `inventory_file` | 3, data | none | new | `PlayerItems`, the inventory keys of a player file |
| `engine_item` | 4 | required | changed | stacks as entities, `SlotChanged`, `SlotPolicy`, gates, `RequestCheck`, request sources |
| `player_holders` | 5 | required | new | the player holders, the vanilla slot policy of the player holders, the restore of their stacks |
| `vanilla_inventory` | 5 | required | changed | gameplay: close, hand actions, creative, drops, persistence glue |
| `inventory_server` | 5, edge | required | new | packet decode, click to request, `MenuSync`, the sync, `set_held_slot` |

Notes:

- `inventory_file` cannot be `no_std` now.
- Each crate without Bevy goes in `NO_BEVY_PACKAGES` of `xtask/src/checks.rs`.

Dependency rules:

- `vanilla_inventory` depends on no packet crate, no network crate, `inventory_click` or
  `inventory_client_view`. It builds and runs in a test app without them.
- `inventory_server` depends on `vanilla_inventory` and the packet crates. Nothing
  depends on `inventory_server` except an application.
- `inventory_menu` no longer names `Holder` of `inventory_player` in an entry. It keeps `EquipSlot` for a route.
- `engine_item` knows no player, no menu and no edition.

Section 5 names the crates that each user takes.

# 4. Public names

Each item below is `pub`. All other items are `pub(crate)`. Each subsection gives the crate, the
layer and one contract line for each name.

## 4.1 `mcrs_minecraft_engine_item` (layer 4), additions

```rust
#[derive(Message)]
pub struct SlotChanged { pub slot: SlotRef }

pub enum Gate { Permits, Refuses, Ask(GateId) }
pub struct GateId(/* private */);
pub struct GateQuery { pub slot: SlotRef, pub actor: Entity, pub stack: Entity }
pub type GateFn = fn(&World, &GateQuery) -> Filter;
pub struct SlotAccess { pub limit: u8, pub take: Gate, pub place: Gate }
pub struct SlotPolicy { /* private */ }
impl SlotPolicy {
    pub fn uniform(access: SlotAccess) -> Self;
    pub fn locked() -> Self;
    pub fn access(&self, index: u16) -> SlotAccess;
    pub fn set(&mut self, index: u16, access: SlotAccess);
}

pub struct RequestSource(/* private */);
pub struct RequestId { pub source: RequestSource, pub serial: u32 }
pub enum RequestKind { Action, Close }
impl Request {
    pub fn close(id: RequestId) -> Self;
    pub fn kind(&self) -> RequestKind;
    pub fn id(&self) -> RequestId;
    pub fn ops(&self) -> impl Iterator<Item = Op<SlotRef, Entity>> + '_;
}
#[derive(EntityEvent)]
pub struct RequestCheck { pub actor: Entity, /* private: the request, the verdict */ }
impl RequestCheck {
    pub fn request(&self) -> &Request;
    pub fn deny(&mut self, denial: Denial);
}
pub struct Denial { pub source: RequestSource, pub code: u16 }
pub enum Refusal { Rejected(Rejected), Denied(Denial) }
pub struct RequestResult { pub holder: Entity, pub id: RequestId, pub outcome: Result<(), Refusal> }

pub enum RegisterError { DuplicateKey, MaskFull, UnsupportedStorage, GateTableFull, SourcesExhausted }
pub trait ItemAppExt {
    fn register_slot_gate(&mut self, gate: GateFn) -> Result<GateId, RegisterError>;
    fn allocate_request_source(&mut self) -> Result<RequestSource, RegisterError>;
}
pub trait StackCommandsExt {
    fn touch(&mut self) -> &mut Self;
}
```

| Name | Contract |
|-|-|
| `SlotChanged` | The slot holds a different stack, a different count or a different component value than before the command. |
| `Gate`, `GateId` | The answer of one filter: a constant, or a question to a function that `register_slot_gate` added. |
| `GateQuery` | The inputs of a gate: the slot, the actor, the stack (ITX-LIM-006). The actor is the entity that made the request. |
| `GateFn` | Reads the world and gives `Filter`. It gives the same answer for each operation of one request. |
| `SlotAccess` | The slot limit in `1..=99` and the two gates of one slot. `limit` out of range clamps. |
| `SlotPolicy` | A component of a holder: one `SlotAccess` for each slot. A holder without it has the limit 99 and permits. |
| `SlotPolicy::locked` | Each slot refuses each take and each place. |
| `SlotPolicy::set` | Changes one entry. An index without an entry changes nothing. |
| `RequestSource` | The identity of one sender of requests in one world. |
| `RequestId` | The source and a number that the source selects. The result repeats the two. |
| `RequestKind` | **(proposal)** `Action` for a request of a player or a plugin; `Close` for the moves of a close. The commit triggers no `RequestCheck` for `Close`, so the moves of a close never reach the denial point (ITX-ATM-012). `Request::new` gives `Action`, `Request::close` gives `Close`. |
| `RequestCheck` | Triggered on the actor before the plan of each `Action` request, not on the holders of the slots. An observer reads the request and can deny it. |
| `Denial` | Who denied and a code of that source. The edge renders it. |
| `Refusal` | The cause of a request that changed nothing: a rejection of the planner or a denial. |
| `RegisterError` | Two new variants: `GateTableFull` and `SourcesExhausted`. The other three exist today. |
| `register_slot_gate` | Adds a function in the plugin build. More than 65,535 gates give `RegisterError::GateTableFull`. |
| `allocate_request_source` | Gives the next source in the plugin build. An overflow gives `RegisterError::SourcesExhausted`. |
| `touch` | Writes `SlotChanged` for the slot of the stack. A system that writes a slotted stack in place calls it. |

A plugin that guards a container does not use `RequestCheck`, because the actor is the player.
It puts a `SlotPolicy` on that holder, or adds a global observer that reads the operations of the
request.

`Filter`, `Op`, `Reason` and `Rejected` come from the planner crate. The commit reads `SlotPolicy`
for each slot that a request names and gives the answers to `plan()`. The `set` operation and the
placement commands read no policy (INV-SLT-007).

## 4.4 `mcrs_minecraft_inventory_file` (layer 3)

```rust
pub struct PlayerItems {
    pub slots: [Option<ItemStack>; 41],
    pub cursor: Option<ItemStack>,
    pub craft_grid: [Option<ItemStack>; 4],
    pub selected: u8,
}
pub struct PlayerInventoryFile { pub items: PlayerItems, pub kept_equipment: NbtCompound }
pub enum FileError { NotACompound, BadShape(&'static str), Stack { key: &'static str, cause: SavedError } }
pub fn read<L: NameLookup>(player: &NbtCompound, names: &L) -> Result<PlayerInventoryFile, FileError>;
pub fn write<L: NameLookup>(file: &PlayerInventoryFile, names: &L, player: &mut NbtCompound)
    -> Result<(), FileError>;
```

| Name | Contract |
|-|-|
| `PlayerItems` | The stacks of a player as values, in the slot order of INV-SLT-001, and the selected slot. |
| `PlayerInventoryFile` | `PlayerItems` and the keys of `equipment` that hold no slot of the player inventory. |
| `read` | Reads `Inventory`, `SelectedItemSlot` and `equipment` (INV-SAV-001, INV-SAV-002). The cursor and the craft grid come back empty. |
| `write` | Writes the three keys. It writes no cursor stack and no craft grid stack. |
| `FileError` | Each variant has `Display`. A caller can match on it. |

`PlayerItems` is also the snapshot type of section 4.5. The carry of a cell crossing keeps its
codec in `vanilla_inventory`, because only the server reads it.

## 4.5 `mcrs_minecraft_vanilla_inventory` (layer 5), gameplay

```rust
pub struct VanillaInventoryPlugin;
pub struct InventoryPersistencePlugin;
pub enum InventorySet { Input, Drops }
pub enum HandAction { DropOne, DropStack, SwapHands }
pub trait PlayerInventoryCommandsExt {                   // for `EntityCommands` of a player
    fn hand_action(&mut self, action: HandAction) -> &mut Self;
    fn select_slot(&mut self, slot: u8) -> &mut Self;
    fn creative_set(&mut self, slot: SlotRef, stack: Option<ItemStack>) -> &mut Self;
    fn creative_drop(&mut self, stack: ItemStack) -> &mut Self;
    fn clear_items(&mut self) -> &mut Self;
    fn restore_items(&mut self, items: PlayerItems) -> &mut Self;
}
#[derive(SystemParam)] pub struct PlayerInventories { /* reads the player holders and their stacks */ }
impl PlayerInventories<'_, '_> {
    pub fn capture(&mut self, player: Entity) -> Option<PlayerItems>;
}
#[derive(Resource)] pub struct VanillaInventoryRequests { pub source: RequestSource }
```

| Name | Contract |
|-|-|
| `VanillaInventoryPlugin` | Gives each new player its holders and their `SlotPolicy`. At the player spawn, it opens the inventory menu. It launches each dropped stack. |
| `InventoryPersistencePlugin` | Bevy glue over `inventory_file`. It replaces `PlayerItemFilePlugin`; an app adds one of the two. |
| `InventorySet::Input` | An edge applies the intents of the players here, in the order of arrival. It is in `TickSet::ClientInput`. |
| `InventorySet::Drops` | Makes an item entity from each `RequestDrop` of a player holder. |
| `hand_action` | The request of INV-SEL-004 or INV-SEL-005. A spectator and an empty hand give no request. |
| `select_slot` | Sets `Selected` for a value in `0..=8`. It ignores each other value (INV-SEL-002). |
| `creative_set`, `creative_drop` | The rules INV-CRE-003 and INV-CRE-004. They do nothing for a player that is not in creative mode. |
| `clear_items` | Despawns each stack of the four player holders, the cursor included. |
| `restore_items` | Replaces the stacks of the player inventory, the cursor and the craft grid, and sets `Selected`. |
| `PlayerInventories::capture` | The stacks of the player as values, or `None` for an entity that is no player. |
| `VanillaInventoryRequests` | The request source of this plugin. A reader of `RequestResult` filters on it. |

An observer of `VanillaInventoryPlugin` does the steps of INV-CLS-001 for each `MenuClosed`. The
cursor stack enters the player inventory by the insert rule. When the closed menu is the first menu,
the craft grid does the same. At a leave and at a dimension change, the observer does the steps for
the open menu and then for the first menu (INV-CLS-003).

`VanillaInventoryPlugin` reads the game rule of section 1 from the game rules of the world.
`RequestPurpose` names the purpose of each request of the plugin and converts it to and from
the serial of `RequestId`.

## 4.6 `mcrs_minecraft_inventory_server` (layer 5, edge)

```rust
pub struct InventoryServerPlugin;
pub enum InventoryServerSet { Sync }
#[derive(Message)] #[non_exhaustive] pub enum InventoryInput { Click { .. }, Close { .. }, Hand { .. }, CreativeSet { .. }, SelectSlot { .. } }
pub struct ClaimedSlot { pub index: u16, pub view: SlotView }
#[derive(Component)] pub struct MenuSync { /* private */ }
impl MenuSync {
    pub fn state_id(&self) -> StateId;
    pub fn view(&self) -> &ClientView;
    pub fn container_id(&self) -> u8;
}
```

| Name | Contract |
|-|-|
| `InventoryServerPlugin` | Decodes the inventory packets, applies them in `InventorySet::Input`, and syncs each open menu. |
| `InventoryServerSet::Sync` | The comparison and the packets. After `TickSet::Project`, before `TickSet::Output`. |
| `InventoryInput` | One decoded inventory packet with its Java fields. It is today's `InventoryInput`. Read only for a user. |
| `ClaimedSlot` | A field type of `InventoryInput::Click`: the menu entry and the client view that the click gives for it. |
| `MenuSync` | A component of a menu: the state id, the client view and the container id of that menu. |

The edge owns `MenuSync` and the sent selected slot. No gameplay crate reads them.

## 4.7 `mcrs_minecraft_player_holders` (layer 5)

`docs/inventory-client-design.md` section 4.2 gives the public names of this crate. It owns
`PlayerHolders` and the slot policy of the player holders. It links no server-only crate.

# 5. Usage

Each sketch names the crates and the names that the use case needs.

**Proxy.** Crates: `item_stack`, the protocol crates. It depends on no inventory crate.

**Minigame: kit.** Crates: `engine_item`, `vanilla_inventory`. `give` adds one stack.
`restore_items(kit.clone())` replaces all stacks by a `PlayerItems` value.

**Minigame: locked slot.** Crate: `engine_item`. The plugin changes one entry of the policy:
`policy.set(8, SlotAccess { limit: 99, take: Gate::Refuses, place: Gate::Refuses })`.

**Minigame: veto.** Crate: `engine_item`. An observer denies a request of a frozen player:

```rust
fn freeze(mut check: On<RequestCheck>, frozen: Query<(), With<Frozen>>, source: Res<ArenaSource>) {
    if frozen.contains(check.actor) { check.deny(Denial { source: source.0, code: 1 }); }
}
```

**Minigame: snapshot and restore.** Crate: `vanilla_inventory`. At the arena entry,
`PlayerInventories::capture(player)` gives a `PlayerItems`. At the exit, `restore_items` puts it
back.

**Server without craft.** The application adds `VanillaInventoryPlugin` and no craft plugin. The
craft result stays empty. To refuse the craft grid also, a system inserts `SlotPolicy::locked()`
on `PlayerHolders::craft_grid`.

**Save viewer.** Crates: `inventory_file`, `item_table`, the NBT crate. No Bevy.

```rust
let file = inventory_file::read(&player_compound, &table)?;
for (index, stack) in file.items.slots.iter().enumerate() { /* draw the slot */ }
```

**Anti-cheat and analytics.** One system has a `MessageReader` for each of `InventoryInput`,
`RequestResult`, `RequestDrop` and `SlotChanged`. It writes nothing.

**Bedrock edge.** Crates: `engine_item`, `vanilla_inventory`. The edge allocates
one `RequestSource`. One stack request is one `Request` (ITX-BED-002):

```rust
let id = RequestId { source: bedrock.source, serial: packet_request_id };
let request = Request::new(id).split(from, to, amount);         // `Take`, `Place` (ITX-BED-001)
commands.entity(player).request(request);
```

The edge keeps the stack network ids in a component of the connection (ITX-BED-003). It reads
`RequestResult` with its own source and sends the answer. It reads `SlotChanged` to give a new
network id to each changed slot.

**Commands `give`, `clear`, `item`.** Crate: `mcrs_minecraft_command_vanilla` over `give`,
`clear_items`, `put_at` and `load_inventory`. They need a specification first.

# 6. The tick: input, cancel point, change source, sync

## 6.1 Phases

| Phase | Set | What runs |
|-|-|-|
| ingest | observers of the edge | Each inventory packet becomes one `InventoryInput`. |
| input | `InventorySet::Input` | The edge queues one command for each input, in the order of arrival (ITX-MOD-005). |
| commit | the command flush of that set | Each command reads the slots after the command before it (ITX-JAV-003). |
| simulate | `TickSet` of the engine | Plugins write slots through commands. |
| drops | `InventorySet::Drops` | Each `RequestDrop` of a player holder becomes an item entity. |
| sync | `InventoryServerSet::Sync` | The edge compares each open menu and writes the packets. |

A click command of the edge does three things. It reads the open menu of the viewer and the slots
of its parts. It runs the click kernel. It queues the engine request.

The other inputs call the commands of `vanilla_inventory`: `close_menu`,
`hand_action`, `select_slot`, `creative_set`, `creative_drop`. A Bedrock edge calls the same
commands. Thus the gameplay has one path for each edition. A close of the first menu also triggers
`MenuClosed`, so the observer of `vanilla_inventory` returns the cursor and the craft grid.

The hand swap: an edge reader of `RequestResult` filters on the source of `VanillaInventoryRequests`
and the serial of the hand swap. After a rejection, it makes the client views of the two slots not
known (INV-SEL-006). The comparison then sends the two stacks. `forget_rejected_swap.rs` does this
today. An exchange of the client views at decode cannot give this result: two equal stacks would
cause no send.

## 6.2 The cancel point

Two levels exist. Data decides the regular case; code decides the rare case.

1. **`SlotPolicy`**: the limit and the gates of each slot. The commit reads it. A locked slot is
   two `Gate::Refuses` values and costs no function call.
2. **`RequestCheck`**: the commit triggers it on the actor before the plan of each `Action`
   request. An observer denies the request. The result is `Refusal::Denied`, and no slot
   changes. The observer of a close makes `Close` requests (section 4.1). The commit triggers no
   `RequestCheck` for them, so the moves of a close never reach this point (ITX-ATM-012).

The check is an observer, not a system set (decision of the owner). Reason: a request of one player must
commit before the next input of that player reads the slots (ITX-JAV-003). A system set between
"all requests of the tick are made" and "all are committed" cannot keep this order.

A gate is a `fn` pointer in a table. It is not a trait object. `GateQuery` gives it the three
inputs that ITX-LIM-006 permits. A gate that reads other state breaks the contract of that rule.
State that the gate derives from the registries, such as the entity type tags of INV-SLT-003, is
not other state: it does not change during a request.

`PlayerHoldersPlugin` of `player_holders` registers the armour place gate (INV-SLT-003). Thus the
server and the client use the same gate. The armour take gate (INV-SLT-004) does not exist yet,
because it needs the game mode of the player, and only the server engine keeps the game mode. Until
then, the take filter of each armour slot is `Gate::Permits`. The off-hand slot has no gate
(INV-SLT-005). `player_holders` builds the policy of the player inventory from `Holder::slot` of
`inventory_player`. The craft result has `place: Gate::Refuses` (INV-SLT-006).

## 6.3 The change source

Every engine path that changes the content of a slot writes `SlotChanged`:

- the hooks of `InSlot`: a stack enters or leaves a slot (`put_at`, `put_in`, `swap_with`,
  `drop_at`, a despawn, `load_inventory`);
- the commit of a request, for each entry of the plan;
- `give` and `give_or_drop` on a holder;
- `consume`, `set`, `unset` and `reset` on a stack that has `InSlot`;
- `touch`, for a system that writes `Count` or an item component in place.

A consumer removes repeats in its own `Local`. The message costs nothing when no slot changes.

## 6.4 The sync

The Java edge compares each open menu one time in each tick (INV-SYN-003). For each menu that
has `MenuSync`:

1. For each entry, read the stack of the slot through `Slots`.
2. Compare it with the client view in place: the kind, the count and the row hash. The comparison
   builds no `ItemStack`.
3. For each mismatch, build the value one time, send the slot packet and store the sent stack in
   the client view.
4. Do the same for the cursor.

The system runs when at least one menu has `MenuSync`. Its cost is viewers × entries of the open
menu. It needs no reverse index from a holder to its menus, and it finds each write: a command, a
`Query<&mut Count>`, a component write, a despawn. `SlotChanged` is not an input of this sync. The
other consumers of section 2 read it.

The comparison reads only the open menu (INV-SYN-006). When a menu closes, the edge copies the
client view of each shared slot to the first menu of the viewer (INV-SYN-007). It finds the shared
slots of the two menus.

The same system keeps the sent selected slot. When `Selected` differs from it and no
`set_carried_item` packet gave the value, the edge sends `set_held_slot` (INV-SEL-003).

# 7. Writers and invariants

## 7.1 Writers

Section 2 names the writer of each stored thing. These rules add to it:

- A player changes a slot only through a request. An edge never calls a placement command for a
  player action. The `set` operation is the one exception (INV-CRE-001, INV-CRE-005).
- A plugin changes a slot through a command. A system that writes a slotted stack in place calls
  `touch`.
- `SlotPolicy` has one writer for each entry. `player_holders` writes the entries of the player
  holders at the spawn. The entry of the cursor
  holder always permits each take and each place (ITX-LIM-004, ITX-LIM-005). By this rule, no
  other plugin writes it. Another plugin can
  write an entry only if `player_holders` never writes it after the spawn.
- Only the edge writes `MenuSync`.
- A menu never owns a stack. The despawn of a menu changes no slot.
- `PlayerHolders` does not change after the spawn of the player.
- A request source is allocated in the plugin build. Each dimension world builds the same plugins
  in the same order, so a plugin has the same source in each world.

## 7.2 Invariants

1. After each command flush, a slot content that differs from the content before the flush has a
   `SlotChanged`, except for a write in place without `touch`.
2. A request applies in full or not at all (ITX-ATM-002). A denial is "not at all".
3. Each `Request` has exactly one `RequestResult`. `RequestCheck` comes before the plan of each
   `Action` request. A `Close` request has no `RequestCheck` (ITX-ATM-012). Each `RequestDrop`
   comes before the result.
4. A viewer has one or two menus. The first one is the inventory menu and lives as long as the
   player.
5. After the sync of a tick, the client view of each entry of the open menu equals the stack of
   its slot. This holds for each writer.
6. A gate gives the same answer for each operation of one request (ITX-LIM-006).

## 7.3 What does not panic

| Input that can be wrong | Result |
|-|-|
| a `SlotPolicy` shorter than the `Slots` of its holder | a slot without an entry has the limit 99 and permits |
| a `GateId` that the table does not have | the gate refuses |
| `MenuEntry` with a part or an index out of range | the slot is "no such slot"; the click does nothing |
| a player file with a bad shape or a stack that does not decode | `FileError`; no slot changes |
