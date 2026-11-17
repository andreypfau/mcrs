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
- `MenuClick { viewer, click }` is the one way to click. A bot and a GUI write the same message.
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
| `Selected` on the client | authoritative truth | the client edge |
| `Click` | a value; one gesture on one menu entry | the reader that makes it |
| `MenuClick` | temporal fact: one click of a viewer | a bot, a GUI or a screen |
| predicted slot content | the content of the holder after the commit; no second copy | the client edge |
| state id of a remote menu | authoritative truth of the client edge | the client edge only |
| hovered slot, tooltip | derived state; computed in each frame | the screens crate |

The client stores no predicted click. A prediction changes the holders directly. A correction
replaces the content of one slot. Thus no list of pending clicks exists, and no state can resync.

# 3. Crate map

The layer numbers are those of `CLAUDE.md`. Each name has the prefix `mcrs_minecraft_`.

| Crate | Layer | Owns |
|-|-|-|
| `container_click` | 3, data, `no_std` | the click value; its codec behind the feature `wire` |
| `inventory_menu` | 3, data | the layout, the menu entries |
| `inventory_click` | 3, rules, `no_std` | a click to operations; `Click::from_input` (a gesture to a click) |
| `inventory_transaction` | 3, rules | the planner of a request |
| `inventory_client_view` | 3 | `StateId`, `ClientView` |
| `inventory_file` | 3, data | the inventory of a player file |
| `engine_item` | 4 | stacks as entities, slots, the commit of a request |
| `engine_inventory_menu` | 4 | the menu as an entity |
| `engine_inventory_click` | 4 | the driver of the click kernel: `MenuClicks`, `MenuClicker`, `MenuDrag`, `MenuClickPlugin` |
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
- `inventory_client` depends on no rendering crate. A bot is the client with rendering off.
- No crate below the edge knows the edition.

# 4. Public names

Each item below is `pub`. All other items are `pub(crate)`. Each subsection gives the crate, the
layer and one contract line for each name.

## 4.1 `mcrs_minecraft_engine_item` (layer 4), changes

The player item file leaves this crate. `ItemPersistencePlugin` stays. It initializes the item
resources of a world that loads stacks without `ItemPlugin`. It registers no codec.

## 4.2 `mcrs_minecraft_player_holders` (layer 5), new

```rust
pub struct PlayerHoldersPlugin;
#[derive(Component)] pub struct PlayerHolders { pub cursor: Entity, pub craft_grid: Entity, pub craft_result: Entity }
#[derive(EntityEvent)] pub struct PlayerItemsRestored { pub entity: Entity }
pub fn restore_holders(In<(Entity, PlayerItems)>, Commands, Query<Option<&PlayerHolders>>);
pub fn restore_carried(In<(Entity, PlayerItems)>, Commands, Query<Option<&PlayerHolders>>);
```

| Name | Contract |
|-|-|
| `PlayerHoldersPlugin` | Gives each new player its holders and their `SlotPolicy`, and opens the inventory menu. It adds `MenuPlugin` when the app has no `MenuPlugin`. |
| `PlayerHolders` | The three child holders of a player. The player entity is the fourth holder. |
| `PlayerItemsRestored` | Triggers after the holders of a player get the stacks of a restore. |
| `restore_holders` | Replaces the stacks of the four player holders and sets `Selected`. |
| `restore_carried` | Replaces the stacks of the cursor and the craft grid. |

The slot policy gives `Gate::Refuses` to the place filter of each armour slot and of the craft
result. The policy is the same on the server and on the client.

## 4.3 `mcrs_minecraft_vanilla_inventory` (layer 5), changes

```rust
pub struct PlayerItemFilePlugin;
#[derive(Component)] pub struct EnderItems(pub Vec<SlotStack>);
#[derive(Component)] pub struct WornEquipment { /* one field for each equipment slot */ }
```

| Name | Contract |
|-|-|
| `VanillaInventoryPlugin` | Adds `PlayerHoldersPlugin` when the app has no `PlayerHoldersPlugin`. The rest is unchanged. |
| `InventoryPersistencePlugin` | Also registers the codec of `EnderItems`. |
| `PlayerItemFilePlugin` | Registers the codecs of `EnderItems`, `SelectedItemSlot`, `Inventory` and `equipment` on the engine holders. An app adds this plugin or `InventoryPersistencePlugin`. |
| `EnderItems` | The ender chest stacks of a player file, as values. |
| `WornEquipment` | The worn stacks of a player file, as values. |

## 4.4 Names of the later pull requests **(proposal)**

| Name | Crate | Contract |
|-|-|-|
| `Click`, `ClickKind`, `Clicked` | `container_click` | One gesture on one menu entry, as data. |
| `Click::from_input` | `inventory_click` | Makes a click from a gesture of a pointer or a key. |
| `MenuClicks`, `MenuClicker`, `MenuDrag`, `MenuClickPlugin` | `engine_inventory_click` | Turns a click of a viewer into one request on the open menu. |
| `MenuClick { viewer, click }` | `inventory_client` | The one way to click on the client. |
| `RemoteMenu` | `inventory_client` | A menu that the server opened, with its state id. |
| `InventoryClientPlugin` | `inventory_client` | Applies the inbound packets to the holders and predicts each `MenuClick`. |

# 5. Usage

