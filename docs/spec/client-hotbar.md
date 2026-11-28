# The hotbar in the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states how the player of the vanilla client changes the selected slot with the keys
1 to 9. It states when the client sends `set_carried_item`, and what the client does with a
clientbound `set_held_slot`. It also states which game modes draw the hotbar in the HUD. The
wheel is in `entity-physics.md`, PHY-INP-022. The selected slot and the two packets are in
`inventory.md`, section 3. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Selected slot**, **hotbar**, **off-hand slot**, **stack**, **empty stack**: as
  `inventory.md`, section 0.1, defines them. In this document, the selected slot is the value
  that the client holds for the local player.
- **Local player**, **frame**, **tick**: as `client-camera.md`, section 0.1, defines them. A tick
  of the client is a **client tick**.
- **Screen**: as `client-widget.md`, section 0.1, defines it.
- **In a world**: as `client-widget.md`, section 0.1, defines it.
- **Spectator menu**, **wheel event**, **notch count**: as `entity-physics.md`, section 0.1,
  defines them.
- **Overlay of a resource reload**: the overlay of `entity-physics.md`, PHY-INP-022, step 1.
- **Creative flag**: as `client-block-breaking.md`, section 0.1, defines it. It is the bit 0x08 of
  the flags of `player_abilities`.
- **HUD**: the GUI that the client draws over the world in each frame while it is in a world.
- **Hotbar keys**: the nine key bindings `key.hotbar.1` to `key.hotbar.9`. The key binding
  `key.hotbar.k` stands for the hotbar slot `k − 1`.
- **Press**: one transition of a key binding from up to down, while no screen is open. A repeat of
  the operating system for a key that stays down is also a press.
- **Toolbar activators**: the key bindings `key.saveToolbarActivator` and
  `key.loadToolbarActivator` of creative mode.
- **HUD game mode**: the game mode of the local player that the `login` packet, the `respawn`
  packet and the `game_event` that changes the game mode give (`client-block-breaking.md`,
  CBK-INP-001).
- **Spectator**: a local player whose own entry of `player_info_update` has the game mode
  spectator, as `entity-physics.md`, section 0.1, defines it.
- **Last sent value**: the slot that the client sent in its last `set_carried_item` (CHB-SYN-001).
- **Main-hand packet**: one of the serverbound packets `player_action` with each status,
  `use_item`, `use_item_on`, `attack` and `interact`. The result of most of them on the server
  depends on the main hand of the player.
- **Selected item name**: the name of the stack in the selected slot. The HUD shows it above the
  hotbar for a short time after the stack in the selected slot changes.
- **Hotbar attack indicator**: the attack indicator that the HUD draws beside the hotbar when the
  option "Attack Indicator" has the value "Hotbar".

### 0.2 Provenance, kind and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar.

The observer of each rule is a player in normal play who sees the HUD, unless the rule names
another observer. For the rules of section 3, the observer is the server that reads the
serverbound packets: a vanilla server (`inventory.md`, INV-SEL-002).

