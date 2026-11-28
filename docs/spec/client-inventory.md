# The inventory menu in the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states what the client does with the inventory menu. It states how a gesture in
the inventory screen becomes a click, and what the client applies to its own stacks before the
server answers. It states the fields of the `container_click` packet that the client sends, and
what the client does with the slot packets of the server. It also states the close of the
inventory menu, and the drop and the hand swap outside a screen. The document does not prescribe
an architecture.

`inventory-transactions.md` states the request, the operations and the click kinds on the
server. `inventory.md` states the slots of a player, the inventory menu and the state id.
`item-stack.md` states the stack and the hashed stack. This document refers to their rules by id
and does not state them again.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**: as `login.md`,
  section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **saved type**, **saved form**, **value entry**,
  **hashed stack**: as `item-stack.md`, section 0.1, defines them.
- **Slot**, **cursor**, **request**, **rejection**, **operation**, **clicked slot**, **drag**: as
  `inventory-transactions.md`, section 0.1, defines them. In this document, the cursor is the
  cursor of the local player as the client holds it.
- **Menu**, **inventory menu**, **open menu**, **state id**, **selected slot**, **off-hand slot**,
  **craft grid**, **craft result**, **player inventory**, **infinite materials**, **client view**:
  as `inventory.md`, section 0.1, defines them. The comparison is the comparison of INV-SYN-003.
- **Local player**: as `client-camera.md`, section 0.1, defines it.
- **Tick**, **frame**: as `client-entities.md`, section 0.1, defines them.
- **Press**, **spectator**, **hotbar keys**, **HUD**, **HUD game mode**: as `client-hotbar.md`,
  section 0.1, defines them.
- **Creative flag**: as `client-block-breaking.md`, section 0.1, defines it.
- **Screen**, **in a world**: as `client-widget.md`, section 0.1, defines them.
- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Mouse position**: as `client-tooltip.md`, section 0.1, defines it. The pointer is at the mouse
  position.
- **Edit box**: as `client-edit-box.md`, section 0.1, defines it.
- **Menu screen**, **container part**: as `client-container-screen.md`, section 0.1, defines them.
- **Inventory screen**: the screen of the vanilla client that shows the inventory menu. A press
  of `key.inventory` opens it.
- **Recipe book**: the panel of the vanilla client that lists the recipes of the player beside the
  inventory screen.
- **Recipe book button**: the button of the inventory screen that shows or hides the recipe book.
- **Window area**: the part of the inventory screen that the background image of the menu covers.
  CINV-LAY-001 states its bounds.
- **Entry area**: the part of the window area that shows one entry of the menu. CINV-LAY-002
  states the entry area of each entry.
- **Empty area**: the part of the window area that is not in an entry area.
- **Target entry**: the entry of the open menu whose entry area holds the mouse pointer. When the
  pointer is not in an entry area, there is no target entry.
- **Mouse press**, **mouse release**: the down transition and the up transition of one mouse
  button while the inventory screen is open.
- **Key event**: while the inventory screen is open, the down transition of a key, or a repeat of
  the operating system for a key that stays down.
- **Shift**, **Control**: the state of the left or the right Shift key, and of the left or the
  right Control key, at the time of the event. Control is the Control key on each operating
  system, also on macOS.
- **Button number**: the number that a mouse button gives to a click. The left button gives 0,
  the right button 1 and the middle button 2. The fourth mouse button gives 4, the fifth gives 5,
  and each further button gives the next number.
- **Pointer index**: the slot index that the mouse pointer names. It is the menu index of the
  target entry, -999 outside the window area, and -1 in the empty area (CINV-LAY-003).
- **Cursor stack**: the stack of the cursor.
- **Slot packet**: a clientbound `container_set_slot`, `container_set_content`, `set_cursor_item`
  or `set_player_inventory` packet.
- **Click**: one `container_click` packet that the client sends. A click has a click kind, a slot
  index and a button (`inventory-transactions.md`, section 6).
- **Local result**: the stacks of the entries of the open menu and the cursor stack after the
  client applies a click to its own stacks (CINV-PRD-001).
- **Unhashable component**: a component whose type is not a saved type (STK-HSH-008).
- **Unhashable stack**: a stack that has one or more unhashable components.

`P[i]` and `M[i]` are as `inventory.md`, section 0.5, states.

### 0.2 Provenance, kind and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule states its label, its kind and its confidence.

The observer of the rules of sections 2, 6 and 7, and of CINV-LAY-004, is the server that reads
the serverbound packets: a vanilla server. The observer of the other rules is a player in normal
play who sees the inventory screen and the HUD, unless the rule names another observer.

