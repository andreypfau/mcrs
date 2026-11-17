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
gameplay gaps of section 1.1. Out of scope: the craft (#208 to #212), the menu of each block (#63),
the death drop, the `set_equipment` packet. Each has a later specification (INV section 9).

## 1.1 What the code has today

The line numbers are those of the commit `9ceac16cd`.

1. Two stack models exist. 27 crates depend on the old crate `mcrs_minecraft_item`. A stack
   crosses the wire through a byte round trip (`sent_stack.rs:23`, `decode_click.rs:64`).
2. A slot write outside a click reaches the client only at the next click or give
   (`menu_queue.rs:7`).
3. `mcrs_minecraft_vanilla_inventory` mixes gameplay, the Java edge and persistence.
   `InventoryInput` carries Java fields.
4. The engine commit reads no filter: each slot has the limit 99 and permits each take and each
   place (`engine_item/src/commit.rs:113`). Only the click path reads `SlotFilter`. Thus the hand
   swap of INV-SEL-005 ignores the place filter of the off hand, and no plugin can lock a slot.
5. A menu is an implementation of the trait `inventory_click::Menu`. `InventoryMenu` is the only
   menu, and `MenuSlot` names a player holder.
6. The player file codec is in two Bevy crates: `vanilla_inventory/src/player_file_codec.rs` and
   `engine_item/src/persist.rs`.
7. Each `RequestId` is a constant of vanilla code (`request_ids.rs`). Two plugins can use one id.
8. Armour slots refuse each place (`click_slots.rs:46`). The off-hand rule is fixed to `AnyItem`.
   No code reads `mcrs:spectators_open_menus`. No system sends `set_held_slot` when the server
   writes `Selected` (INV-SEL-003). The commands `clear`, `give` and `item` do not exist.
9. Of the experiments of INV section 12 and ITX section 11, only E3, E8, E9 and E11 of INV and E9
   of ITX have a recorded result.
10. Four more ceilings are in `sync_menus.rs`, `close_container.rs`,
    `engine_item/src/request.rs` and `engine_item/src/spawn_stack.rs`.

# 2. State classification

| Stored thing | Class | Owner or writer |
|-|-|-|
| `Slots`, `InSlot`, `Count`, item components | as `docs/item-design.md` section 2 | as that section |
| `SlotPolicy` of a holder | authoritative truth | the plugin that owns the holder; one writer for each slot entry |
| gate table (`SlotGates`) | authoritative truth, immutable after the plugin build | `register_slot_gate` |
| request source counter | authoritative truth, immutable after the plugin build | `allocate_request_source` |
| `PlayerHolders` | authoritative truth: the three child holders of a player | the system that gives a new player its holders |
| `Selected` | authoritative truth | the `select_slot` command; a system of a plugin |
| `MenuLayout` of a menu | authoritative truth, immutable for the life of the menu | the `open_menu` command |
| `MenuParts` of a menu | authoritative truth, immutable for the life of the menu | the `open_menu` command |
| `MenuButtons` of a menu | authoritative truth, immutable for the life of the menu | the `open_menu` command |
| first menu of a player | authoritative truth: the inventory menu, as long as the player lives | `VanillaInventoryPlugin`, with `open_menu` at the player spawn |
| `MenuOf` on a menu | authoritative truth of the viewer relation | the `open_menu` and `close_menu` commands |
| `Menus` on a viewer | materialized: the ordered list of the menus of the viewer | the relationship hooks of `MenuOf` |
| open menu | derived: the last entry of `Menus`; not stored | none |
| `SlotChanged` | temporal fact: a slot got a different content | the `InSlot` hooks and the engine commands |
| `RequestCheck`, `RequestResult`, `RequestDrop`, `Given` | temporal fact | the command that does the action |
| `MenuOpened`, `MenuClosed`, `EntryPressed` | temporal fact | the menu commands; the edge for `EntryPressed` |
| `InventoryInput` | temporal fact: one decoded packet | the packet observers of the Java edge |
| `MenuSync` of a menu: state id and client view | authoritative truth of the Java edge; no other state can give it | the Java edge only |
| container id of a menu | authoritative truth of the Java edge | the Java edge only |
| sent selected slot | authoritative truth of the Java edge: the slot that the client has | the Java edge only |
| drag state of a viewer (`ClickDrag`) | authoritative truth of the Java edge | the Java edge only |
| stack network ids | authoritative truth of the Bedrock edge, for one connection | the Bedrock edge only |
| `PlayerItems` | a value; a snapshot that its owner keeps | the reader that makes it |
| values of a player file | authoritative truth while the player has no entity | the save |

Cost models of the materialized things:

- `Menus` makes "the open menu of this player" one read. A menu opens and closes at the rate of
  player input, so the hook cost is small.
- No reverse index from a holder to its menus exists. Section 6 states why the recommended sync
  does not need it.

`SlotChanged` is a change list, not a second copy of state. A consumer that has no stored baseline
needs it: equipment broadcast, analytics, a comparator, a save mark. A consumer that has a baseline
(the client view) compares and does not need it.

# 3. Crate map

The owner chose the crate names. Step 0 gives four crates of today a new name:
`inventory_plan` becomes `inventory_transaction`, `player_slots` becomes `inventory_player`,
`inventory_view` becomes `inventory_client_view`, and `menu` becomes `inventory_menu`. This
document uses the new names for the code of today too. The layer numbers are those of `CLAUDE.md`. Each name has
the prefix `mcrs_minecraft_`.

| Crate | Layer | Bevy | Status | Owns |
|-|-|-|-|-|
| `inventory_transaction` | 3, rules, `no_std` | none | no change | the planner of an atomic request |
| `inventory_player` | 3, rules, `no_std` | none | changed | the 41 slots, the vanilla slot rules as data, the insert rule, the close moves |
| `inventory_click` | 3, rules, `no_std` | none | no change | Java click kinds to operations; the traits `Menu` and `Slots` |
| `inventory_menu` | 3, data | feature `bevy`: derives only | changed | menu registry keys, `MenuLayout`, `MenuEntry`, the layout of the inventory menu |
| `inventory_client_view` | 3 | none | changed | `StateId`, `ClientView` |
| `inventory_file` | 3, data | none | new | `PlayerItems`, the inventory keys of a player file |
| `inventory_predict` | 3, rules | none | new, with #129 | slots as values, the prediction of a click |
| `engine_item` | 4 | required | changed | stacks as entities, `SlotChanged`, `SlotPolicy`, gates, `RequestCheck`, request sources |
| `engine_inventory_menu` | 4 | required | new | the menu entity, the viewer relation, open and close, button entries |
| `vanilla_inventory` | 5 | required | changed | gameplay: player holders, vanilla slot policy, close, hand actions, creative, drops, persistence glue |
| `inventory_server` | 5, edge | required | new | packet decode, click to request, `MenuSync`, the sync, `set_held_slot` |

Notes:

- `inventory_file` and `inventory_predict` cannot be `no_std` now. Each one states
  `// std: mcrs_minecraft_item_stack`.
- `engine_inventory_menu`, `vanilla_inventory` and `inventory_server` state `// std: mcrs_minecraft_engine`.
- Each crate without Bevy goes in `NO_BEVY_PACKAGES` of `xtask/src/checks.rs`.

Dependency rules:

- `vanilla_inventory` depends on no packet crate, no network crate, `inventory_click` or
  `inventory_client_view`. It builds and runs in a test app without them.
- `inventory_server` depends on `vanilla_inventory`, `engine_inventory_menu` and the packet crates. Nothing
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

## 4.2 `mcrs_minecraft_inventory_menu` (layer 3), changes

```rust
pub struct MenuEntry { pub part: u8, pub index: u16 }
pub enum RouteWhen { Always, Wears { slot: EquipSlot, target: u16 } }
pub struct Route { pub from: MenuRange, pub when: RouteWhen, pub to: MenuRange }
pub struct MenuLayout { /* private */ }
impl MenuLayout {
    pub const INVENTORY: MenuLayout;
    pub fn new(entries: Vec<MenuEntry>, routes: Vec<Route>) -> Option<Self>;
    pub fn len(&self) -> u16;
    pub fn is_empty(&self) -> bool;
    pub fn entry(&self, index: u16) -> Option<MenuEntry>;
    pub fn index_of(&self, entry: MenuEntry) -> Option<u16>;
    pub fn routes(&self, source: u16) -> impl Iterator<Item = Route> + '_;
    pub fn permits_pick_up_all(&self, index: u16) -> bool;
    pub fn permits_drag(&self, index: u16) -> bool;
    pub fn kind(&self) -> Option<MenuType>;
}
```

| Name | Contract |
|-|-|
| `MenuEntry` | The slot `index` of the menu part `part`. It names no entity and no player holder. |
| `Route` | One destination range of a quick move from the entries `from`, in order (ITX-JAV-009). |
| `RouteWhen::Wears` | The route applies when the stack is equippable in `slot` and the entry `target` is empty (INV-MNU-004). |
| `MenuLayout` | The entries, the routes and the two per-entry permissions of one menu. A built-in layout borrows static tables. |
| `MenuLayout::new` | Gives `None` for more than 128 entries or a route out of range. |
| `MenuLayout::INVENTORY` | The 46 entries of INV-MNU-001. Its parts are: 0 player inventory, 1 craft grid, 2 craft result. |
| `MenuLayout::kind` | The menu registry entry for the open packet, or `None` for the inventory menu. |

`InventoryMenu` and `MenuSlot` go away in the pull request that moves their last caller. The trait
`inventory_click::Menu` stays, because the click kernel is generic. It gets one implementation:
a layout with the slots of its parts.

## 4.3 `mcrs_minecraft_engine_inventory_menu` (layer 4)

```rust
pub struct MenuPlugin;
pub const MAX_PARTS: usize = 4;
#[derive(Component)] pub struct MenuParts { /* private: at most MAX_PARTS holders */ }
impl MenuParts {
    pub fn new(holders: impl IntoIterator<Item = Entity>) -> Option<Self>;
    pub fn holder(&self, part: u8) -> Option<Entity>;
    pub fn slot(&self, entry: MenuEntry) -> Option<SlotRef>;
}
#[derive(Component)] pub struct MenuButtons { /* private: one bit for each entry */ }
impl MenuButtons {
    pub fn with(self, entry: u16) -> Self;
    pub fn contains(&self, entry: u16) -> bool;
}
#[derive(Component)] pub struct MenuOf(pub Entity);     // relationship: menu -> viewer
#[derive(Component)] pub struct Menus { /* private */ } // relationship target on the viewer
impl Menus {
    pub fn open(&self) -> Option<Entity>;
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_;
}
pub trait MenuCommandsExt {                              // for `EntityCommands` of a viewer
    fn open_menu(&mut self, layout: MenuLayout, parts: MenuParts, buttons: MenuButtons) -> &mut Self;
    fn close_menu(&mut self) -> &mut Self;
}
#[derive(Message, EntityEvent)] pub struct MenuOpened { pub viewer: Entity, pub menu: Entity }
#[derive(Message, EntityEvent)] pub struct MenuClosed { pub viewer: Entity, pub menu: Entity }
#[derive(Message)] pub struct EntryPressed { pub viewer: Entity, pub menu: Entity, pub entry: u16, pub button: u8 }
```

| Name | Contract |
|-|-|
| `MenuParts` | The holder of each menu part, in the order of the layout. `new` gives `None` for more than `MAX_PARTS`. |
| `MenuParts::slot` | The slot reference of a menu entry, or `None` when the part does not exist. |
| `MenuButtons` | The entries that are buttons. A click on a button moves no stack. |
| `MenuOf`, `Menus` | A Bevy relationship. `Menus` keeps the order in which the menus opened; `open` gives the last one. |
| `MenuOpened`, `MenuClosed` | A menu started or stopped to exist for the viewer. An edge sends its open or close packet from them. |
| `open_menu` | Spawns a menu for the viewer, then writes and triggers `MenuOpened`. A viewer has at most one menu above its first menu; an open menu above the first one closes first. |
| `close_menu` | Writes and triggers `MenuClosed` for the open menu. It then despawns that menu if it is above the first menu. It never despawns the first menu (INV-CLS-002). |
| `EntryPressed` | A viewer clicked a button entry. The edge writes it. |

`MenuPlugin` registers the messages, the relation and one observer. It adds no system. A menu is
one entity for one viewer. Two players that open one chest have two menus over one holder. The
despawn of a viewer despawns its menus. The observer runs when a part holder loses `Slots`. It
closes each open menu that names that holder. No system scans the menus in each tick.

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
#[derive(Component)] pub struct PlayerHolders { pub cursor: Entity, pub craft_grid: Entity, pub craft_result: Entity }
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
| `VanillaInventoryPlugin` | Gives each new player its holders and their `SlotPolicy`. At the player spawn, it calls `open_menu` with `MenuLayout::INVENTORY` and the parts player, craft grid and craft result. It launches each dropped stack. |
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
The serial numbers of its requests are public constants beside `HandAction`.

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

The edge owns `MenuSync`, the drag state and the sent selected slot. No gameplay crate reads them.

## 4.7 `mcrs_minecraft_inventory_predict` (layer 3), with #129

| Name | Contract |
|-|-|
| `LocalMenu` | The stacks that a client holds for one menu, as values: `new(layout)`, `entry`, `cursor`, `set_entry`, `set_cursor`. |
| `LocalMenu::predict` | Applies one `Click` with the click kernel and the planner. It lists the entries that changed. |

The click encoder is one function in the client crate. It takes a `Click`, the list of `predict`
and the state id, and gives the `container_click` packet with hashed stacks. The names of this
crate are fixed with #129, not here.

# 5. Usage

Each sketch names the crates and the names that the use case needs.

**Proxy.** Crates: `item_stack`, the protocol crates. It depends on no inventory crate.

**Bot.** Crates: `inventory_menu`, `inventory_click`, `inventory_transaction`, `inventory_predict`, `item_table`.

```rust
let mut menu = LocalMenu::new(MenuLayout::INVENTORY);
menu.set_entry(36, Some(stone));                           // from `container_set_content`
menu.predict(click, player, &mut drag, &table, &mut changed)?;
let packet = encode_click(click, state_id, &menu, &changed, &table);
```

**Minigame: kit.** Crates: `engine_item`, `vanilla_inventory`. `give` adds one stack.
`restore_items(kit.clone())` replaces all stacks by a `PlayerItems` value.

**Minigame: virtual chest with buttons.** Crates: `engine_item`, `engine_inventory_menu`, `inventory_menu`.

```rust
let shop = commands.spawn((Slots::new(27), SlotPolicy::locked())).id();
commands.entity(icon).put_at(shop, 13);
let parts = MenuParts::new([shop, player])?;
commands.entity(player).open_menu(layout.clone(), parts, MenuButtons::default().with(13));

fn buy(mut pressed: MessageReader<EntryPressed>, mut commands: Commands) {
    for press in pressed.read() { /* give the item to press.viewer */ }
}
```

`layout` is a `MenuLayout` that the plugin builds one time with `MenuLayout::new`. The holder
`shop` is locked, so a quick move from the player inventory finds no room in it.

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
`RequestResult`, `RequestDrop`, `SlotChanged` and `EntryPressed`. It writes nothing.

**Bedrock edge.** Crates: `engine_item`, `engine_inventory_menu`, `vanilla_inventory`. The edge allocates
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
`clear_items`, `put_at` and `load_inventory`. They need a specification first (section 9, point 9).

## 5.1 Examples

**(decided)** Three example packages under `examples/` prove the boundaries of the inventory crates.

| Example | Boundary | Content | Delivered by |
|-|-|-|-|
| `inventory_save_viewer` | no Bevy, no tokio | reads the inventory of a player file | step 2 |
| `inventory_minigame` | no protocol crate, no network crate, no `mcrs_minecraft_inventory_server` | one headless app; each feature is one small plugin of it: a kit with snapshot and restore (step 2), a locked slot and a veto (step 3), a shop menu with button entries (step 4), a request source of its own that reads `RequestResult` (step 6) | steps 2, 3, 4, 6 |
| `inventory_bot_click` | no render crate, no server crate | a headless client predicts a click and encodes the packet | step 8 |

- Each package has an entry in `examples/dependencies.json` with its forbidden dependencies. The
  dependency check of `xtask` enforces the boundary; `examples/README.md` states the mechanism.
- Each boundary has its own package. The check resolves an example with its default features
  only, so a feature gate inside one package proves nothing.
- Each package has one behaviour test binary `tests/it/`. `inventory_minigame` has one case for
  each plugin.
- The proxy use case gets no inventory example. It needs no inventory crate, so the item
  migration gives its proof.

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
of its parts. It runs the click kernel. It queues the engine request. A click on a button entry
moves nothing: the edge writes `EntryPressed` and sets the claimed client views, so that the sync
corrects the prediction of the client.

The other inputs call the commands of `vanilla_inventory` and `engine_inventory_menu`: `close_menu`,
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
"all requests of the tick are made" and "all are committed" cannot keep this order. Section 9,
point 2, states the alternative.

A gate is a `fn` pointer in a table. It is not a trait object. `GateQuery` gives it the three
inputs that ITX-LIM-006 permits. A gate that reads other state breaks the contract of that rule.

`vanilla_inventory` registers two gates: the armour place gate (INV-SLT-003) and the armour take
gate (INV-SLT-004). The off-hand slot has no gate (INV-SLT-005). It builds the policy of the player
inventory from `Holder::slot` of `inventory_player`. The craft result has `place: Gate::Refuses`
(INV-SLT-006).

## 6.3 The change source

Every engine path that changes the content of a slot writes `SlotChanged`:

- the hooks of `InSlot`: a stack enters or leaves a slot (`put_at`, `put_in`, `swap_with`,
  `drop_at`, a despawn, `load_inventory`);
- the commit of a request, for each entry of the plan;
- `give` and `give_or_drop` on a holder;
- `consume`, `set`, `unset` and `reset` on a stack that has `InSlot`;
- `touch`, for a system that writes `Count` or an item component in place.

A consumer removes repeats in its own `Local`. The message costs nothing when no slot changes.

## 6.4 The sync: option (b), recommended

The Java edge compares each open menu one time in each tick (INV-SYN-003). For each menu that
has `MenuSync`:

1. For each entry, read the stack of the slot through `MenuParts` and `Slots`.
2. Compare it with the client view in place: the kind, the count and the row hash. The comparison
   builds no `ItemStack`. This lifts the ceiling of `sync_menus.rs:110`.
3. For each mismatch, build the value one time, send the slot packet and store the sent stack in
   the client view.
4. Do the same for the cursor.

The system runs when at least one menu has `MenuSync`. Its cost is viewers × entries of the open
menu. It needs no reverse index from a holder to its menus, and it finds each write: a command, a
`Query<&mut Count>`, a component write, a despawn. `SlotChanged` is not an input of this sync. The
other consumers of section 2 read it.

The comparison reads only the open menu (INV-SYN-006). When a menu closes, the edge copies the
client view of each shared slot to the first menu of the viewer (INV-SYN-007). It finds the shared
slots through `MenuParts::slot` of the two menus.

The same system keeps the sent selected slot. When `Selected` differs from it and no
`set_carried_item` packet gave the value, the edge sends `set_held_slot` (INV-SEL-003).

## 6.5 The sync: option (a), the alternative

The edge compares only the entries whose slot has a `SlotChanged` in this tick, and the entries
that a click claims. The exact differences from option (b):

| | Option (a) | Option (b) |
|-|-|-|
| cost for each tick | changed slots × menus that show them | viewers × entries |
| input of the sync | `SlotChanged` only | the slots; no message |
| reverse index holder to menus | necessary; one more materialized component with hooks | not necessary |
| a write in place without `touch` | the client does not get it until the next change of that slot | the client gets it in the same tick |
| rule for a user | a slotted stack changes only by a command, or with `touch` | `touch` is necessary only for the consumers of `SlotChanged` |
| agreement with INV-SYN-003 | the same result only if each writer obeys the rule | the rule as written |

The owner chose option (b) (section 9, point 1). With option
(a), step 1 adds the component `ShownIn` on a holder and its hooks, and `docs/item-design.md`
section 2 loses "a system (`Query<&mut Count>`)" as a writer of a slotted stack.

# 7. Writers and invariants

## 7.1 Writers

Section 2 names the writer of each stored thing. These rules add to it:

- A player changes a slot only through a request. An edge never calls a placement command for a
  player action. The `set` operation is the one exception (INV-CRE-001, INV-CRE-005).
- A plugin changes a slot through a command. A system that writes a slotted stack in place calls
  `touch`.
- `SlotPolicy` has one writer for each entry. `vanilla_inventory` writes the entries of the player
  holders at the spawn. The entry of the cursor
  holder always permits each take and each place (ITX-LIM-004, ITX-LIM-005). By this rule, no
  other plugin writes it. Another plugin can
  write an entry only if `vanilla_inventory` never writes it after the spawn.
- Only the edge writes `MenuSync`. Only the menu commands insert or remove `MenuOf`.
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
   its slot. This holds for each writer under option (b).
6. A gate gives the same answer for each operation of one request (ITX-LIM-006).

## 7.3 What does not panic

| Input that can be wrong | Result |
|-|-|
| a `SlotPolicy` shorter than the `Slots` of its holder | a slot without an entry has the limit 99 and permits |
| a `GateId` that the table does not have | the gate refuses |
| `MenuParts` that names a despawned holder | each entry of that part is "no such slot"; the menu closes in the same tick |
| `MenuEntry` with a part or an index out of range | `MenuParts::slot` gives `None`; the click does nothing |
| a player file with a bad shape or a stack that does not decode | `FileError`; no slot changes |

# 8. Budgets

Each number is a budget, not a measurement. A measurement with the real data pack replaces it in
`PERF.md`. An exception to a rule of `CLAUDE.md` needs that number.

| Measure | Budget |
|-|-|
| tick with no inventory input and no viewer | no system of `MenuPlugin`, `VanillaInventoryPlugin` or `InventoryServerPlugin` runs |
| sync, option (b), one viewer, 46 entries, no mismatch | at most 3 µs; no allocation |
| sync, option (b), 1,000 viewers, no mismatch | at most 3 ms for each tick |
| sync, one mismatch | one value built, one packet encoded one time |
| request with a `SlotPolicy` of constants only | the budget of `docs/item-design.md` section 8; no function call |
| `SlotChanged` | one message of 16 bytes for each changed slot; no allocation after the buffer grows |

A hot path that claims zero allocations has a test with the counting allocator
(`mcrs_test_alloc`): the sync without a mismatch, and a request under a policy.

One measurement exists for the code before step 1. The reference machine of `PERF.md` runs the
release profile, and each player is compared in each tick. A player with 36 stacks costs 2.8 µs,
and a player with no stack costs 0.18 µs. Thus 1,000 players cost 2.8 ms and 10,000 players cost
28 ms of the 50 ms tick. Most of that time builds the 46 stack values of `sync_menus.rs:110`. The
comparison makes no allocation for a stack without a component patch. A stack with a patch is not
measured. Step 1 repeats the measurement with the comparison in place and writes it to `PERF.md`.

If the measurement of 1,000 viewers exceeds its budget, the period of the full comparison becomes
a field of `InventoryServerPlugin`. This deviates from INV-SYN-003, and the specification must then
list it.

# 9. Open points

| # | Point | Recommended answer |
|-|-|-|
| 1 | The sync: option (a) or option (b) of section 6. | **Decided: option (b).** It is INV-SYN-003 as written, it finds each writer, and it needs no reverse index. |
| 2 | The cancel point: the observer `RequestCheck`, or a system set `Validate` over pending requests. | **Decided: the observer.** A set needs requests that wait, and a request that waits breaks ITX-JAV-003 for two clicks of one player in one tick. A set is possible only if the input phase runs one schedule pass for each input. |
| 3 | `inventory-transactions.md` has no result for a denial. ITX-ATM-006 knows "applied" and "rejection" only. | Answered by the specification: ITX-ATM-006 to ITX-ATM-012, ITX-JAV-021, ITX-JAV-022. A denial is a third result. The moves of a close never reach the denial point. |
| 4 | The signature of `GateFn`. It reads `&World` inside the commit command. | Keep it if the commit can lend the world while it plans. If not, the gate gets the two `EntityRef` values of the actor and the stack. |
| 5 | Two plugins that write one entry of `SlotPolicy`. | Accept the one-writer rule of section 7.1. A second layer of policy is not built until a user needs it. |
| 6 | The routes and the entries of each block menu, and of a generic chest. No specification states them (INV section 9). | The specification of #63 states them before `MenuLayout` gets a constructor for a chest. A minigame builds its layout with `MenuLayout::new`. |
| 7 | `EntryPressed::button` carries the Java button number. | Keep it until the Bedrock edge exists. Then replace it by an enum of both editions. |
| 8 | The `equippable` component. `item-stack.md` lists it in a table row only; no rule states its fields. | A specification check of `item-stack.md` first (step 5). The armour place gate waits for it. |
| 9 | The commands `clear`, `give` and `item` have no specification (INV section 9). | A specification before the commands. `clear_items` exists first, because the decision about the cursor is fixed. |
| 10 | How a game rule outside the `minecraft` namespace is registered. `mcrs_minecraft_game_rule` holds generated keys only. | Step 5 finds the path. If none exists, the rule is a field of `VanillaInventoryPlugin` until it does. |
| 11 | `mcrs:spectators_open_menus` belongs to the specification of the block menus (INV section 9). | `vanilla_inventory` reads it where it calls `open_menu`. The rule has no effect until #63. |
| 12 | U1 of `inventory.md`: the items that the off-hand slot accepts. | **Decided by the owner: each item.** The off-hand slot has no filter (INV-SLT-005), and no game rule or tag changes it. |
| 13 | U2 of `inventory.md`: the place of the comparison in the tick. | Experiment E7 and E8. Until then: after `TickSet::Project`. |
| 14 | Q2 of `inventory-transactions.md`: is 128 the maximum of operations? It also limits a close (`close_container.rs:44`). | Keep 128. A close with more moves sends two `Close` requests. Change only when the craft specification needs more. |
| 15 | Q3: `docs/item-design.md` section 4.3 differs from the transaction specification. | The code follows the specification (`Op::Drop`, `Op::Set`, the reasons). Section 4.3 of that document changes. |
| 16 | Q5, Q6, U1, U3 of `inventory-transactions.md`. | Answers of the specification. Q5 needs E16; Q6 is a choice of the owner; U1 needs E4; U3 belongs to the craft. |
| 17 | `engine_item/src/spawn_stack.rs:16`: one archetype move for each component at a spawn. | Keep the ceiling. Lift it when a profile of `restore_items` or of a join shows it. |
| 18 | `EnderItems` and `WornEquipment` stay in `engine_item/src/persist.rs`. | Move their pure codec to `inventory_file` in step 2 if the move is mechanical. If not, a later step. |
| 19 | `MAX_PARTS` is 4, and a menu has at most 128 entries. | Keep the two. The largest vanilla menu has 90 entries. |

# 10. Steps

Each step is one pull request. Steps 0 to 6 run in this order. Section 10.1 runs in parallel. The
check of each step is `cargo nextest run -p <crate>` for each named crate and its direct
dependents.

**Step 0: the crate names.**

- Crates: the four crates that section 3 names, and each crate that depends on one of them.
- Files: the four crate directories, each `Cargo.toml` that names them, each `use` line, and the
  crate lists of `xtask`.
- Done: no file names an old crate name. No type and no behaviour changes. No other pull request
  that touches these crates is open before this one merges.

**Step 1: the change source.**

- Crates: `mcrs_minecraft_engine_item`, `mcrs_minecraft_vanilla_inventory`.
- Files: `engine_item/src/in_slot.rs`, `commit.rs`, `consume.rs`, `give.rs`, `load_inventory.rs`,
  `store.rs`, `stack_commands_ext.rs`, `holder_commands_ext.rs`, `item_plugin.rs`, a new
  `slot_changed.rs`;
  `vanilla_inventory/src/sync_menus.rs`, `menu_queue.rs`, `vanilla_inventory_plugin.rs`.
- Done: a test app calls `put_at` and `consume` on a slot of a player. The sync of the same tick
  writes the slot packet. The comparison builds no value without a mismatch, and a test with the
  counting allocator shows it. The ceilings of `menu_queue.rs` and `sync_menus.rs` are gone.

**Step 2: the split and the file codec.**

- Crates: new `mcrs_minecraft_inventory_file`, new `mcrs_minecraft_inventory_server`;
  `mcrs_minecraft_vanilla_inventory`, `mcrs_minecraft_engine_item`, `mcrs_minecraft_server`.
- Files to the file crate: only the pure parts of `player_file.rs`, `player_file_codec.rs`,
  `saved_stack.rs` and `player_stacks.rs`. These parts stay in `vanilla_inventory`: the `World`
  and `Query` glue (`capture_player_file`, `apply_player_file`, `PlayerStacks::from_holders`), the
  item table lookup in the world, the map of errors to the error type of the engine, and the
  carry of a cell crossing (`read_carried`, `write_carried`, the carried keys).
  `kept_equipment.rs` stays in `vanilla_inventory`.
- Files to the edge crate: `decode_click.rs`, `apply_click.rs`, `apply_inventory_input.rs`,
  `click_slots.rs`, `click_menu.rs`, `click_drag.rs`, `claimed_slot.rs`, `holder_slots.rs`,
  `inventory_input.rs`, `inventory_menu_sync.rs`, `menu_content.rs`, `menu_queue.rs`,
  `full_sends.rs`, `send_full_state.rs`, `sent_stack.rs`, `sync_menus.rs`,
  `forget_rejected_swap.rs`, and the packet observers of `hand_action.rs`, `close_container.rs`,
  `set_carried_item.rs` and `set_creative_mode_slot.rs`. Only edge files use `holder_slots.rs`.
- New in `vanilla_inventory`: `PlayerInventoryCommandsExt`, `PlayerInventories`.
- Done: `vanilla_inventory` has no dependency on a packet crate, a network crate,
  `inventory_click` or `inventory_client_view`, and the dependency check of `xtask` states it. A test app
  with `ItemPlugin` and `VanillaInventoryPlugin` only runs a hand swap and a close. The example
  `inventory_save_viewer` reads the inventory of a player file with no Bevy dependency. The
  example `inventory_minigame` gets its kit plugin with snapshot and restore. The server behaves
  as before.

**Step 3: the slot policy and the cancel point.**

- Before: the answer to open point 3.
- Crates: `mcrs_minecraft_engine_item`, `mcrs_minecraft_inventory_player`,
  `mcrs_minecraft_vanilla_inventory`, `mcrs_minecraft_inventory_server`.
- Files: `engine_item/src/commit.rs`, `request.rs`, `request_result.rs`, `item_app_ext.rs`, new
  `slot_policy.rs`, `gate.rs`, `request_check.rs`; `inventory_player/src/slot_filter.rs`;
  `inventory_server/src/click_slots.rs`; the holder spawn of `vanilla_inventory`.
- Done: a request to a locked slot gives `Reason::TakeDenied` or `Reason::PlaceDenied`. A hand
  swap with a place filter of a plugin on the off-hand slot has a rejection. An
  observer denies a request, and no slot changes. The click path and the commit read the same
  policy; `ClickSlots` holds no filter of its own. The example `inventory_minigame` gets a plugin
  with a locked slot and a veto.

**Step 4: the menu as data.**

- Crates: `mcrs_minecraft_inventory_menu`, new `mcrs_minecraft_engine_inventory_menu`, `mcrs_minecraft_inventory_server`,
  `mcrs_minecraft_vanilla_inventory`.
- Files: `inventory_menu/src/inventory_menu.rs` and `menu_slot.rs` become `menu_layout.rs`, `menu_entry.rs`
  and `route.rs`; the click and sync files of the edge; `close_container.rs` and
  `close_inventory_menu.rs` of the gameplay.
- Done: the inventory menu is the value `MenuLayout::INVENTORY`, and the tests of the click kernel
  pass over it without a change of their tables. A test opens a second menu over a holder with 27
  slots, clicks a button entry and reads `EntryPressed`. A close gives the client views back to
  the inventory menu (INV-SYN-007). `InventoryMenu` and `MenuSlot` are deleted. The example
  `inventory_minigame` gets a shop menu plugin with button entries.

**Step 5: `equippable` and the game rules.**

- Before: a specification check of `docs/spec/item-stack.md` for `equippable` and for the
  enchantment effect that locks an armour slot. A gap is a question to its author.
- Crates: `mcrs_minecraft_item_stack`, `mcrs_minecraft_vanilla_inventory`,
  `mcrs_minecraft_inventory_server`, `mcrs_minecraft_game_rule` or the engine crate that holds the
  game rules.
- Files: the component type in `item_stack`; the two armour gates of `vanilla_inventory`; the
  removal of the off-hand rule and its gate (INV-SLT-005); the quick
  move call in the edge that passes `None` for the equip slot today (`click_menu.rs:26`); the
  sent selected slot in the sync.
- Done: a helmet enters the head slot by a click and by a quick move (INV-SLT-003, INV-MNU-004).
  The game rule `mcrs:spectators_open_menus` exists with the default value true. The off-hand
  slot accepts each item, and no switch for it exists. A system that writes `Selected` causes one
  `set_held_slot` packet. The ceiling of `click_slots.rs` is gone.

**Step 6: the request ids.**

- Crates: `mcrs_minecraft_engine_item`, `mcrs_minecraft_vanilla_inventory`,
  `mcrs_minecraft_inventory_server`.
- Files: `engine_item/src/request_id.rs`, `item_app_ext.rs`; `vanilla_inventory/src/request_ids.rs`;
  each caller of `Request::new`.
- Done: no crate makes a `RequestSource` without `allocate_request_source`. Two plugins in one
  test app get two sources. `request_ids.rs` holds serial numbers only. The example
  `inventory_minigame` gets a plugin with a request source of its own that reads `RequestResult`.

**Step 7: the experiments.**

- Tool: `tools/mcrs_minecraft_probe` sends the inventory packets and records the menu packets.
- Run E1 to E11 of `inventory.md` and E1 to E15 of `inventory-transactions.md`. E16 needs a
  Bedrock server and waits for the Bedrock edge.
- Done: each experiment has a recorded result as a fixture. Each rule that a result contradicts
  is a question to the specification author. U2 of `inventory.md` and U1 of
  `inventory-transactions.md` have an answer.

**Step 8: the clicks of the client (#129).**

- Crates: new `mcrs_minecraft_inventory_predict`; the client crates.
- Before: groups 4 and 8 of `item-design.md` section 10.3.5 (gate W2).
- Done: a headless client predicts a click with the kernel that the server uses, and sends the
  `container_click` packet with hashed stacks. A test runs the client and the server in one
  process: after a click, the server sends no slot packet. The example `inventory_bot_click`
  predicts a click and encodes the packet with no render crate and no server crate.

## 10.1 The migration from the old stack crate, in parallel

This is steps 11 and 12 of `docs/item-design.md` section 10. Section 10.3 of that document gives
the groups, their order, the new home of each type and the gates. This section states only the
effect of each group on the inventory. At most three migrations are open at one time, this one
included.

The protocol crate cannot go first. It gets old types from `particle` and `recipe`, and the bare
wire form of a stack needs the codec of each component type before a client can read it.

| Group of `item-design.md` 10.3.5 | Effect on the inventory |
|-|-|
| 4, stacks of serverbound packets | The byte round trips of `decode_click.rs:64` and `set_creative_mode_slot.rs:57` go away. The edge reads the hashed stack and the creative stack from the packet. This group can come first. |
| 5, item links | none |
| 6, drops and block entities | A container of a block entity holds `SlotStack`. |
| 8, client state and the stacks and templates of clientbound packets | The client holds its stacks in the entity form. The byte round trips of `sent_stack.rs:23` and `vanilla_item_entity/src/wire_stack.rs:25` go away. The edge passes `ItemStack` to the packet. The proxy use case of section 5 is complete. Necessary before step 8. |
| 1, 2, 3, 7, 9 | none |
| 10, the end | The pull request that moves the last user deletes `mcrs_minecraft_item`. |

Step 8 needs groups 4 and 8. Group 8 waits for the wire codecs of all 122 component types, so
step 8 also waits for them.

# 11. Done conditions of the whole work

1. A `put_at` or a `consume` of a plugin reaches the client in the sync of the same tick.
2. `mcrs_minecraft_vanilla_inventory` builds and runs in a test app that has no Java packet crate.
3. An example outside the workspace crates reads the inventory of a player file with no Bevy
   dependency. The example `inventory_save_viewer` shows it.
4. A plugin locks a slot, denies a request and opens a menu with button entries, with no change
   to a crate of this document. The example `inventory_minigame` shows it.