Each rule that comes from the reference keeps the label `source_suggested`.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CHB-MODE` | the game mode that the rules read, and a change of the game mode |
| `CHB-KEY` | the hotbar keys |
| `CHB-SYN` | `set_carried_item` and `set_held_slot` in the client |
| `CHB-HUD` | the hotbar in the HUD for each game mode |
| `CHB-DEV` | intentional deviations |

### 0.4 Scope

In scope:

- the hotbar keys outside a screen, and their result in each game mode;
- the moments when the client sends `set_carried_item`;
- the result of a clientbound `set_held_slot` in the client;
- which game mode draws the hotbar, and what the HUD draws in its place in spectator mode;
- the selected slot after a change of the game mode.

Out of scope:

- the wheel: `entity-physics.md`, PHY-INP-022 and PHY-INP-023;
- the content of the spectator menu, and its draw;
- the swap click with a hotbar key inside a menu screen. `inventory-transactions.md`,
  ITX-JAV-010, states it on the server. Issue 129 states it in the client;
- the save and the load of a hotbar in creative mode with the toolbar activators;
- the layout of the hotbar: the positions, the sizes and the sprites;
- the selected item name and the hotbar attack indicator, except the rule that hides them
  (CHB-HUD-002);
- the pick of a block or of an entity with the pick key;
- the debug keys that use the digit keys.

### 0.5 Constants

| Value | Data | Rule |
|-------|------|------|
| Hotbar keys: default keys | the keys 1 to 9 of the main row of the keyboard | CHB-KEY-001 |
| Toolbar activators: default keys | C (save), X (load) | CHB-KEY-001 |
| `key.spectatorHotbar`: default key | the middle mouse button | CHB-HUD-002 |
| Range of the selected slot | `0..8` | CHB-SYN-003 |
| Last sent value after a `login` packet | 0 | CHB-SYN-001 |

---

## 1. The game mode

### CHB-MODE-001: The source of the game mode

- **Inputs:** the HUD game mode, and the game mode of the own entry of `player_info_update`.
- **Behavior:**
  1. The HUD rules of section 4 read the HUD game mode.
  2. The input rules of section 2 and `entity-physics.md`, PHY-INP-022, step 9, read the
     spectator of section 0.1.
  3. An implementation can read one source for all these rules.
- **Postconditions:**
  - A vanilla server sends `player_abilities`, `player_info_update` and the `game_event` of a new
    game mode in the same tick. Thus the two sources agree for a player in normal play.
  - When the two sources disagree, the result of the rules of sections 2 and 4 is unspecified.

Provenance: `design`. Kind: unspecified. Confidence: high. Reason: the vanilla client reads two
sources, and a player in normal play never sees them disagree.

### CHB-MODE-002: A change of the game mode keeps the selected slot

- **Inputs:** a `game_event` that changes the game mode, or a `player_info_update` that changes
  the game mode of the local player.
- **Behavior:**
  1. The client does not change the selected slot.
  2. The client does not change the last sent value, and it sends no `set_carried_item`.
- **Postconditions:**
  - A player selects the slot 5 in survival mode and changes to spectator mode. The HUD draws no
    hotbar (CHB-HUD-002). The player changes back to survival mode. The selection frame is at the
    slot 5.
  - In spectator mode, the hotbar keys and the wheel do not change the selected slot (CHB-KEY-001,
    PHY-INP-022). A `set_held_slot` changes it in each game mode (CHB-SYN-003).
  - A vanilla server does not change its selected slot for a new game mode.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

---

## 2. The hotbar keys

### CHB-KEY-001: A hotbar key outside a screen

- **Inputs:** a press of the hotbar key for the slot `k`, while the client is in a world, no
  screen is open and the overlay of a resource reload does not show.
- **Behavior:** the client handles the press in the next client tick. The first true condition
  gives the result:
  1. The local player is a spectator: the press goes to the spectator menu. When the spectator
     menu does not show, the press opens it. When the spectator menu shows, the press selects its
     entry `k`. The selected slot does not change.
  2. The creative flag is on, and a toolbar activator is down: the press saves or loads a hotbar
     of creative mode. The selected slot does not change.
  3. Else: the selected slot becomes `k`.
- **Postconditions:**
  - The press of the key for the slot that is already selected changes nothing.
  - The HUD shows the new selection frame in the first frame after the client tick.
  - The client sends the new slot as CHB-SYN-001 states.
  - In survival, adventure and creative mode, the result is the same. In creative mode, the creative
    flag is on. Thus there the key selects a slot only when no toolbar activator is down.
  - A client without a spectator menu ignores a press of a spectator.
- **Edge cases:**
  - A screen is open, for example the chat screen or the death screen: the press goes to the
    screen. After the screen closes, the press has no result (CHB-KEY-002).
  - A player in a bed has the chat screen of the bed open. Thus the hotbar keys do not change the
    selected slot.

| Key binding | Slot `k` | Default key |
|-------------|----------|-------------|
| `key.hotbar.1` | 0 | 1 |
| `key.hotbar.2` | 1 | 2 |
| `key.hotbar.3` | 2 | 3 |
| `key.hotbar.4` | 3 | 4 |
| `key.hotbar.5` | 4 | 5 |
| `key.hotbar.6` | 5 | 6 |
| `key.hotbar.7` | 6 | 7 |
| `key.hotbar.8` | 7 | 8 |
| `key.hotbar.9` | 8 | 9 |

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### CHB-KEY-002: A hotbar key inside a screen

- **Inputs:** a down transition of a hotbar key while a screen is open.
- **Behavior:**
  1. The key goes to the screen. It is not a press of CHB-KEY-001.
  2. The selected slot does not change, in each screen.
  3. A menu screen, for example the inventory screen, can make a swap click from the key. The slot
     under the mouse pointer and the hotbar slot `k` are the two slots of the click.
     `inventory-transactions.md`, ITX-JAV-010, states the click on the server. Issue 129 states it
     in the client.
  4. A screen with a text box in edit mode ignores the key (`client-edit-box.md`, the rule of the
     key bindings).
- **Postconditions:** the client does not keep the key for the time after the screen closes.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### CHB-KEY-003: Two hotbar keys in one client tick

- **Inputs:** presses of two or more hotbar keys between two client ticks.
- **Behavior:**
  1. After the client handles the presses, the selected slot is the slot of one of the keys.
     Which key gives the result is unspecified.
  2. Each press has its result in the client tick after the press, or in a later client tick.
- **Postconditions:** the client sends at most one `set_carried_item` for the presses of one
  client tick (CHB-SYN-001).

Provenance: `source_suggested`. Kind: unspecified. Confidence: medium. Reason: a player does not
press two hotbar keys in 50 ms with an expectation of an order.

---

## 3. `set_carried_item` and `set_held_slot`

### CHB-SYN-001: The client sends a change of the selected slot

- **Observer:** the server that reads the serverbound packets.
- **Inputs:** the selected slot and the last sent value.
- **Behavior:**
  1. The client keeps the last sent value. A `login` packet sets it to 0. A `respawn` packet does
     not change it.
  2. When the selected slot is not the last sent value, the client sends `set_carried_item` with
     the selected slot. Then the last sent value becomes the selected slot.
  3. The client does step 2 one time in each client tick. It also does step 2 before each
     main-hand packet (CHB-SYN-002).
  4. The client never sends `set_carried_item` with the last sent value.
- **Postconditions:**
  - The client sends `set_carried_item` only for a change. In one client tick, it sends at most
    one for step 3 and one before each main-hand packet.
  - A wheel event or a press changes the selected slot. The client sends it no later than the
    end of the next client tick.
  - Three notches of the wheel between two client ticks change the selected slot from 0 to 3.
    The client sends one `set_carried_item` with 3.
  - One notch away from the player and one notch toward the player between two client ticks give
    the slot of the start. The client sends nothing.
  - A screen does not stop the sends. A `set_held_slot` that arrives while a screen is open gives
    its echo of CHB-SYN-003.
- **Edge cases:** the order of `set_carried_item` and the movement packets of one client tick is
  unspecified. The bound of the postconditions applies.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### CHB-SYN-002: `set_carried_item` before a main-hand packet

- **Observer:** the server that reads the serverbound packets, and the player who sees the result
  of the main-hand packet.
- **Inputs:** a main-hand packet that the client is about to send.
- **Behavior:**
  1. Before the client sends the main-hand packet, it does step 2 of CHB-SYN-001.
  2. Thus the server applies the main-hand packet to the slot that the HUD shows as selected.
- **Postconditions:**
  - A player presses the key 3 and the swap key in one client tick. The `player_action` with the
    status 7 swaps the off-hand slot with the slot 2.
  - A player presses the key 3 and the attack key on a block in one client tick. The
    `player_action` with the status 0 comes after the `set_carried_item` with 2.
  - This agrees with `client-block-breaking.md`, CBK-PKT-004.

Provenance: `design`. Kind: deviation. Confidence: high.

### CHB-SYN-003: The client applies `set_held_slot`

- **Observer:** a player who sees the HUD, and a server that sends `set_held_slot`.
- **Inputs:** a clientbound `set_held_slot` with the VarInt `v`.
- **Behavior:**
  1. When `v` is in the range `0..8`, the selected slot becomes `v`.
  2. When `v` is out of the range, the client ignores the packet. The selected slot does not
     change, and the client keeps the connection.
  3. The packet does not change the last sent value.
  4. The client applies the packet in each game mode, also while a screen is open.
- **Postconditions:**
  - After step 1, the client sends `v` back at the next step 2 of CHB-SYN-001 when `v` is not the
    last sent value. A vanilla server keeps its slot for this echo (`inventory.md`, INV-SEL-002).
  - The echo keeps the two sides equal when a `set_carried_item` and a `set_held_slot` cross.
    Example: the client sends 5, and the server sends 2 before it reads the 5. The server sets 5,
    and then the echo sets 2. The client and the server end at 2.
  - A server sends the slot 4 at the join. The client then sends `set_carried_item` with 4.
  - A player in spectator mode does not see the change. The HUD shows it after a change to another
    game mode (CHB-MODE-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### CHB-SYN-004: The selected slot of a new local player

- **Inputs:** a `login` packet or a `respawn` packet.
- **Behavior:**
  1. After a `login` packet, the selected slot is 0 until the first `set_held_slot`.
  2. After a `respawn` packet, the selected slot until the next `set_held_slot` is unspecified. The
     vanilla client sets it to 0. An implementation can keep the old value.
- **Postconditions:** the server sends `set_held_slot` in the same tick as the `respawn` packet
  (`inventory.md`, INV-SEL-003; `dimension-change.md`, row 16 of the sequence). The level loading
  screen covers the HUD at that time. Thus a player does not see the value of step 2.

Provenance: `source_suggested`. Kind: unspecified. Confidence: medium.

---

## 4. The hotbar in the HUD

### CHB-HUD-001: The hotbar in survival, adventure and creative mode

- **Inputs:** a frame in a world. The HUD game mode is survival, adventure or creative.
- **Behavior:** the HUD draws these parts of the hotbar:
  1. the background of the nine hotbar slots;
  2. the selection frame at the selected slot;
  3. the frame of the off-hand slot, when the off-hand slot does not hold the empty stack;
  4. the stack of each hotbar slot and of the off-hand slot;
  5. the selected item name, and the hotbar attack indicator, when their own rules show them.
- **Postconditions:** the HUD reads the selected slot in each frame. A change shows in the next
  frame.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

### CHB-HUD-002: No hotbar in spectator mode

- **Inputs:** a frame in a world. The HUD game mode is spectator.
- **Behavior:**
  1. The HUD does not draw the parts 1 to 5 of CHB-HUD-001.
  2. While the spectator menu does not show, the HUD draws nothing in the place of the hotbar.
  3. While the spectator menu shows, the HUD draws the spectator menu in the place of the
     hotbar. The key `key.spectatorHotbar` and a hotbar key (CHB-KEY-001) open it. The wheel
     does not open it (PHY-INP-022, step 9). This document does not state its draw.
- **Postconditions:**
  - The hotbar shows again in the first frame after a change to another game mode. The selection
    frame is at the selected slot (CHB-MODE-002).
  - The stacks of the player inventory do not change because of this rule.
  - The other parts of the HUD, for example the hearts and the experience bar, have their own
    rules. This rule does not hide them.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

---

## 5. Known vanilla quirks

None.

---

## 7. Intentional deviations

### CHB-DEV-001: `set_carried_item` before each main-hand packet

- **Rule:** CHB-SYN-002.
- **Reference:** the vanilla client sends `set_carried_item` at the start of each client tick,
  and before most main-hand packets. It handles the hotbar keys later in the same client tick.
  It does not send `set_carried_item` before `player_action` with the status 0 (the start of a
  break) and with the status 7 (the swap with the off hand). A hotbar key and one of these
  actions in one client tick thus send the action before the new slot.
- **Reason:** the server then applies the action to the slot of the last tick, and not to the slot
  that the player sees. A swap moves the wrong stack. A break that the client predicts as
  instant can fail on the server, and the block comes back.
- **Visible to the client:** yes. A fast player can press a hotbar key and the swap key or the
  attack key in one client tick.
- **Bug reference:** none.
- **Test:** a differential test of the serverbound packets ignores the position of a
  `set_carried_item` with respect to a `player_action` in the same client tick. It compares the
  value of each `set_carried_item`.

---

## 8. Unresolved

None.
