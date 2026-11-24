# Inventory Client Design

This document fixes the crate map and the public API of the inventory on the client. It contains no
implementation. `docs/inventory-design.md` fixes the inventory of the server. This document builds
on it and uses its terms. The behaviour is in `spec/inventory.md` (`INV-*`) and
`spec/inventory-transactions.md` (`ITX-*`). The behaviour of the client is in
`spec/client-inventory.md`, `spec/client-container-screen.md` and
`spec/client-creative-inventory.md`.

A line that has the mark **(proposal)** is a choice of this document. A line that has the mark
**(decided)** is a decision of the owner and is fixed.

# 1. Purpose and scope

The client, a bot and a third-party GUI must use the inventory crates of the server. Today the
holders and the slot policy of a player need the server engine. This document makes them usable
alone. It also gives the plan of the client work. The users are:

| User | Needs |
|-|-|
| proxy, packet logger | the typed click of a packet; no rules crate and no Bevy |
| bot on Bevy | a click as a message, the prediction and the packet |
| bot without Bevy | its own slot array, the click kernel and the packet |
| minigame server | a virtual chest menu with button entries, locked slots, denied requests |
| third-party GUI | the slots as state; a click from a pointer gesture |
| anti-cheat, analytics | the intents and the results, read only |
| session record and replay | stored clicks that give the same slots again |
| Bedrock edge | requests that it builds directly, with no click |
| save viewer | the inventory of a player file; no Bevy |

Terms. Each term has the meaning of `docs/inventory-design.md` section 1. This document adds these
terms:

- **Click**: the value `Click` of `container_click`. It is one gesture on one menu entry.
- **Prediction**: the result of a click on the client before the server answers.
- **Correction**: a slot content from the server that replaces the content of the client.
- **Remote menu**: a menu on the client that the server opened.

Decisions of the owner **(decided)**:

- The click value (`Click`, `ClickKind`, `Clicked`) is in its own crate `container_click`. The
  crate is layer 3 data, `no_std` and has no Bevy. It is the content of the `container_click`
  packet, and its codec is behind the feature `wire`.
- The packet `ServerboundContainerClick` carries `Click`. `ContainerInput` does not exist.
- No crate maps a click to a packet. No crate name has `java`, `packet` or `wire` for this purpose.
- The client keeps its stacks in `engine_item` holders, as the server does.
- The client stores no predicted click. The server corrects each slot that differs.
- `MenuInput { viewer, action }` is the one way to click in the open menu and to close it. Its
  `MenuAction` is `Click(Click)` or `Close`. A bot and a GUI write the same message. The client
  applies and sends the inputs in the order of the writes (CINV-PKT-007).
- `HandInput { viewer, action }` is the drop key and the swap key outside a screen. Its
  `HandAction` is the one that the server uses.
- The driver of the click kernel is its own crate `engine_inventory_click`, not part of
  `engine_inventory_menu`. Thus a minigame and a Bedrock edge do not link the Java click kernel.
- `Screen::Menu` is one variant. The open menu gives its content.
- The client derives the hovered slot and the tooltip in each frame.

In scope: the holders and the slot policy without the server engine, the click value, the client
edge, the screens. Out of scope: the craft, the menu of each block. Each has a later
specification.

# 2. State classification