The fixture `inventory_packets` of `tools/fixtures.json` confirms the wire layout of
CINV-PKT-001. The codecs of the game write it. Section 11 lists the experiments that the other
rules need.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CINV-GES` | a gesture in the inventory screen and the click that it gives |
| `CINV-PKT` | the fields of the `container_click` packet |
| `CINV-PRD` | the local result of a click |
| `CINV-RCV` | the slot packets of the server |
| `CINV-HSH` | the hashed stack of an unhashable stack |
| `CINV-CLS` | the close of a menu |
| `CINV-HND` | the drop and the hand swap outside a screen |
| `CINV-LAY` | the window area, the entry areas and the pointer index of the inventory screen |
| `CINV-DEV` | intentional deviations |

### 0.4 Scope

In scope:

- each gesture in the inventory screen that gives a click: a mouse press, a mouse release and a
  key event;
- the container id, the state id, the changed slots and the cursor of the click;
- the local result of a click;
- the packets `container_set_slot`, `container_set_content`, `set_cursor_item` and
  `set_player_inventory` in the client;
- the hashed stack of an unhashable stack;
- the close of the inventory menu and of another menu, by the client and by the server;
- the drop key and the swap key outside a screen;
- the window area, the entry areas and the pointer index of the inventory screen while the
  recipe book does not show, and the area of the recipe book button.

Out of scope. `client-container-screen.md` states each of these items:

- the drag, the slots that it adds and the count preview of a drag;
- the double click, its time limit, and the pick up all click that it gives;
- the container screens of the generic menus, their textures, their geometry, and the bounds of
  their window area and of an entry area;
- the parts of the item tooltip of a stack, for the component types of `item-stack.md`.

`client-creative-inventory.md` states the creative screen, and when it replaces the inventory
screen.

A later specification states each of these other items:

- the container screens of the other menus;
- the other parts of the item tooltip, its image and its style;
- the recipe book: when it shows, its panel, the window area and the pointer index while it
  shows, and a mouse press that the recipe book takes;
- the focus of an entry by the keyboard, and the activation key on a focused entry;
- a click that an item takes for itself (`inventory-transactions.md`, ITX-JAV-008, row 1), and the
  take filter of the craft result;
- the content of the craft result after a change of the craft grid;
- the inventory screen of a mount;
- the short growth animation of a stack in the hotbar after a slot packet.

### 0.5 Constants

| Value | Data | Rule |
|-------|------|------|
| Container id of the inventory menu | 0 | CINV-PKT-002 |
| Pointer index outside the window area | -999 | CINV-GES-001, CINV-GES-002 |
| Pointer index in the empty area | -1 | CINV-GES-002 |
| Button of the swap with the off-hand key | 40 | CINV-GES-003 |
| Button number of the middle mouse button | 2 | CINV-GES-001, CINV-GES-002 |
| Maximum number of changed slots in one click | 128 | CINV-PKT-001 |
| `key.inventory`: default key | E | CINV-CLS-001 |
| `key.drop`: default key | Q | CINV-GES-004, CINV-HND-001 |
| `key.swapOffhand`: default key | F | CINV-GES-003, CINV-HND-002 |
| `key.pickItem`: default key | the middle mouse button | CINV-GES-001 |
| Size of the window area | 176 × 166 | CINV-LAY-001 |
| Size of an entry area | 16 × 16 | CINV-LAY-002 |
| Corner and size of the recipe book button, relative to the window area | (104, 61), 20 × 18 | CINV-LAY-004 |

---

## 1. Gestures

### CINV-GES-001: A mouse press with the empty cursor stack

- **Inputs:** a mouse press of the button `b` while the cursor stack is the empty stack.
- **Behavior:** the first row whose condition is true gives the result. "Pick with creative"
  means: `b` is the button of `key.pickItem`, and the creative flag is on.

| # | Button `b` | Condition | Result |
|---|------------|-----------|--------|
| 1 | not left, not right, not pick with creative | `b` is bound to `key.swapOffhand` or to a hotbar key | CINV-GES-003 |
| 2 | not left, not right, not pick with creative | | no click |
| 3 | each other button | the pointer is in the empty area | no click |
| 4 | pick with creative | | clone, the pointer index, the button number |
| 5 | left or right | the pointer is outside the window area | throw, slot index -999, the button number |
| 6 | left or right | Shift is down | quick move, the pointer index, the button number |
| 7 | left or right | | pick up, the pointer index, the button number |

- **Postconditions:**
  - The client sends the click at the mouse press. The next mouse release then gives no click,
    unless the release ends a double click (section 0.4).
  - A click on an entry that holds the empty stack is a click. Rows 6 and 7 do not read the stack
    of the target entry.
  - The clicks of row 4 with the slot index -999 and of row 5 have no operation on each server
    (ITX-JAV-012, ITX-JAV-013). The server still compares after them (ITX-JAV-004).
  - Row 4 occurs only before the creative screen replaces the inventory screen (section 0.4).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-GES-002: A mouse press and a mouse release with a cursor stack

- **Inputs:** a mouse press or a mouse release of the button `b` while the cursor stack is not the
  empty stack.
- **Behavior:**
  1. A mouse press with a cursor stack opens a drag when two conditions are true. The button is
     left, right, or pick with creative. The pointer is not in the empty area.
  2. A mouse press with a cursor stack gives no click.
  3. A mouse release of a button that is not the button of the open drag closes the drag. It gives
     no click. The mouse release that comes next also gives no click, for each button, unless
     it gives a double click. `client-container-screen.md`, CCS-DRG-004, states the order of
     these checks.
  4. The drag adds a slot only when the pointer moves while the button is down. When the drag
     holds one slot or more at the mouse release of its button, the drag gives the result. Section
     0.4 puts the drag out of scope.
  5. For each other mouse release, the first row of the table below whose condition is true gives
     the result. This includes a mouse release with no open drag.

| # | Button `b` | Condition | Result |
|---|------------|-----------|--------|
| 1 | the button of `key.pickItem` | | clone, the pointer index, the button number |
| 2 | each other button | the pointer is outside the window area | pick up, slot index -999, the button number |
| 3 | each other button | Shift is down | quick move, the pointer index, the button number |
| 4 | each other button | | pick up, the pointer index, the button number |

- **Postconditions:**
  - Row 2 drops the cursor stack with the button 0, and one item of it with the button 1
    (ITX-JAV-007). Shift has no effect outside the window area.
  - Row 1 occurs also when the creative flag is off. The server then does nothing (ITX-JAV-012).
  - Take a press and a release of the left button on one entry with a cursor stack. The pointer
    does not move. These give one pick up click at the release.
  - A mouse release with no mouse press after the inventory screen opened gives no click.
  - A mouse release after a press on the recipe book button gives no click (CINV-LAY-004).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-GES-003: The hotbar keys and the off-hand key

- **Inputs:** one of these events:
  - a key event of `key.swapOffhand` or of a hotbar key `key.hotbar.k` that no edit box takes
    (CINV-GES-005);
  - a mouse press of a button that is bound to one of these key bindings, when row 1 of
    CINV-GES-001 gives the result.
- **Behavior:** `i` is the pointer index.
  1. When the cursor stack is not the empty stack, or no target entry exists, the event gives no
     click.
  2. For `key.swapOffhand`, the click is: swap, slot index `i`, button 40.
  3. For `key.hotbar.k`, the click is: swap, slot index `i`, button `k − 1`.
  4. When one key is bound to `key.swapOffhand` and to a hotbar key, step 2 gives the result.
- **Postconditions:**
  - The selected slot does not change (`client-hotbar.md`, CHB-KEY-002).
  - A click occurs also when the target entry and `P[k − 1]` hold the empty stack, and when the
    target entry is the entry of `P[k − 1]`.
  - `inventory.md`, INV-MNU-003, states the slot that the button names.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-GES-004: The drop key and the pick key in the screen

- **Inputs:** a key event of `key.drop` or of `key.pickItem` that no edit box takes
  (CINV-GES-005). A target entry exists, and it holds a stack that is not empty. `i` is the pointer
  index.
- **Behavior:**
  1. A key event of `key.pickItem` while the creative flag is on gives the click: clone, slot index
     `i`, button 0.
  2. Else, a key event of `key.drop` gives the click: throw, slot index `i`. The button is 1 when
     Control is down, and 0 in the other case.
- **Postconditions:**
  - The click of step 2 occurs also when the cursor stack is not the empty stack. The server then
    does nothing (ITX-JAV-013).
  - Each repeat of the operating system for a `key.drop` that stays down gives one more click.
  - The drop key in the screen does not send `player_action` (CINV-HND-001).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-GES-005: Events that give no click

- **Inputs:** an event in the inventory screen that no rule of this section names. Also, a key
  event while an edit box of the screen has the focus.
- **Behavior:**
  1. The event gives no click.
  2. While an edit box of the screen has the focus, each key event goes to the edit box. The
     Escape key is the exception. CINV-GES-003 and CINV-GES-004 do not apply to these events.
  3. A key event of the Escape key, or of `key.inventory` that no edit box takes, closes the menu
     (CINV-CLS-001). It gives no click.
- **Postconditions:** each click that the client sends comes from a row of CINV-GES-001 to
  CINV-GES-004, from a drag or from a double click.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 2. The click packet

### CINV-PKT-001: The fields of `container_click`

- **Inputs:** a click.
- **Behavior:** the serverbound `container_click` packet has these fields, in this order:

| Field | Form | Value |
|-------|------|-------|
| container id | VarInt | CINV-PKT-002 |
| state id | VarInt | CINV-PKT-003 |
| slot index | signed 16-bit integer, big-endian | the slot index of the click |
| button | signed byte | the button of the click |
| click kind | VarInt | the wire value of ITX-JAV-006 |
| number of changed slots, `N` | VarInt, `0..128` | CINV-PKT-004 |
| changed slots | `N` times: menu index (signed 16-bit integer, big-endian), then a hashed stack (STK-HSH-001) | CINV-PKT-004 |
| cursor | hashed stack | CINV-PKT-005 |

- **Postconditions:**
  - A vanilla server gives a decode error for `N` larger than 128. The inventory menu has 46
    entries, so a click of the inventory menu has at most 46 changed slots.
  - The order of the changed slots has no meaning.

Provenance: `source_suggested_black_box_confirmed` (fixture `inventory_packets`, case
`container_click`). Kind: wire. Confidence: high.

### CINV-PKT-002: The container id

- **Inputs:** a click.
- **Behavior:**
  1. The container id is the container id of the open menu. For the inventory menu, it is 0
     (INV-MNU-001).
  2. The client sends a click only for the open menu.
- **Postconditions:** a vanilla server ignores a click with another container id (ITX-JAV-001,
  check 1).

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

### CINV-PKT-003: The state id

- **Inputs:** a click, and the state id that the client holds for the open menu.
- **Behavior:**
  1. The client holds a state id for each menu. For a new menu, it is 0.
  2. The client applies a `container_set_slot` or a `container_set_content` to a menu. Then the
     state id of that menu becomes the state id of the packet (CINV-RCV-001, CINV-RCV-002).
  3. The click carries the state id that the client holds for the open menu at the time of the
     click.
  4. The client does not change a state id for any other cause. A click, `set_cursor_item` and
     `set_player_inventory` do not change it.
- **Postconditions:**
  - Two clicks with no `container_set_slot` and no `container_set_content` between them carry the
    same state id.
  - A new local player after a `respawn` packet has a new inventory menu, with the state id 0 until
    the next `container_set_content` (INV-SYN-005).
  - When the state id of the click is not the state id of the server, the server sends the full
    content (ITX-JAV-004, row 1).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-PKT-004: The changed slots

- **Inputs:** the stack of each entry of the open menu before the click, and the local result.
- **Behavior:**
  1. An entry is changed when its stack in the local result is not equal to its stack before the
     click (STK-EQ-002).
  2. The list holds each changed entry one time, with its menu index and the hashed stack of its
     stack in the local result.
  3. The list holds no other entry.
  4. CINV-HSH-001 states the hashed stack of an unhashable stack.
- **Postconditions:**
  - An entry that changes and then returns to its first stack in one click is not in the list.
  - For each entry that is not in the list, the server compares the stack that it sent last
    (ITX-JAV-004).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-PKT-005: The cursor

- **Inputs:** the local result.
- **Behavior:** the cursor field is the hashed stack of the cursor stack in the local result. The
  client sends it in each click, also when the cursor stack did not change. The empty stack is the
  byte `00` (STK-HSH-001).
- **Postconditions:** the server compares its cursor stack with this field after each click
  (ITX-JAV-004).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-PKT-006: A click that changes nothing

- **Inputs:** a click whose local result is equal to the stacks before the click. Three examples: a
  click whose request has a rejection (CINV-PRD-002), a click whose request has no operation, and
  a click of a spectator (CINV-PRD-003).
- **Behavior:** the client sends the click. By CINV-PKT-004 and CINV-PKT-005, the list of changed
  slots is empty, and the cursor field holds the cursor stack before the click.
- **Postconditions:** the server applies its own request. When its result differs from the stacks
  of the client, it sends each stack that differs (ITX-JAV-004).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Reason: the stacks of
the server can differ from the stacks of the client. Thus only the server can decide that a
click changes nothing.

### CINV-PKT-007: The order of the clicks and the close

- **Inputs:** two or more events that each give a click or a close of CINV-CLS-001 or CINV-CLS-004,
  in one frame or in more frames. A tick of these rules is an event at the time when the client
  runs it.
- **Behavior:**
  1. The client handles each event at once, in the order of the events.
  2. The client sends the `container_click` packets and the `container_close` packet in the order
     of the events that give them.
  3. The client makes the local result of a click before it sends the click.
  4. The local result of a click is the start of the next click.
  5. When a click comes before a close, the client makes the local result of the click before the
     close. CINV-CLS-002 or CINV-CLS-004 then applies to that local result.
- **Postconditions:**
  - The server applies the clicks and the close in the same order (ITX-MOD-005, INV-CLS-001).
  - After a close, no event gives a click until the inventory screen or a menu screen opens again. A gesture that
    the close interrupts gives no click. An example is a mouse press with a cursor stack, then a
    key event of `key.inventory` before the mouse release.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

---

## 3. The local result

### CINV-PRD-001: The client applies the click to its own stacks

- **Inputs:** a click, the stacks of the entries of the open menu, the cursor stack, the creative
  flag and the HUD game mode. The local player is not a spectator. CINV-PRD-003 states the click of
  a spectator.
- **Behavior:**
  1. The client makes the request of the click with the rules of `inventory-transactions.md`,
     ITX-JAV-006 to ITX-JAV-013, from its own stacks.
  2. Infinite materials is the creative flag. The filters and the slot limits are those of
     `inventory.md`, section 1. The take filter of INV-SLT-004 reads the HUD game mode.
  3. The client applies the request with the operations of `inventory-transactions.md`, section 3,
     and the `set` operation of INV-CRE-005 for a clone. The result is the local result.
  4. The stacks of the entries and the cursor stack become the local result at once.
- **Postconditions:**
  - The inventory screen shows the local result in the next frame. It does not wait for the
    server.
  - A dropped stack (ITX-OP-020) leaves the stacks of the client. The client makes no item entity
    for it. The server makes the item entity (IEN-SPN-001).
  - The client does not do the checks of ITX-JAV-001. A gesture gives only a slot index that the
    menu has, -999 or -1.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-PRD-002: A request of the client with a rejection

- **Inputs:** a request of CINV-PRD-001 that has a rejection (ITX-REJ-001).
- **Behavior:**
  1. No stack of the client changes.
  2. The local result is the stacks before the click.
  3. The client sends the click as CINV-PKT-006 states.

Provenance: `design`. Kind: semantic. Confidence: high. Reason: a request applies in full or not
at all (ITX-ATM-002), on the client as on the server.

### CINV-PRD-003: A click of a spectator

- **Observer:** a player who sees the inventory screen, and the server that reads the click.
- **Inputs:** a press of `key.inventory` in a world with no open screen, or a gesture in the
  inventory screen. The local player is a spectator at the time of the press or the gesture.
- **Behavior:**
  1. A press of `key.inventory` opens the inventory screen also for a spectator.
  2. Each gesture of section 1 gives the same click as for a local player who is not a spectator.
  3. The client does not apply the click to its own stacks. The local result is the stacks before
     the click.
  4. The client sends the click as CINV-PKT-006 states. The list of changed slots is empty. The
     cursor field holds the cursor stack before the click.
- **Postconditions:**
  - A vanilla server changes no slot. It sends a `container_set_content` with each entry, the
    cursor stack and its state id (ITX-JAV-001, check 2, and INV-SYN-005).
  - The client applies that packet (CINV-RCV-002). The inventory screen shows no change of a stack
    at any time.
  - The server can hold a game mode that is not spectator, for example after a change of the game
    mode that the client has not received yet. The server then applies the click. It sends each
    stack that differs from the claim of the click (ITX-JAV-004). The client applies these packets.
  - A gesture after a change of the game mode follows the new game mode.

Provenance: `design`. Kind: deviation (CINV-DEV-003). Confidence: high. Steps 1 and 2, the send of
the click in step 4, and the first postcondition: `source_suggested`, kind client agreement.
Reason: a vanilla server never applies the click of a spectator. A local result then shows stacks
that the server does not hold until its answer arrives. The hand actions of section 7 also have no
local result for a spectator.

---

## 4. Packets from the server

### CINV-RCV-001: `container_set_slot`

- **Observer:** a player who sees the inventory screen, and the server that reads the next click.
- **Inputs:** a clientbound `container_set_slot` with a container id, a state id, a menu index and
  a stack.
- **Behavior:**
  1. When the container id is 0, the packet applies to the inventory menu. This is also true while
     another menu is open.
  2. When the container id is the container id of the open menu, the packet applies to the open
     menu.
  3. In each other case, the client ignores the packet.
  4. The packet sets the stack of the entry with the menu index, and the state id of the menu
     (CINV-PKT-003).
- **Postconditions:** CINV-RCV-005 states a packet that arrives after a click.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-RCV-002: `container_set_content`

- **Observer:** a player who sees the inventory screen, and the server that reads the next click.
- **Inputs:** a clientbound `container_set_content` with a container id, a state id, a list of
  stacks and a cursor stack (INV-SYN-005).
- **Behavior:**
  1. The packet applies to a menu as steps 1 to 3 of CINV-RCV-001 state. A packet for a menu that
     is not open and that is not the inventory menu changes nothing.
  2. The stack with the list index `j` becomes the stack of the entry with the menu index `j`.
  3. The state id of the menu becomes the state id of the packet.
  4. When the packet applies to the open menu, the cursor stack becomes the cursor stack of the
     packet.
- **Postconditions:**
  - A packet with the container id 0 while another menu is open changes the entries of the
    inventory menu. The cursor stack after it is unspecified.
  - CINV-RCV-006 states a list whose length is not the number of entries of the menu.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. The cursor stack of
the first postcondition: kind unspecified. Reason: the client has one cursor, and the open menu
owns it. A vanilla server sends this packet with the container id 0 only while the inventory menu
is open (INV-SYN-005). Thus a parity test must not compare the cursor stack of the other case.

### CINV-RCV-003: `set_cursor_item`

- **Inputs:** a clientbound `set_cursor_item` with a stack.
- **Behavior:** the cursor stack becomes the stack of the packet, for the open menu.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-RCV-004: `set_player_inventory`

- **Inputs:** a clientbound `set_player_inventory` with an index and a stack.
- **Behavior:**
  1. The client applies each index from 0 to 40, which are the indices of INV-SLT-001. The index
     40 is the off-hand slot.
  2. The stack becomes the stack of `P[i]`, where `i` is the index of the packet.
  3. Each entry of a menu that shows `P[i]` shows the new stack.
- **Postconditions:** CINV-RCV-006 states an index that is not in INV-SLT-001.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-RCV-005: A packet after a click

- **Observer:** the server that reads the next click.
- **Inputs:** a packet of CINV-RCV-001 to CINV-RCV-004 that arrives after the client sent a click.
- **Behavior:**
  1. The client applies each packet when it arrives, in the order of arrival.
  2. The client keeps no record of a click after it sends the click. A click has no answer packet
     of its own.
  3. A packet replaces a stack of the local result of a click, also when the server has not read
     that click yet.
  4. The client does not apply a local result again after a packet.
- **Postconditions:**
  - The server can send a `container_set_slot` before it reads a click of the client. The client
    then applies the packet and its new state id. The click carries the older state id, so the
    server sends the full content (ITX-JAV-004, row 1). The client applies it by CINV-RCV-002.
  - Assume that the server sends no `container_set_slot` and no `container_set_content` between the
    local result and its read of the click. Then the click carries the state id of the server. The
    server then sends only the stacks that differ from the claim of the click (ITX-JAV-004, rows 2
    to 4).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Reason: the state id
finds the lost update. A client that keeps an old local result after a packet shows a stack that
the server does not hold.

### CINV-RCV-006: A slot packet that names no slot

- **Inputs:** a packet of CINV-RCV-001, CINV-RCV-002 or CINV-RCV-004 for a menu that the packet
  applies to.
- **Behavior:**
  1. A `container_set_slot` whose menu index is not an index of the menu changes nothing. Its
     state id does not apply.
  2. In a `container_set_content` with more stacks than entries, the client ignores each stack
     after the last entry.
  3. In a `container_set_content` with fewer stacks than entries, each entry without a stack in
     the packet keeps its stack.
  4. A `set_player_inventory` whose index is less than 0 or more than 40 changes nothing.
  5. The client keeps the connection in each case.

Provenance: `design`. Kind: deviation (CINV-DEV-001). Confidence: high. Step 3, and step 4 for an
index more than 42: `source_suggested`, kind client agreement.

---

## 5. The unhashable stack

### CINV-HSH-001: The hashed stack of an unhashable stack

- **Observer:** the server that compares the click (ITX-JAV-004).
- **Inputs:** an unhashable stack that the client must write as a hashed stack: the stack of a
  changed entry (CINV-PKT-004) or the cursor stack (CINV-PKT-005).
- **Behavior:**
  1. The client writes a present hashed stack with the item id of `minecraft:air`, the count 1, no
     value entries and no removal entries.
  2. The client sends the click. The click does not stop, and the client keeps the connection.
- **Postconditions:**
  - The hashed stack of step 1 matches no stack of the server. A stack that is not empty never has
    the item air (STK-EMP-001, STK-HSH-003).
  - A server that follows STK-HSH-003 thus sends the stack that it holds for the entry or for the
    cursor (ITX-JAV-004). The client applies it (CINV-RCV-001, CINV-RCV-003).
  - A vanilla server can give an internal error at the comparison (STK-HSH-008).
  - The client does not compute the hash of a stack that is not in the claim of the click. An
    unhashable stack in an entry that the click does not change has no effect.

Provenance: `design`. Kind: deviation (CINV-DEV-002). Confidence: medium. This rule does not
depend on Q4 of `item-stack.md`. Reason: a hash that the client guesses can match a different
stack of the server. The client then keeps a stack that the server does not hold. A value entry
that the client leaves out has the same risk, for a stack of the server that does not have that
component. A claim that matches no stack always gets the stack of the server.

---

## 6. The close of a menu

### CINV-CLS-001: The client closes the inventory menu

- **Inputs:** the inventory menu is the open menu, the inventory screen is open, and one of these
  events occurs:
  - a key event of `key.inventory` that no edit box takes (CINV-GES-005);
  - a key event of the Escape key;
  - a tick in which the local player is dead or is removed from the world;
  - a tick in which the local player is inside a portal that shows the portal effect, for
    example a nether portal.
- **Behavior:**
  1. The client sends the serverbound `container_close` packet with the container id 0.
  2. The client closes the inventory screen.
  3. The open menu stays the inventory menu.
  4. CINV-CLS-002 states the stacks.
- **Postconditions:** the vanilla server then does the steps of INV-CLS-001. It sends no packet as
  an answer to `container_close` (INV-CLS-002).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-CLS-002: The stacks at a close of the inventory menu

- **Observer:** a player who sees the inventory screen and the HUD.
- **Inputs:** a close of the inventory menu by CINV-CLS-001 or by CINV-CLS-003.
- **Behavior:**
  1. The client does not move the cursor stack. The cursor stack stays until a packet of the
     server changes it.
  2. The client does not move the stacks of the craft grid. Each stays until a packet of the
     server changes it.
  3. The stack of the craft result, `M[0]`, becomes the empty stack.
- **Postconditions:**
  - After a close of the client with a cursor stack, a vanilla server puts the cursor stack into
    the player inventory (INV-CLS-001). The comparison sends a `container_set_slot` for each entry
    that changed, and a `set_cursor_item` with the empty stack (INV-SYN-003, INV-SYN-009). The
    client applies them (section 4).
  - The cursor stack does not show while no screen is open. When the player opens the inventory
    screen again before these packets arrive, the screen shows the old cursor stack.
  - At a close, the client does not compute the insert rule of INV-INS-001.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Reason: the server
decides where each stack goes (INV-CLS-004, INV-INS-003). A prediction of the client can show
a stack in a slot that the server does not fill.

### CINV-CLS-003: The server closes the menu

- **Observer:** a player who sees the screen, and a server that closes the screen of the player.
- **Inputs:** a clientbound `container_close` packet.
- **Behavior:**
  1. The client closes the open menu and its screen, for each container id of the packet.
  2. The open menu becomes the inventory menu.
  3. The client sends no `container_close`.
  4. When the open menu was the inventory menu, CINV-CLS-002 states the stacks.
  5. When the open menu was another menu, steps 3 and 4 of CINV-CLS-004 state the stacks.
- **Postconditions:** a server can close the inventory screen of the player with the container
  id 0. A vanilla server never does this (INV-CLS-002).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CINV-CLS-004: The client closes another menu

- **Observer:** the server that reads the serverbound packets, and a player who sees the HUD.
- **Inputs:** a menu other than the inventory menu is the open menu, and its menu screen is open.
  One of the events of CINV-CLS-001 occurs. A key event then occurs in the menu screen in place of
  the inventory screen.
- **Behavior:**
  1. The client sends the serverbound `container_close` packet with the container id of the open
     menu. This is the container id of the `open_screen` packet that opened the menu (CCS-OPN-001).
  2. The client closes the menu screen. No screen is then open.
  3. The cursor stack becomes the empty stack (INV-SYN-009).
  4. The client discards the stacks of the container part. It does not move a stack of the
     container part or the cursor stack into the player inventory.
  5. The open menu becomes the inventory menu. The stacks of the player inventory do not change.
- **Postconditions:**
  - The vanilla server then does the steps of INV-CLS-001. It sends no packet as an answer to
    `container_close` (INV-CLS-002).
  - The comparison sends a `container_set_slot` with the container id 0 for each entry that the
    old cursor stack fills. It sends no `set_cursor_item` (INV-SYN-009). The client applies these
    packets (section 4).
  - At a close, the client does not compute the insert rule of INV-INS-001. Until these packets
    arrive, no slot of the player inventory shows the stack that was in the cursor.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Reason: the server
decides where each stack goes (INV-CLS-004, INV-INS-003). After the close, the client view of the
cursor on the server is the empty stack (INV-SYN-009). The cursor stack of the client must agree
with it.

---

## 7. The hand actions outside a screen

### CINV-HND-001: The drop key outside a screen

- **Observer:** the server that reads the serverbound packets, and a player who sees the HUD.
- **Inputs:** a press of `key.drop` while the client is in a world. `n` is the selected slot.
- **Behavior:**
  1. When the local player is a spectator, the press has no result.
  2. When Control is down, the client removes all items of `P[n]`. Else it removes one item of
     `P[n]`. When `P[n]` holds the empty stack, nothing changes.
  3. The client sends `set_carried_item` when CHB-SYN-002 requires it.
  4. The client sends `player_action` with the action 4 when Control is down, and with the action
     5 in the other case. The position is 0, the face is 0 and the sequence is 0 (BRK-REQ-001,
     CBK-SEQ-003).
  5. When step 2 removed one item or more, the local player swings the main hand. The swing sends
     no packet.
- **Postconditions:**
  - The client sends step 4 also when `P[n]` holds the empty stack.
  - The server makes the request of INV-SEL-004. After the drop, the client view of `P[n]` on the
    server is the stack of the server, so a vanilla server that agrees sends no packet.
  - When a plugin denies the drop, the comparison sends `P[n]` (ITX-JAV-022). The client applies
    it (CINV-RCV-001).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.
Experiment E17 of `inventory-transactions.md` confirms step 2 for the removal of one item
(ITX-JAV-025).

### CINV-HND-002: The swap key outside a screen

- **Observer:** the server that reads the serverbound packets, and a player who sees the HUD.
- **Inputs:** a press of `key.swapOffhand` while the client is in a world. `n` is the selected
  slot.
- **Behavior:**
  1. When the local player is a spectator, the press has no result.
  2. The client sends `set_carried_item` when CHB-SYN-002 requires it.
  3. The client sends `player_action` with the action 7. The position is 0, the face is 0 and the
     sequence is 0 (BRK-REQ-001, CBK-SEQ-003).
  4. The client changes no stack. `P[n]` and `P[40]` change when the packets of the server arrive.
     The comparison of INV-SYN-003 sends them (INV-SEL-005, INV-SEL-006).
- **Postconditions:**
  - The HUD shows the swap after the round trip to the server.
  - A press while the two slots hold the empty stack also sends the packet.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.
Experiment E17 of `inventory-transactions.md` confirms step 4 (ITX-JAV-025).

---

## 8. The areas of the inventory screen

The rules of this section apply while the recipe book does not show. A later specification states
the window area and the pointer index while the recipe book shows. The bounds of an area and the
pointer test are those of `client-container-screen.md`, CCS-LAY-005 and CCS-LAY-006.

### CINV-LAY-001: The window area

- **Inputs:** W and H.
- **Behavior:**
  1. The window area is the rectangle with the left edge `left`, the top edge `top`, the width
     `iw` = 176 and the height `ih` = 166.
  2. `left` is `floor((W − 176) / 2)`, and `top` is `floor((H − 166) / 2)`.
  3. The screen draws the background of an in-game screen (`client-widget.md`, WID-SCR-004).
  4. The screen then copies the source rectangle (0, 0, 176, 166) of the texture
     `minecraft:textures/gui/container/inventory.png` to (`left`, `top`), at the same size. One
     texel is one GUI unit, and the texture is 256 × 256 texels.
- **Postconditions:**
  - With W = 427 and H = 240, the window area has the corner (125, 37).
  - The window area stays at the centre of the screen after a resize (`client-widget.md`,
    WID-SCR-003).
  - The status effects that the screen shows at the right of the window area do not move the
    window area.

Provenance: `source_suggested`. Kind: layout. Confidence: high.

### CINV-LAY-002: The entry areas

- **Inputs:** the entries of the inventory menu (`inventory.md`, INV-MNU-001).
- **Behavior:** each of the 46 entries has an entry area of 16 × 16 GUI units. The table gives
  the top left corner of the entry area, relative to (`left`, `top`). `c` is the column and `r` is
  the row, from 0.

| Menu index | Entry | Corner (x, y) |
|------------|-------|---------------|
| 0 | the craft result | (154, 28) |
| `1 + c + 2·r`, with `c` and `r` in `0..1` | the slot `c + 2·r` of the craft grid | (`98 + 18·c`, `18 + 18·r`) |
| `5 + r`, with `r` in `0..3` | `P[39 − r]`: head, chest, legs and feet, from the top | (8, `8 + 18·r`) |
| `9 + 9·r + c`, with `r` in `0..2` and `c` in `0..8` | `P[9 + 9·r + c]` | (`8 + 18·c`, `84 + 18·r`) |
| `36 + c`, with `c` in `0..8` | `P[c]` | (`8 + 18·c`, 142) |
| 45 | `P[40]`, off hand | (77, 62) |

- **Postconditions:**
  - Two entry areas do not overlap. Each entry area is in the window area.
  - Each entry area exists while the screen is open. An armour entry with the empty stack also
    has its entry area.
  - `M[0]` has the corner (`left + 154`, `top + 28`), and `M[44]` has the corner (`left + 152`,
    `top + 142`).

Provenance: `source_suggested`. Kind: layout. Confidence: high.

### CINV-LAY-003: The pointer index

- **Inputs:** the mouse position (`mx`, `my`), the window area of CINV-LAY-001 and the entry areas
  of CINV-LAY-002.
- **Behavior:** the first row whose condition is true gives the pointer index.

| # | Condition | Pointer index |
|---|-----------|---------------|
| 1 | the pointer is outside the window area (CCS-LAY-006, edge cases, with `iw` = 176 and `ih` = 166) | -999 |
| 2 | the pointer is in the entry area of `M[i]` (CCS-LAY-005, step 1) | `i`, and `M[i]` is the target entry |
| 3 | | -1, and the pointer is in the empty area |

- **Postconditions:**
  - The gap of 2 GUI units between two entry areas is a part of the empty area.
  - The picture of the player, from (`left + 26`, `top + 8`) to (`left + 75`, `top + 78`), is a
    part of the empty area. The title of the screen is also a part of the empty area.
  - At (`left + 7`, `top + 84`) the pointer index is -1. At (`left + 8`, `top + 84`) it is 9.
  - At (`left + 175`, `top + 165`) the pointer index is -1. At (`left + 176`, `top + 165`) it is
    -999.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CINV-LAY-004: The recipe book button

- **Inputs:** a mouse press while the pointer is in the area of the recipe book button.
- **Behavior:**
  1. The area of the recipe book button is `x ≤ mx < x + 20` and `y ≤ my < y + 18`. The corner
     (`x`, `y`) is (`left + 104`, `top + 61`).
  2. A mouse press of the left button in this area gives no click and opens no drag. The press
     shows the recipe book (section 0.4).
  3. The next mouse release gives no click, for each button.
  4. A mouse press of another button in this area has the result of the empty area. Section 1
     gives the result.
- **Postconditions:**
  - The area of the recipe book button is in the empty area. It overlaps no entry area.
  - This rule comes before CINV-GES-001 and CINV-GES-002.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

---

## 9. Known vanilla quirks

None.

---

## 10. Intentional deviations

### CINV-DEV-001: A slot packet that names no slot

- **Rule:** CINV-RCV-006.
- **Reference:** the vanilla client gives an internal error for each of these packets, and then
  ends the connection:
  - a `container_set_slot` with a menu index that the menu does not have;
  - a `container_set_content` with more stacks than entries;
  - a `set_player_inventory` with a negative index.

  The vanilla client puts the stack of a `set_player_inventory` with the index 41 or 42 into an
  equipment slot of the local player. These slots are the body armour slot and the saddle slot.
  No entry of the inventory menu shows them, and the player model does not show them.
- **Reason:** a packet with a bad index shows a defect of the server. The other stacks of the
  menu stay correct, so the client can continue.
- **Visible to the client:** only with a server that sends such a packet. A vanilla server does
  not.
- **Bug reference:** none.
- **Test:** a differential test does not send such a packet.

### CINV-DEV-002: The hashed stack of an unhashable stack

- **Rule:** CINV-HSH-001.
- **Reference:** the vanilla client decodes each component and has the saved form of each saved
  type. STK-HSH-008 and question Q4 of `item-stack.md` do not decide what it sends for a component
  whose type is not a saved type.
  The reference source suggests that the hash fails and the click stops with an error.
- **Reason:** a click must not stop the client. The server must correct each claim that the
  client cannot make.
- **Visible to the client:** yes, on the wire. After such a click, a server that follows
  STK-HSH-003 sends the stack of the entry or of the cursor. The player sees no change of a stack.
- **Bug reference:** none.
- **Test:** a differential test does not use a stack with a component whose type is not a saved
  type.

### CINV-DEV-003: A click of a spectator

- **Rule:** CINV-PRD-003.
- **Reference:** the vanilla client applies the click of a spectator to its own stacks, as for
  each other player. The click carries the changed slots and the cursor stack of that local result.
  The `container_set_content` of the server then puts back the stacks before the click.
- **Reason:** a spectator cannot change a slot on a vanilla server (ITX-JAV-001, check 2). The
  local result shows a stack in a place that the server does not fill.
- **Visible to the client:** yes. With the reference, the player sees the stack move, and the stack
  goes back after one round trip. With CINV-PRD-003, the player sees no change. On the wire, the
  claim of the click is empty.
- **Bug reference:** none.
- **Test:** a differential test does not compare the changed slots and the cursor field of a click
  of a spectator. It does not compare the stacks of the screen before the answer of the server.

---

## 11. Unresolved

These experiments have not run.

- **E1** (recorder, vanilla client). Open the inventory screen. Do each row of CINV-GES-001 to
  CINV-GES-005 at four places. The places are an entry with 10 stone, an empty entry, the empty
  area, and outside the window area. Do each with the empty cursor stack. Do each again with 5
  dirt in the cursor. Record the fields of each click. Expect the tables of section 1, the fields
  of CINV-PKT-002 to CINV-PKT-007, and the stacks of CINV-PRD-001.
- **E2** (recorder, vanilla client). Put 5 stone in the cursor and close the inventory screen with
  E. Record the serverbound `container_close` and the clientbound packets of the next two ticks.
  Expect the container id 0, a `container_set_slot` for the entry that gets the stone, and a
  `set_cursor_item` with the empty stack (CINV-CLS-001, CINV-CLS-002).
- **E3** (recorder with a proxy module). Delay each clientbound packet by 500 ms. Give the player
  one stone by a command. Click on an entry in the same tick. Record the state id of the click
  and the next clientbound packet. Expect the older state id and a `container_set_content`
  (CINV-RCV-001 to CINV-RCV-005).
- **E4** (proxy). With the inventory screen open, send a clientbound `container_close` with the
  container id 5. Expect the screen to close and no serverbound `container_close`
  (CINV-CLS-003). Then send a `container_set_content` with 40 stacks. Expect that the entries 40
  to 45 keep their stacks. Then send a `set_player_inventory` with the index 43. Expect no change
  of a stack (CINV-RCV-006, steps 3 and 4). Then send a `set_player_inventory` with the index 40
  and one stone. Expect the stone in the entry with the menu index 45 (CINV-RCV-004).
- **E5** (recorder, vanilla client). Outside a screen, press Q, then Control and Q, then F. Do
  each with a stack in the selected slot and with the empty stack. Do each again as a spectator.
  Record the serverbound packets. Expect CINV-HND-001 and CINV-HND-002.
- **E6** (recorder, vanilla client). Set a GUI of 427 × 240 GUI units, and open the inventory
  screen with the recipe book closed. Each entry and the cursor hold the empty stack. Press the
  left button at the corner of each entry area of CINV-LAY-002. Then press it 1 GUI unit to the
  left of each corner, and at the points of CINV-LAY-003. Record the slot index of each click.
  Expect the menu indices of CINV-LAY-002, no click in the empty area, and -999 outside the window
  area. Then put 5 dirt in the cursor. Press and release the left button at (229, 98). Expect no
  click (CINV-LAY-004).
- **E7** (recorder, vanilla client). Put 5 dirt in the cursor and 10 stone in an entry. Let the
  input agent give these events in one frame: a mouse press and a mouse release of the left
  button on that entry, then a key event of E. Record the serverbound packets and the clientbound
  packets of the next two ticks. Expect a `container_click` with 5 dirt in the entry and 10 stone
  in the cursor, then a `container_close`. Expect a `container_set_slot` for the entry that gets
  the stone, and a `set_cursor_item` with the empty stack. Open the screen again and put 5 dirt in
  the cursor. Press the left button on an empty entry, and press E before the release. Expect a
  `container_close` and no `container_click` (CINV-PKT-007).
- **E8** (recorder, vanilla client, vanilla server). Put 5 stone in the slot 0 of a chest, and open
  the chest. Press the left button on that entry, then press E. Record the serverbound
  `container_close` and the clientbound packets of the next two ticks. Expect the container id of
  the `open_screen` packet, a `container_set_slot` with the container id 0 for the entry that gets
  the stone, and no `set_cursor_item`. Open the inventory screen. Expect the stone in the player
  inventory and the empty cursor stack. Do the steps again with Escape in place of E
  (CINV-CLS-004).
- **E9** (recorder, vanilla client). Put 10 stone in `P[9]`, and change the player to spectator
  mode. Press E. Expect the inventory screen. Press the left button on the entry with the menu
  index 9. Then put the pointer on that entry and press the key 1. Record the serverbound packets
  and the clientbound packets of the next two ticks. Expect a `container_click` with pick up, slot
  index 9 and button 0, and a `container_click` with swap, slot index 9 and button 0. Expect a
  `container_set_content` after each click, with the 10 stone in the entry 9 and the empty cursor
  stack (CINV-PRD-003, steps 1 and 2, and the first postcondition).