Each use case names the crates that a user takes. A use case passes when the user needs no crate
outside its row. Each crate name has the prefix `mcrs_minecraft_`.

| # | User | What it does | Crates it takes | Check |
|-|-|-|-|-|
| 1 | Proxy, packet logger | Reads the typed `Click` of a packet. Removes a shift click in a lobby. | `protocol_game` (with its `container_click`) | It builds with no rules crate and no Bevy. |
| 2 | Bot on Bevy | Writes `MenuClick`. Gets the prediction and the packet. | `inventory_client` | An example clicks with no GUI crate and no render crate in `cargo tree`. |
| 3 | Bot without Bevy (load test) | Keeps its own slot array. Runs the click kernel. Sends the packet. | `inventory_click`, `inventory_transaction`, `protocol_game` (with its `container_click`) | It builds with no Bevy in `cargo tree`. |
| 4 | Minigame server | Opens a virtual chest with button entries. Locks a slot. Denies a request. | `engine_item`, `engine_inventory_menu` | A plugin opens a 27-slot menu and handles a button with no change to an inventory crate. |
| 5 | Third-party GUI (egui, web) | Draws the slots from state. Writes `MenuClick` through `Click::from_input`. | `inventory_client` | It needs no screen crate of ours. |
| 6 | Anti-cheat, analytics | Reads the intents and the results. Writes nothing. | the messages of `engine_item` and of the edge | One system with message readers only. |
| 7 | Session record and replay | Stores `Click` values. Runs them through the kernel again. | `container_click`, `inventory_click` | The same clicks give the same slots. |
| 8 | Bedrock edge | Builds a `Request` directly. Does not use `Click`. | `engine_item`, `engine_inventory_menu` | No crate below the edge knows the edition. |
| 9 | Save viewer | Reads the inventory of a player file. | `inventory_file` | It builds with no Bevy. |

The final check of each row:

1. Run `cargo tree -p <crate>` for the crates of the row. No crate outside the row appears. The
   crates below the row in the layer order are an exception.
2. Rows 2, 3 and 4 have an example or a test that runs.
3. The names of the decisions in section 1 exist in the code with the same meaning.

**Client holders.** Crates: `engine_item`, `player_holders`. The client adds
`ItemPlugin` and `PlayerHoldersPlugin`. Each entity that gets the `Player` of `engine_world` gets
the four holders and the inventory menu. Today the client marks its local player only with the
`Player` of `client_world`. Step C also inserts the `Player` of `engine_world` on the local player
**(proposal)**.

```rust
app.add_plugins((ItemPlugin::new(table), PlayerHoldersPlugin));
```

**Bot (use case 2) (proposal).**

```rust
app.add_plugins((ClientWorldPlugins::headless(), InventoryClientPlugin));

fn act(player: Single<Entity, With<Player>>, mut clicks: MessageWriter<MenuClick>) {
    let viewer = *player;
    clicks.write(MenuClick { viewer, click: Click::from_input(Clicked::Entry(36), ClickInput::primary()) });
    clicks.write(MenuClick { viewer, click: Click::from_input(Clicked::Entry(9), ClickInput::shift_primary()) });
}
```

**GUI (use case 5) (proposal).**

```rust
let entry = layout_geometry.entry_at(pointer)?;
clicks.write(MenuClick { viewer, click: Click::from_input(entry, ClickInput::from_pointer(button, mods)) });
```

# 6. The tick of the client **(proposal)**

| Phase | What runs |
|-|-|
| ingest | The client edge applies each inbound inventory packet to the holders. |
| input | A bot, a GUI or a screen writes `MenuClick`. |
| predict | The client edge runs the planner and the commit of the server for each `MenuClick`. |
| output | The client edge sends one click packet for each `MenuClick`. |
| frame | The screens crate derives the hovered slot and the tooltip. |

A prediction uses the same planner and the same commit as the server. Thus a click gives the same
slots on the two sides when the two sides have the same slots. The client edge stores no predicted
click. When the server sends a slot content, the client edge replaces the content of that slot.
The server sends the content of each slot that differs (INV-SYN-002).

The drop and the hand swap follow the rule of `spec/inventory-transactions.md` for them.

# 7. Migration

Each step is one pull request.

| Step | Issue | Content |
|-|-|-|
| A | #982 | The holders and the slot policy are usable by a client. This document. |
| B | none | The click value (`Click`, `ClickKind`, `Clicked`) moves to the new crate `container_click`. The packet `ServerboundContainerClick` carries `Click`, and `ContainerInput` is deleted. The click adapter moves from `inventory_server` to the new crate `engine_inventory_click` as `MenuClicks`, `MenuClicker`, `MenuDrag` and `MenuClickPlugin`. |
| C | #129 | The new crate `inventory_client`: the inbound packets on `engine_item` holders, `RemoteMenu`, `MenuClick`, and the prediction with the planner and the commit of the server. It stores no predicted click. Also the pointer and the keys, the carried stack, and the drop and the hand swap by `spec/inventory-transactions.md`. Specification: `spec/client-inventory.md`. |
| D | #130 | The open and the close of the menus of the server, generic layouts, container screens, drag paint, double click, item tooltips. Specification: `spec/client-container-screen.md`. |
| E | #131 | The creative screen. Specification: `spec/client-creative-inventory.md`. |