| Stored thing | Class | Owner or writer |
|-|-|-|
| `Slots`, `InSlot`, `SlotPolicy` on the client | as `docs/inventory-design.md` section 2 | the client edge; `PlayerHoldersPlugin` for the policy |
| `PlayerHolders` on the client | authoritative truth: the three child holders of the local player | `PlayerHoldersPlugin` |
| `Selected` on the client | materialized derived state: a projection of `SelectedHotbarSlot`. The client derives it from the hotbar slot until the GUI half removes the old value. | one projection system of the client world |
| `PlayerGameMode` on the client | materialized derived state: a projection of `LocalGameMode`. The client derives it from the local game mode until the GUI half reads one value. The take filter (INV-SLT-004) and the spectator check read it. | one projection system of the client world, in the fixed tick and in each frame |
| `Abilities` on the client | authoritative truth: the abilities of the last `player_abilities` packet or game mode change. Its `instant_build` is the creative flag, which gives infinite materials to a predicted click (CINV-PRD-001) and the creative flag of a gesture. | the observers of `GameModePlugin` in the client world |
| `Click` | a value; one gesture on one menu entry | the reader that makes it |
| `MenuInput` | temporal fact: one click, one creative click or one close of a viewer, in the order of the inputs | a bot, a GUI or a screen |
| `HandInput` | temporal fact: one press of the drop key or the swap key outside a screen | a bot or the key input of the client |
| `MenuDrag` | authoritative truth: the drag in progress on one menu | `MenuClicks`; `MenuClickPlugin` inserts it when the menu opens and resets it when a menu of the viewer closes |
| predicted slot content | the content of the holder after the commit; no second copy | the client edge |
| state id of a remote menu | authoritative truth of the client edge | the client edge only |
| hovered slot, tooltip | derived state; computed in each frame | the screens crate |
| `CreativeContent` | materialized derived state: the tab lists and the search list of the creative screen, built from the creative report, the registries, the tags, the enabled features and the operator condition. A build decodes about 4000 stacks. It runs again only when one of these inputs changes (CCI-CNT-005), never in a tick. | the creative screen; one system builds it again |
| content of the selected creative tab | derived state: an items tab shows a slice of `CreativeContent`. The search tab shows the stacks of the search list that `SearchQuery::matches`, computed again when the search text or `CreativeContent` changes: one pass over about 2300 stacks. | the creative screen |
| creative drop counter | authoritative truth of the client edge (CCI-SYN-003) | a creative action adds 20; one `FixedUpdate` system subtracts 1 while the counter is above 0 |

The client stores no predicted click. A prediction changes the holders directly. A correction
replaces the content of one slot. Thus no list of pending clicks exists, and no state can resync.

# 3. Crate map

The layer numbers are those of `CLAUDE.md`. Each name has the prefix `mcrs_minecraft_`.

| Crate | Layer | Owns |
|-|-|-|
| `container_click` | 3, data, `no_std` | the click value; `Click::from_input` (a gesture to a click); its codec behind the feature `wire` |
| `inventory_menu` | 3, data | the layout, the menu entries |
| `inventory_click` | 3, rules, `no_std` | a click to operations |
| `creative_content` | 3, rules | the tab lists and the search list of the creative screen, and the match of the search text |
| `inventory_transaction` | 3, rules | the planner of a request |
| `inventory_client_view` | 3 | `StateId`, `ClientView` |
| `inventory_file` | 3, data | the inventory of a player file |
| `engine_item` | 4 | stacks as entities, slots, the commit of a request |
| `engine_inventory_menu` | 4 | the menu as an entity |
| `engine_inventory_click` | 4 | the driver of the click kernel: `MenuClicks`, `MenuClicker`, `MenuClickError`, `MenuDrag`, `MenuClickPlugin` |
| `player_holders` | 5 | `PlayerHolders`, the vanilla slot policy, `PlayerHoldersPlugin` |
| `vanilla_inventory` | 5 | the gameplay and the persistence of the server |
| `inventory_server` | 5, edge | the server side of the Java exchange |
| `inventory_client` | 5, edge | the client side of the Java exchange; no rendering |
| screens crate | 6 or 7 | the inventory, container and creative screens; a plugin on `inventory_client` |

Dependency rules:

- `engine_item` and `player_holders` link no server-only crate: `engine`,
  `engine_server`, `save` and `server`. The check `layers` of `xtask` fails when one of them does.
- `vanilla_inventory` depends on `engine` and `vanilla_item_entity`. A client does not take it.
- `engine_inventory_menu` does not depend on `inventory_click`. Only `engine_inventory_click` links
  the click kernel.
- `MenuClicks` reads the whole world, because a slot gate reads the world. A system that takes
  `MenuClicks` takes no other parameter that writes, except `Commands`.
- `inventory_client` depends on no rendering crate. A bot is the client with rendering off.
- No crate below the edge knows the edition.

# 4. Public names

Each item below is `pub`. All other items are `pub(crate)`. Each subsection gives the crate, the
layer and one contract line for each name.

## 4.1 `mcrs_minecraft_engine_item` (layer 4), changes

The player item file leaves this crate. `ItemPersistencePlugin` stays. It initializes the item
resources of a world that loads stacks without `ItemPlugin`. It registers no codec.

`ItemPlugin` takes no item table **(decided)**. `ItemPlugin::default()` registers the messages, the
sets and the observers. `ItemTable` is a resource. It is present exactly while the app can create
stacks. The server inserts it where it builds the table. The client bakes it from the registries of
each session and removes it when the session ends. The client uses the previous table again when
the inputs of the bake are equal, because each bake leaks the table data.
A tag update in a session that changes the bake inputs bakes a new table. Live stacks keep their
values, and the server sends the slots again.

`mcrs_minecraft_engine_registries` gets `read_packet_stack`. It reads the stack of a packet into an
`ItemStack` through `RegistryNames`. The client edge uses it for each inbound slot. It also gets
`ItemTableInputs`, the inputs of the bake. It reads the same registry keys as `RegistryNames`, so
a new name table is a new input of the bake.

## 4.2 `mcrs_minecraft_player_holders` (layer 5), new

```rust
pub struct PlayerHoldersPlugin;
#[derive(Component)] pub struct PlayerHolders { pub cursor: Entity, pub craft_grid: Entity, pub craft_result: Entity }
pub fn restore_holders(In<(Entity, PlayerItems)>, /* system parameters */);
pub fn restore_carried(In<(Entity, PlayerItems)>, /* system parameters */);
```

| Name | Contract |
|-|-|
| `PlayerHoldersPlugin` | Gives each new player its holders and their `SlotPolicy`, and opens the inventory menu. It adds `MenuPlugin` when the app has no `MenuPlugin`. |
| `PlayerHolders` | The three child holders of a player. The player entity is the fourth holder. |
| `restore_holders` | Replaces the stacks of the four player holders and sets `Selected`. |
| `restore_carried` | Replaces the stacks of the cursor and the craft grid. |

The slot policy gives `Gate::Refuses` to the place filter of the craft result. `PlayerHoldersPlugin`
registers the armour place gate (INV-SLT-003), and the policy gives that gate to the place filter
of each armour slot. The gate reads the `minecraft:equippable` component of the stack. It reads the
entity type id of the player and the entity type tags that contain it from one resource. An
observer writes this resource each time the app inserts or removes `CurrentRegistries`. When the
gate does not register, the place filter of each armour slot is `Gate::Refuses`. The armour take
gate (INV-SLT-004) does not exist yet, because it needs the game mode of the player. Until then,
the take filter of each armour slot is `Gate::Permits`. The policy is the same on the server and on
the client.

## 4.3 `mcrs_minecraft_vanilla_inventory` (layer 5), changes

```rust
#[derive(Component)] pub struct EnderItems(pub Vec<SlotStack>);
```

| Name | Contract |
|-|-|
| `VanillaInventoryPlugin` | Adds `PlayerHoldersPlugin` when the app has no `PlayerHoldersPlugin`. The rest is unchanged. |
| `InventoryPersistencePlugin` | Also registers the codec of `EnderItems`. |
| `EnderItems` | The ender chest stacks of a player file, as values. |

## 4.4 Names of the later pull requests **(proposal)**

| Name | Crate | Contract |
|-|-|-|
| `Click`, `ClickKind`, `Clicked` | `container_click` | One gesture on one menu entry, as data. |
| `Click::from_input` | `container_click` | Makes a click from a gesture of a pointer or a key. |
| `MenuClicks`, `MenuClicker`, `MenuClickError`, `MenuDrag`, `MenuClickPlugin` | `engine_inventory_click` | Turns a click of a viewer into one request on the open menu. |
| `MenuInput { viewer, action }`, `MenuAction::{Click, Close}` | `inventory_client` | The one way to click in and to close the open menu on the client. |
| `MenuAction::Creative(CreativeClick)`, `CreativeClick { area, click }`, `CreativeArea::{Grid, Hotbar, Inventory, Destroy}` | `inventory_client` | One click in the creative screen: the area of the screen and the click of the gesture. The client edge applies it to the holders and sends a `set_creative_mode_slot` packet for each changed entry. |
| `CreativeContent`, `SessionInputs`, `OperatorCondition`, `SearchQuery` | `creative_content` | The tab lists and the search list from the creative report and the session, and the search match, with no Bevy. |
| `HandInput { viewer, action }` | `inventory_client` | The drop key and the swap key outside a screen. |
| `InventoryClientSet::{Predict, Send}` | `inventory_client` | The sets of the prediction and of the send, in `Update` for `MenuInput` and in `FixedUpdate` for `HandInput`. A writer of an input runs before `Predict`. |
| `RemoteMenu` | `inventory_client` | A menu that the server opened, with its container id and the last state id of the server, as raw packet values. |
| `StackView<'a> { stack: &'a ItemStack, table: &'a ItemTable }` | `model` | The input of the item model evaluator: a layer 3 value, not an entity. |
| `InventoryClientPlugin` | `inventory_client` | Applies the inbound packets to the holders, and predicts and sends each `MenuInput` and `HandInput`. |

# 5. Usage

Each use case names the crates that a user takes. A use case passes when the user needs no crate
outside its row. Each crate name has the prefix `mcrs_minecraft_`.

| # | User | What it does | Crates it takes | Check |
|-|-|-|-|-|
| 1 | Proxy, packet logger | Reads the typed `Click` of a packet. Removes a shift click in a lobby. | `protocol_game` (with its `container_click`) | It builds with no rules crate and no Bevy. |
| 2 | Bot on Bevy | Writes `MenuInput`. Gets the prediction and the packet. | `inventory_client` | An example clicks with no GUI crate and no render crate in `cargo tree`. |
| 3 | Bot without Bevy (load test) | Keeps its own slot array. Runs the click kernel. Sends the packet. | `inventory_click`, `inventory_transaction`, `protocol_game` (with its `container_click`) | It builds with no Bevy in `cargo tree`. |
| 4 | Minigame server | Opens a virtual chest with button entries. Locks a slot. Denies a request. | `engine_item`, `engine_inventory_menu` | A plugin opens a 27-slot menu and handles a button with no change to an inventory crate. |
| 5 | Third-party GUI (egui, web) | Draws the slots from state. Writes `MenuInput` through `Click::from_input`. | `inventory_client` | It needs no screen crate of ours. |
| 6 | Anti-cheat, analytics | Reads the intents and the results. Writes nothing. | the messages of `engine_item` and of the edge | One system with message readers only. |
| 7 | Session record and replay | Stores `Click` values. Runs them through the kernel again. | `container_click`, `inventory_click` | The same clicks give the same slots. |
| 8 | Bedrock edge | Builds a `Request` directly. Does not use `Click`. | `engine_item`, `engine_inventory_menu` | No crate below the edge knows the edition. |
| 9 | Save viewer | Reads the inventory of a player file. | `inventory_file` | It builds with no Bevy. |

The final check of each row:

1. Run `cargo tree -p <crate>` for the crates of the row. No crate outside the row appears. The
   crates below the row in the layer order are an exception.
2. Rows 2, 3 and 4 have an example or a test that runs.
3. The names of the decisions in section 1 exist in the code with the same meaning.

**Client holders (decided).** Crates: `engine_item`, `player_holders`, `inventory_client`.
`ClientRegistriesPlugin` of `client_world` adds the observer that spawns the local player. It also
adds `ItemPlugin`, `PlayerHoldersPlugin` and `InventoryClientPlugin` when the app does not have
them. An app that configures one of these plugins adds it before `ClientRegistriesPlugin`. Thus
the holders always come with their slot policy and with the receive observers, and a headless bot
needs no GUI crate. The local player does not get the `Player` of `engine_world`. The
code that spawns the local player calls `restore_holders` with empty items. Thus the player gets
the four holders and the open inventory menu, as a new player on the server. `client_world`
depends on `inventory_client`. The reverse dependency does not exist. The client edge finds the
local player as the one entity with `PlayerHolders` and `Menus`. A `respawn` packet removes the
`RemoteMenu` of the inventory menu. A menu without `RemoteMenu` has the state id 0 (CINV-PKT-003).
An inbound slot that changes only the count of its stack writes `Count` in place and keeps the
stack entity.

```rust
app.add_plugins(ClientRegistriesPlugin::default());
```

**Item icons (decided).** The item model evaluator reads a `StackView`. It is a pure function of the
value, so a model viewer gives it a value and needs no holder. One client system makes the icon
layers from the `engine_item` stacks. It makes them again only for the slots of `SlotChanged`, and
for all stacks when the item models change.

**Bot (use case 2) (proposal).**

```rust
app.add_plugins((ClientNetworkPlugin { holds_vanilla_pack: true, play: Some(GAME_CLIENTBOUND) }, ClientRegistriesPlugin::default()));

fn act(player: Single<Entity, With<Player>>, mut inputs: MessageWriter<MenuInput>) {
    let viewer = *player;
    for click in [Click::from_input(Clicked::Entry(36), input), Click::from_input(Clicked::Entry(9), shift)].into_iter().flatten() {
        inputs.write(MenuInput { viewer, action: MenuAction::Click(click) });
    }
    inputs.write(MenuInput { viewer, action: MenuAction::Close });
}
```

**GUI (use case 5) (proposal).**

```rust
let entry = layout_geometry.entry_at(pointer)?;
if let Some(click) = Click::from_input(entry, input) {
    inputs.write(MenuInput { viewer, action: MenuAction::Click(click) });
}
```

# 6. The frame and the tick of the client

The client handles the input of a screen in the frame of the input, and the keys outside a screen
once a tick.

| Input | Schedule | Why |
|-|-|-|
| `MenuInput` (click, close) | `Update`, after `ClientNetworkSystems::Receive`, before `ClientNetworkSystems::Flush` | The client applies a click in the frame of its event (CINV-PKT-007, CINV-PRD-001). The packet leaves in the same frame. |
| `HandInput` (drop key, swap key) | `FixedUpdate` | The client handles these keys once a tick (CINV-HND-001, CINV-HND-002). The packet leaves at the flush of the next `Update`, after `set_carried_item`. |

The data flow of one click in one frame:

| Step | System | What it does |
|-|-|-|
| ingest | `ClientNetworkSystems::Receive` | The client edge applies each inbound inventory packet to the holders. |
| input | a bot, a GUI or a screen, before `InventoryClientSet::Predict` | Writes `MenuInput`. |
| predict | `InventoryClientSet::Predict` | Queues one command for each `MenuInput`, in the order of the writes. A click command runs the planner (`MenuClicks::plan`) and the commit of `engine_item`, then puts one `container_click` with the claims into the outbox. A close command puts one `container_close` into the outbox and empties the craft result. |
| output | `InventoryClientSet::Send` | Writes the outbox to the connection in order. |
| flush | `ClientNetworkSystems::Flush` | Sends the bytes. |
| frame | the screens crate | Derives the hovered slot and the tooltip from the predicted holders. |

A headless client runs the same `Update` schedule with no window, so a bot gets the same order.
The projections of `PlayerGameMode` and `Selected` run before `InventoryClientSet::Predict` in the
two schedules.

A prediction uses the same planner and the same commit as the server. Thus a click gives the same
slots on the two sides when the two sides have the same slots. The client edge stores no predicted
click. When the server sends a slot content, the client edge replaces the content of that slot.
The server sends the content of each slot that differs (INV-SYN-002).

The drop and the hand swap follow the rule of `spec/inventory-transactions.md` for them. The swing
of the main hand after a drop (CINV-HND-001, step 5) sends no packet, and the client has no swing
state yet, so the client edge does nothing for it.

# 7. Migration

Each step is one pull request.

| Step | Issue | Content |
|-|-|-|
| A | #982 | The holders and the slot policy are usable by a client. This document. |
| B | #984 | The click value (`Click`, `ClickKind`, `Clicked`) moves to the new crate `container_click`. The packet `ServerboundContainerClick` carries `Click`, and `ContainerInput` is deleted. The click adapter moves from `inventory_server` to the new crate `engine_inventory_click` as `MenuClicks`, `MenuClicker`, `MenuClickError`, `MenuDrag` and `MenuClickPlugin`. |
| C1 | #129 (refs) | `ItemPlugin` takes no table; `ItemTable` is a session resource on the client. The new crate `inventory_client` applies the inbound packets to the `engine_item` holders of the local player, and `RemoteMenu` keeps the state id. The old slot table of the client still feeds the GUI. |
| C1b | #129 (refs) | The evaluator reads `StackView`. The icons, the hotbar and the inventory screen read the holders. The entity stack model of `mcrs_minecraft_item` and the old slot table of the client are deleted. |
| C2 | #129 | `MenuInput` and `HandInput`, and the prediction with the planner and the commit of the server. It stores no predicted click. Also the pointer and the keys, the carried stack, and the drop and the hand swap by `spec/inventory-transactions.md`. Specification: `spec/client-inventory.md`. |
| D | #130 | The open and the close of the menus of the server, generic layouts, container screens, drag paint, double click, item tooltips. Specification: `spec/client-container-screen.md`. |
| E | #131 | The creative screen. Specification: `spec/client-creative-inventory.md`. |
