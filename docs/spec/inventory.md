# Player inventory: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| World version | `5127` |
| Status | `draft` |

Source of the version values: `assets/minecraft/version.json`.

This document states the slots of a player, the inventory menu, the selected slot and the insert
rule. It also states the creative inventory, the state id with the resync, and the close of a
menu. It does not prescribe an architecture.

`inventory-transactions.md` states the request, the operations and the clicks. `item-stack.md`
states the stack. `persistence.md` states the player file. This document refers to their rules by
id. Section 9 lists what this document does not state.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**: as `login.md`,
  section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **maximum stack size**: as `item-stack.md`, section 0.1,
  defines them.
- **Holder**, **slot**, **slot reference**, **cursor**, **operation**, **request**, **slot limit**,
  **capacity**, **take filter**, **place filter**, **dropped stack**, **edge**, **clicked slot**,
  **drag**, **plugin**, **denial**: as `inventory-transactions.md`, section 0.1, defines them.
- **Player inventory**: the holder of a player that has the 41 slots of INV-SLT-001.
- **Hotbar**: the slots 0 through 8 of the player inventory.
- **Main slots**: the slots 0 through 35 of the player inventory. The hotbar is a part of them.
- **Armour slots**: the slots 36 through 39 of the player inventory.
- **Off-hand slot**: the slot 40 of the player inventory.
- **Craft grid**: the holder of a player that has the 4 slots of the 2 by 2 grid.
- **Craft result**: the holder of a player that has the one result slot of the craft grid.
- **Selected slot**: the slot of the hotbar that is the main hand of the player (INV-SEL-001).
- **Menu**: an ordered list of slot references that a player has open. Each entry has a menu index.
- **Inventory menu**: the menu of INV-MNU-001. Each player always has one.
- **Open menu**: the menu that the clicks of a player go to (INV-MNU-002).
- **State id**: a number of a menu that the server and the client use to find a lost update
  (INV-SYN-001).
- **Client view**: what the server knows of the stack that the client holds for a slot
  (INV-SYN-002).
- **Alike stack**: a stack in a slot that can accept more items of a given stack (INV-INS-001).
- **Infinite materials**: the ability that a player in creative mode has.
- **Dimension change**: as `dimension-change.md`, section 0.1, defines it.
- **Cell**: a part of a dimension that the server simulates on its own. A player does not see the
  border between two cells of one dimension.
- **Cell crossing**: a move of a player from one cell to another cell of the same dimension.

### 0.2 Provenance

The labels are those of `inventory-transactions.md`, section 0.2. A rule states its kind after the
label (`README.md`, section "Semantics, not accidents").

Experiments E3, E8, E9 and E11 of section 12 have a recorded trace of the vanilla server. A rule
that such a trace confirms cites the trace by path. Each other experiment is a proposal.

### 0.3 Artefacts cited

- `assets/mcrs/reports/packets.json`: the packet id report of the game. It gives the packet names
  that this document uses.
- The data pack of the game: the `minecraft:equippable` component of each item, and the
  enchantment effect `minecraft:prevent_armor_change`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `INV-SLT` | the holders and the slots of a player, their slot limits and filters |
| `INV-MNU` | the inventory menu |
| `INV-SEL` | the selected slot and the hand actions |
| `INV-INS` | the insert rule |
| `INV-CRE` | the creative inventory and the operations that make items |
| `INV-SYN` | the state id, the client view and the packets that the server sends |
| `INV-CLS` | the cursor stack and the craft grid when a menu closes, the player leaves or the player changes dimension |
| `INV-SAV` | the player file |

### 0.5 Notation

The notation is that of `inventory-transactions.md`, section 0.5. `P[i]` is the slot `i` of the
player inventory. `M[i]` is the entry with the menu index `i` of the inventory menu.

## 1. The slots of a player

**INV-SLT-001** (`source_suggested_black_box_confirmed`, wire). The player inventory has 41 slots.
The `set_player_inventory` packet uses these indices. The `Slot` field of the player file uses
the indices `0..35` (INV-SAV-001).

| Index | Name | Slot limit | Filters |
|-------|------|------------|---------|
| `0..8` | hotbar | 99 | none |
| `9..35` | the main slots that are not in the hotbar | 99 | none |
| 36 | feet | 1 | INV-SLT-003, INV-SLT-004 |
| 37 | legs | 1 | INV-SLT-003, INV-SLT-004 |
| 38 | chest | 1 | INV-SLT-003, INV-SLT-004 |
| 39 | head | 1 | INV-SLT-003, INV-SLT-004 |
| 40 | off hand | 99 | none (INV-SLT-005) |

Experiment E1 decides the indices.

**INV-SLT-002** (`design`, semantic). A player has four holders: the player inventory, the cursor,
the craft grid and the craft result. The craft grid has the slots `0..3`, in rows from the top
left. The craft result has the slot 0. The slot limit of each of these six slots is 99. An action
that states "each slot of the player" applies to each slot of the four holders. Reason: issue 521
decides that the cursor stack is a slot of the player.

**INV-SLT-003** (`source_suggested`, semantic). The place filter of an armour slot permits a stack
when each of these conditions is true:

- the stack has the `minecraft:equippable` component;
- the `slot` of that component is the name of the armour slot;
- that component has no `allowed_entities`, or `allowed_entities` contains `minecraft:player`.

See experiment E2.

**INV-SLT-004** (`source_suggested`, semantic). The take filter of an armour slot refuses when
these two conditions are true:

- the stack of the slot has an enchantment with the effect `minecraft:prevent_armor_change`;
- the player is not in creative mode.

In each other case the take filter permits. See experiment E2.

**INV-SLT-005** (`design`, semantic). The off-hand slot has no take filter and no place filter.
It accepts each stack in each edition, and no configuration changes this. Reason: decision of the
owner in issue 521. A later version of Bedrock Edition gives the player a full second hand, so the
off hand accepts each item and mcrs needs no switch.

**INV-SLT-006** (`source_suggested_black_box_confirmed`, semantic). The place filter of the slot of
the craft result refuses each stack. Its take filter belongs to the specification of the craft
(section 9). The trace `tools/mcrs_minecraft_probe/traces/inventory_e03.json` confirms the place
filter in two steps:

- `M[0]` holds the empty stack, and the cursor holds 10 stone. A primary click on `M[0]` changes
  no slot, and the cursor keeps the 10 stone.
- A drag of 8 stone over `M[0]` and four other entries puts no stone in `M[0]` (INV-MNU-005).

**INV-SLT-007** (`design`, semantic). The filters of this section apply to a request. They do not
apply to the `set` operation (INV-CRE-001) or to a command. Reason: a command and the creative
inventory must be able to fill each slot.

**INV-SLT-008** (`design`, semantic). Until the specification of the craft exists, the slot of the
craft result holds the empty stack. A change of the craft grid does not change it. Reason: the
specification of the craft states the content of the slot (section 9). The vanilla server shows
the result of a recipe there. In the trace `tools/mcrs_minecraft_probe/traces/inventory_e03.json`,
one oak log in `M[1]` puts 4 oak planks in `M[0]`, and 2 stone in `M[2]` put a stone button in
`M[0]`. The vanilla server sends `M[0]` at each change of the craft grid, and each such packet
increases
the state id. Thus a parity test must compare neither `M[0]` nor a state id before that
specification exists.

## 2. The inventory menu

**INV-MNU-001** (`source_suggested_black_box_confirmed`, wire). The inventory menu has the
container id 0 and 46 entries.

| Menu index | Slot |
|------------|------|
| 0 | the slot of the craft result |
| `1..4` | the slots `0..3` of the craft grid |
| 5 | `P[39]`, head |
| 6 | `P[38]`, chest |
| 7 | `P[37]`, legs |
| 8 | `P[36]`, feet |
| `9..35` | `P[9]` through `P[35]` |
| `36..44` | `P[0]` through `P[8]` |
| 45 | `P[40]`, off hand |

The inventory menu has no data values. See experiment E1.

**INV-MNU-002** (`source_suggested_black_box_confirmed`, semantic). The inventory menu exists from
the join of the player until the player leaves. No packet opens it. The open menu is the inventory
menu when the player has no other menu open. The inventory menu is always valid for the player
(ITX-JAV-001, check 3). The trace `tools/mcrs_minecraft_probe/traces/inventory_e03.json` confirms
it: the join sends no `open_screen`, and each click with the container id 0 has its effect. The
trace `tools/mcrs_minecraft_probe/traces/inventory_e08.json` shows the same after a chest closes:
a pickup on `M[36]`, which holds 3 dirt, with no prediction gets `M[36]` with the empty stack and
the cursor with 3 dirt.

**INV-MNU-003** (`source_suggested`, semantic). The button of the swap click names a slot of the
player inventory (ITX-JAV-010). The buttons `0..8` name `P[0]` through `P[8]`. The button 40 names
`P[40]`. This rule applies to each menu. See experiment E10 of `inventory-transactions.md`.

**INV-MNU-004** (`source_suggested_black_box_confirmed`, semantic). A quick move in the inventory
menu has the
destination list of the first row whose condition is true (ITX-JAV-009). `e` is the `slot` of the
`minecraft:equippable` component of the stack of `S`. `A(e)` is the entry of the inventory menu
for that armour slot.

| # | Condition | Destinations, in order |
|---|-----------|------------------------|
| 1 | `S` is `M[0]`. | `M[44]` down to `M[9]` |
| 2 | `S` is one of `M[1]` through `M[8]`. | `M[9]` up to `M[44]` |
| 3 | `e` is `head`, `chest`, `legs` or `feet`, and `A(e)` holds the empty stack. | `A(e)` |
| 4 | `e` is `offhand`, and `M[45]` holds the empty stack. | `M[45]` |
| 5 | `S` is one of `M[9]` through `M[35]`. | `M[36]` up to `M[44]` |
| 6 | `S` is one of `M[36]` through `M[44]`. | `M[9]` up to `M[35]` |
| 7 | `S` is `M[45]`. | `M[9]` up to `M[44]` |

Rows 3 and 4 do not read `allowed_entities`. When the place filter of the one destination
refuses, the click moves no item. Section 11, U4, asks the owner about the kind of this result.
Deviation D4 of `inventory-transactions.md` does not show in
this menu. The trace `tools/mcrs_minecraft_probe/traces/inventory_e03.json` confirms each row:

| Source, before the click | Result |
|--------------------------|--------|
| 4 oak planks in `M[0]` | `M[44]` holds 4. |
| 64 stone in `M[1]` or in `M[45]` | `M[10]` and `M[40]` hold 64, and `M[9]` holds 16. |
| an iron helmet in `M[9]`, `M[5]` empty | `M[5]` holds the helmet. |
| an iron helmet in `M[9]`, a golden helmet in `M[5]` | `M[36]` holds the iron helmet. |
| an iron helmet with `allowed_entities` of `minecraft:zombie` in `M[9]`, `M[5]` empty | No slot changes. |
| a shield in `M[36]` | `M[45]` holds the shield. |
| 64 stone in `M[9]` | `M[40]` holds 64, and `M[36]` holds 40. |
| 64 stone in `M[36]` | `M[10]` holds 64, and `M[9]` holds 40. |
| an iron helmet in `M[5]` | `M[9]` holds the helmet. |

Before each click of the table, `M[10]` and `M[40]` hold 40 stone. In the first row, one oak log
in `M[1]` gives the 4 oak planks.

**INV-MNU-005** (`source_suggested_black_box_confirmed`, semantic). In the inventory menu, pick up
all cannot take from `M[0]` (ITX-JAV-011). The inventory menu permits a drag to each entry
(ITX-JAV-017). The trace `tools/mcrs_minecraft_probe/traces/inventory_e03.json` confirms it:

- `M[0]` holds 4 oak planks, `M[9]` holds 10, and the cursor holds 3. Pick up all on an empty slot
  takes the 10 of `M[9]` and none of `M[0]`.
- A drag of drag kind 0 with 8 stone over `M[0]`, `M[2]`, `M[5]`, `M[11]` and `M[45]` puts 2
  stone in `M[2]`, `M[11]` and `M[45]`. The place filters of `M[0]` and `M[5]` refuse the stone.

**INV-MNU-006** (`source_suggested`, semantic). A player on the server can always drop items. Thus
the last condition of ITX-JAV-013 is always true. See experiment E13 of
`inventory-transactions.md`.

## 3. The selected slot and the hand actions

**INV-SEL-001** (`source_suggested_black_box_confirmed`, semantic). Each player has a selected
slot, a number in the range `0..8`. The main hand of the player is the stack of `P[n]`, where `n`
is the selected slot. A change of the selected slot moves no stack. The selected slot of a new
player is 0.

**INV-SEL-002** (`source_suggested`, semantic). The serverbound `set_carried_item` packet has one
short. When the value is in the range `0..8`, it becomes the selected slot. When the value changes
the selected slot and the player uses the item of the main hand, that use stops. The server
ignores each other value and keeps the connection. See experiment E4.

**INV-SEL-003** (`source_suggested_black_box_confirmed`, wire). The clientbound `set_held_slot`
packet has one VarInt, the selected slot. The server sends it in each of these cases:

- when the player joins;
- when the server sends the full state of the player again, after a respawn or a change of
  dimension;
- when the server changes the selected slot itself.

The server does not send it as an answer to `set_carried_item`. See experiment E4.

**INV-SEL-004** (`source_suggested_black_box_confirmed`, semantic). The `player_action` packet
with the status 4 or 5 drops from the selected slot. `n` is the selected slot. The status 4 gives
the request `drop(P[n], count(P[n]))`. The status 5 gives the request `drop(P[n], 1)`. When
`P[n]` holds the empty stack, the server does nothing. The server ignores the packet of a
spectator. IEN-SPN-001 states the item entity. After the drop, the client view of `P[n]` is the
stack that the server holds. ITX-JAV-022 states the result when a plugin denies the request. See
experiment E5.

**INV-SEL-005** (`design`, semantic). The `player_action` packet with the status 7 gives the
request `swap(P[n], P[40])`, where `n` is the selected slot. When the two slots hold the empty
stack, the server does nothing. When the request applies and the player uses an item, that use
stops. The server ignores the packet of a spectator. Reason: a request is the only path that
changes a slot, so the filters of the two slots apply with no other rule. See experiment E5.

**INV-SEL-006** (`design`, client agreement). When the request of INV-SEL-005 has a rejection or
a denial (ITX-JAV-022), the client view of `P[n]` and of `P[40]` becomes not known. The
comparison of INV-SYN-003 then sends the two stacks. Reason: the server does not know what the
client shows for the two slots, so it sends both. ITX-JAV-021 states that the vanilla client
changes neither slot before the answer. Experiment E17 of `inventory-transactions.md` decides
it. By default, the two slots have no filter (INV-SLT-001, INV-SLT-005). Only a filter that a
plugin gives to `P[n]` or to `P[40]` can cause such a rejection.

## 4. The insert rule

**INV-INS-001** (`source_suggested_black_box_confirmed`, semantic). The insert rule puts the items
of a stack `T` into the player inventory. A slot holds an alike stack when each of these
conditions is true:

- the slot holds a stack with the item and the components of `T` (STK-EQ-001);
- that stack can stack (STK-EQ-003);
- its count is smaller than the capacity of the slot for it.

While `T` has items, the server selects the first slot of this list and moves items to it:

1. the selected slot, when it holds an alike stack;
2. the off-hand slot, when it holds an alike stack;
3. the first main slot that holds an alike stack, from `P[0]` to `P[35]`;
4. the first main slot that holds the empty stack, from `P[0]` to `P[35]`.

The slot receives as many items as its capacity permits. The insert ends when `T` has no items or
when the list gives no slot. The insert rule never fills an armour slot. It never fills an
off-hand slot that holds the empty stack. A stack with a damage value above 0 cannot stack. Thus
it goes only to a main slot that holds the empty stack. Result R16 of `item-entity.md` shows the
order.

**INV-INS-002** (`design`, semantic). The insert rule has one result: the number of items that
found no room. The caller decides what these items become. This table lists the callers.

| Caller | The items that found no room |
|--------|------------------------------|
| a pickup of an item entity | IEN-PCK-002, IEN-PCK-005, IEN-PCK-006 |
| the `give` command | IEN-SPN-004 |
| the stack of `S` in row 9 of ITX-JAV-010 | a dropped stack of the request |
| the cursor stack and the craft grid at a menu close, a leave or a dimension change (INV-CLS-001, INV-CLS-003) | an item entity, IEN-SPN-001 |

**INV-INS-003** (`design`, semantic). At each step of INV-INS-001, the insert rule does not select
a slot whose place filter refuses the stack `T`. This applies to each caller of INV-INS-002:

- the pickup of an item entity;
- the `give` command;
- row 9 of ITX-JAV-010;
- the close of a menu, a leave and a dimension change (INV-CLS-001, INV-CLS-003).

The items that find no room go to the caller, as INV-INS-002 states.

| Before the insert | After the insert |
|-------------------|------------------|
| A plugin refuses each stack in `P[3]`, which holds 10 stone. `T` is 20 stone. | `P[3]` holds 10. The 20 stone go to the other main slots. |
| As the row above, and each other main slot holds dirt. | `P[3]` holds 10. The 20 stone find no room. |

Reason: a plugin can give each slot a place filter. A slot that refuses must not stop or capture
an insert while another slot has room. The vanilla server has no such slot, so the default
behaviour does not change. See deviation D7.

## 5. The creative inventory

The vanilla client does not send a click for its creative inventory screen. It computes the
result and sends one `set_creative_mode_slot` packet for each slot that changed.

**INV-CRE-001** (`design`, semantic). `set(X, stack)` is an operation that only the edge can add
to a request. It puts a given stack, or the empty stack, in the slot `X`. It reads no filter and
no slot limit. The stack that `X` held does not go to another slot. ITX-ATM-005 does not apply
to a request that has a `set`. A `set` applies when `X` exists; in the other case the rejection is
`no_such_slot`. Reason: issue 660 lists the creative set as an operation, and the clicks of
INV-CRE-005 make items. This rule answers question Q1 of `inventory-transactions.md`.

**INV-CRE-002** (`source_suggested_black_box_confirmed`, wire). The serverbound
`set_creative_mode_slot` packet has a short, the slot number, and a wire stack with length prefix
(STK-NET-007). STK-NET-009 applies to the stack. See experiment E6.

**INV-CRE-003** (`source_suggested`, semantic). The server does these checks on the packet, in
this order. The first check that fails ends the work on the packet. The server then changes no
slot, and INV-CRE-006 applies.

1. The player has infinite materials.
2. The feature flags of the world enable the item of the stack.
3. The stack is the empty stack, or its count is not larger than its maximum stack size.
4. The slot number is in the range `1..45`, or it is negative.

A negative slot number is a creative drop. IEN-SPN-007 states one more check for it, the
counter of the connection, after check 4. See experiment E6.

**INV-CRE-004** (`source_suggested`, semantic). A packet that passes the checks with a slot number
`i` in the range `1..45` gives the request `set(M[i], stack)`. `M[i]` is an entry of the
inventory menu, also when the player has another menu open. The client view of `M[i]` becomes
that stack. Thus the server sends no packet for the slot when the request applies. An armour slot
can hold a stack that its place filter refuses, with a count above 1. See experiment E6.

**INV-CRE-005** (`design`, semantic). Two clicks make items with the `set` operation:

- The clone click of ITX-JAV-012 gives `set(C, stack)`. The stack is a copy of the stack of `S`
  with the count `max(S)`.
- The drag of drag kind 2 of ITX-JAV-020 gives one `set(X, stack)` for each slot `X` that changes.
  One `set(C, stack)` follows, with the count that ITX-JAV-020 states.

The conditions and the counts are those of ITX-JAV-012 and ITX-JAV-020.

**INV-CRE-006** (`design`, client agreement). A packet that fails a check of INV-CRE-003 with a
slot number `i` in the range `0..45` is a claim of the client. The client view of `M[i]` becomes
the stack of the packet. The comparison of INV-SYN-003 then sends the stack that the server holds,
when the two differ. A packet with each other slot number changes no client view. Reason: a
refusal has the path of each other change (INV-SYN-004), and the client must not keep a stack
that the server does not hold. A denial of the request of INV-CRE-004 has the same result
(ITX-JAV-022). See deviation D3.

## 6. The state id and the client view

**INV-SYN-001** (`source_suggested`, wire). Each menu has a state id. It starts at 0 when the menu
starts to exist. Before the server sends a `container_set_content` packet or a
`container_set_slot` packet for the menu, it sets the state id to `(state id + 1) & 32767`. The
packet carries the new value. No other packet changes the state id. The `container_click` packet
carries the last state id that the client received for the menu. See experiment E7.

**INV-SYN-002** (`source_suggested`, client agreement). For each entry of the open menu and for
the cursor, the server keeps a client view. A client view is one of these:

- not known;
- a stack: the stack that the server sent last for the slot, or the stack of a
  `set_creative_mode_slot` packet for the slot (INV-CRE-004, INV-CRE-006);
- a hashed stack: the value that the last `container_click` packet gave for the slot.

The server holds a stack that matches a client view when one of these conditions is true:

- the client view is a stack, and the two stacks are equal (STK-EQ-002);
- the client view is a hashed stack, and it matches the stack of the server (STK-HSH-003).

A client view that is not known matches no stack. See experiment E7.

**INV-SYN-003** (`source_suggested`, client agreement). The server compares one time in each tick
of the player. It also compares after each `container_click` packet that passes the checks of
ITX-JAV-001 and has the state id of the server. Row 1 of ITX-JAV-004 states the other case.
For each entry of the open menu that does not match its client view, the server sends the stack
in a `container_set_slot` packet. When the cursor stack does not match its client view, the server
sends it in a `set_cursor_item` packet. The client view of each slot that the server sends becomes
the sent stack. ITX-JAV-004 states the same comparison after a click. See experiment E7.

**INV-SYN-004** (`design`, client agreement). Outside the cases of INV-SYN-005, a change of an
entry of the open menu or of the cursor reaches the client by the comparison of INV-SYN-003. This
does not depend on the cause of the change: a request, a pickup, a command, the use of an item or
another plugin. Reason: one path gives the
same result for each writer of a slot, and issue 409 decides it for a refused click.

**INV-SYN-005** (`source_suggested`, wire). A `container_set_content` packet has the container id,
the state id, the stack of each entry of the menu and the cursor stack. The entries are in the
order of the menu index. The server sends it for the inventory menu in each of these cases:

- when the player joins;
- when the server sends the full state of the player again, after a respawn or a change of
  dimension;
- for the open menu, in the cases of ITX-JAV-001, check 2, and ITX-JAV-004, row 1.

After the packet, the client view of each entry and of the cursor is the sent stack. The first
packet after the join has the state id 1 and 46 stacks. See experiment E7.

**INV-SYN-006** (`source_suggested_black_box_confirmed`, client agreement). The server compares
only the open menu. While another menu is open, a change of an entry of the inventory menu that
the open menu does not have sends no packet. Such entries are the armour slots, the off-hand slot
and the craft grid. The first comparison after that menu closes sends each such entry that
changed. The craft result is not in this rule. The specification of the craft states when the
server sends it (section 9). The trace `tools/mcrs_minecraft_probe/traces/inventory_e08.json`
confirms the rule:

- While a chest is open, `item replace` puts a helmet in `P[39]`, a shield in `P[40]` and dirt in
  the slot 0 of the craft grid. The server sends no packet for these three slots.
- The close of the chest sends `M[1]`, `M[5]` and `M[45]` with the container id 0.

In that trace, the vanilla server sends `M[0]` two times with the container id 0 while the chest
is open, after the change of the craft grid. These two packets increase the state id of the
inventory menu.

**INV-SYN-007** (`source_suggested_black_box_confirmed`, client agreement). When a menu closes, the
inventory menu takes the client view of each slot that the two menus share. Thus the server does
not send the main slots again after a close. The state id of the inventory menu does not change at
a close.

The trace `tools/mcrs_minecraft_probe/traces/inventory_e08.json` confirms the client views of the
slots:

- While the chest is open, a change of `P[0]` goes in a `container_set_slot` packet with the
  container id of the chest. After the close, the server does not send `M[36]` again.
- The first packet with the container id 0 after the close has the state id that follows the last
  such packet before the close.
- A click with that state id and a correct prediction gets no packet back.

**INV-SYN-009** (`source_suggested`, client agreement). At the close of a menu other than the
inventory menu, in each of the two directions, the client view of the cursor becomes the empty
stack. Thus the comparison sends no `set_cursor_item` for the cursor that step 1 of INV-CLS-001
empties. At the close of the inventory menu, the client view of the cursor does not change.

The trace `tools/mcrs_minecraft_probe/traces/inventory_e09.json` shows the packets. It does not
show the cursor that the vanilla client displays after the close, and in each case the cursor was
empty when the chest opened. A recording of the vanilla client must confirm the rule.

- A chest is open, and the cursor holds a stack. After a serverbound `container_close` and after
  a clientbound `container_close`, the server sends the slot that the stack enters and no
  `set_cursor_item`.
- The inventory menu is open, and the cursor holds a stack. After a serverbound
  `container_close`, the server sends `set_cursor_item` with the empty stack.

**INV-SYN-008** (`design`, unspecified). The server can also send a `set_player_inventory` packet
for a slot of the player inventory. The packet has a VarInt, the index of INV-SLT-001, and a wire
stack. It carries no state id. A parity test must not compare if a change arrives in this packet
or in a `container_set_slot` packet. Reason: the vanilla client shows the same stack for each of
the two.

## 7. The close of a menu, the leave and the dimension change

**INV-CLS-001** (`source_suggested_black_box_confirmed`, semantic). The serverbound
`container_close` packet closes the
open menu. The server does not compare the container id of the packet. The server then does these
steps:

1. The cursor stack enters the player inventory by the insert rule (INV-INS-001). The items that
   find no room become one item entity (IEN-SPN-001). The cursor then holds the empty stack.
2. When the open menu is the inventory menu, each stack of the craft grid does step 1 also. The
   order is from the slot 0 to the slot 3. The craft result then holds the empty stack.
3. The open menu becomes the inventory menu.

The inventory menu continues to exist after the packet. IEN-SPN-006 states the same result for the
cursor stack. A plugin cannot deny the moves of steps 1 and 2 (ITX-ATM-012). INV-CLS-004 and
INV-CLS-005 state the slots that these moves fill and the filters that they read.

The trace `tools/mcrs_minecraft_probe/traces/inventory_e09.json` confirms the rule. `P[0]` is the
selected slot in each case. A slot that a row does not name holds the empty stack.

| Before the close | After the close |
|------------------|-----------------|
| `P[7]` holds 5 stone. The cursor holds 10 stone. | `P[7]` holds 15. |
| Each main slot holds 64 dirt. The cursor holds 10 stone. | One item entity of 10 stone. |
| `P[7]` holds 5 stone. The cursor holds 4 stone. The slot 0 of the craft grid holds 3 stone, and the slot 2 holds 2 dirt. | `P[7]` holds 12, and `P[0]` holds 2 dirt. |
| Each main slot holds 64 dirt. The cursor holds 4 stone. The slot 0 of the craft grid holds 3 andesite, and the slot 3 holds 1 granite. | Three item entities: 4 stone, 3 andesite and 1 granite. |
| The cursor holds 2 stone. The packet has the container id 7, and the open menu is the inventory menu. | `P[0]` holds 2 stone. |
| A chest with the container id 2 is open. The cursor holds 3 stone. The packet has the container id 9. | `P[0]` holds 3 stone. The next change of `P[1]` goes in a packet with the container id 0. |

The cursor and the craft grid hold the empty stack after each case. The trace
`tools/mcrs_minecraft_probe/traces/inventory_e08.json` confirms the condition of step 2: the
slot 0 of the craft grid holds dirt while a chest is open. After the close of the chest, the server
sends `M[1]` with the dirt. Two item entities of one item
can merge in a later tick (IEN-MRG-001), at a tick that a parity test must not compare.

**INV-CLS-002** (`source_suggested_black_box_confirmed`, semantic). The server sends no packet as an
answer to `container_close`. The comparison of INV-SYN-003 sends the slots that step 1 and step 2
changed.
When the server closes a menu itself, it sends a clientbound `container_close` packet with the
container id of that menu. It then does the steps of INV-CLS-001. The server never closes the
inventory menu. INV-SYN-009 states the client view of the cursor after a close.

The traces `tools/mcrs_minecraft_probe/traces/inventory_e09.json` and
`tools/mcrs_minecraft_probe/traces/inventory_e11.json` confirm it:

- After each serverbound `container_close`, the server sends only the slots and the cursor that
  changed.
- A chest is open, and the cursor holds 6 stone. A `setblock` command removes the chest. The
  server sends `container_close` with the container id of the chest. After it, the server sends
  `M[36]` with 6 stone.

**INV-CLS-003** (`design`, semantic). The server does the steps of INV-CLS-001 at these events:

- the player leaves the server;
- the player does a dimension change.

It does them for the open menu and then for the inventory menu. Thus the cursor stack and the stacks
of the craft grid enter the player inventory.
Only the items that find no room become item entities. These item entities are in the dimension
that the player leaves.

At a leave, the server does the steps before it writes the player file. Thus the player file holds
these stacks in the player inventory. At a dimension change, the server does the steps before the
player enters the new dimension. When the open menu is not the inventory menu, the server sends
`container_close` as INV-CLS-002 states.

At a cell crossing, the server does not do these steps, and the open menu stays open. The cursor
stack and the stacks of the craft grid move with the player to the new cell.

Reason: issue 521 decides that a dimension change closes the menus as a leave does. Thus the
stacks stay with the player. A cell crossing changes nothing, because the player does not see it.
See deviations D4 and D6.

**INV-CLS-004** (`source_suggested_black_box_confirmed`, semantic). The moves of a close fill only
the slots that the insert rule selects, in the order of INV-INS-001. Thus a close can fill only
these slots:

1. the selected slot, when it holds an alike stack;
2. the off-hand slot, when it holds an alike stack;
3. a main slot.

A close never fills an armour slot, an off-hand slot that holds the empty stack, a slot of the craft
grid or the craft result. The insert of a close skips each slot whose place filter refuses the
stack (INV-INS-003). On the vanilla server, no main slot and no off-hand slot has a place filter.
Thus no destination of a close refuses a stack there.

The trace `tools/mcrs_minecraft_probe/traces/inventory_e11.json` confirms the rule. Each main slot
holds 64 dirt before the first case, and the armour slots hold the empty stack in each case.

| Before the close | After the close |
|------------------|-----------------|
| `P[40]` holds the empty stack. The cursor holds an iron helmet. | `P[39]` and `P[40]` hold the empty stack. One item entity holds the helmet. |
| `P[40]` holds the empty stack. The slot 0 of the craft grid holds an iron helmet. | As the row above. |
| `P[40]` holds 10 stone. The cursor holds 60 stone. | `P[40]` holds 64. One item entity holds 6 stone. |
| `P[0]`, `P[5]` and `P[40]` hold 10 stone. `P[5]` is the selected slot. The cursor holds 60 stone. | `P[5]` holds 64, `P[40]` holds 16 and `P[0]` holds 10. No item entity. |

**INV-CLS-005** (`design`, semantic). The moves of a close read no take filter. Thus each stack of
the cursor and of the craft grid leaves its slot. The items that find no room become item entities,
as step 1 of INV-CLS-001 states. Thus the moves of a close never have a rejection of ITX-REJ-001,
and a close never loses a stack.

| Before the close | After the close |
|------------------|-----------------|
| A plugin refuses each take from the slot 0 of the craft grid. That slot holds 20 stone. | That slot holds the empty stack. The 20 stone are in the player inventory. |

Reason: the player file does not hold the cursor and the craft grid (INV-SAV-001). A stack that
stays there at a leave is lost.

## 8. The player file

**INV-SAV-001** (`source_suggested_black_box_confirmed`, semantic). The player file holds the
player inventory and the selected slot. It does not hold the cursor stack, the craft grid or the
craft result. `persistence.md`, section "Player keys", states the fields.

| Slots | Field |
|-------|-------|
| `P[0]` through `P[35]` | an entry of `Inventory` with the same number in `Slot` |
| `P[36]`, `P[37]`, `P[38]`, `P[39]` | `equipment.feet`, `equipment.legs`, `equipment.chest`, `equipment.head` |
| `P[40]` | `equipment.offhand` |
| the selected slot | `SelectedItemSlot` |

A slot that holds the empty stack has no entry and no key. See experiment E10.

**INV-SAV-002** (`design`, semantic). A load does not read a filter or a slot limit. A slot holds
the stack that the file gives for it. A `SelectedItemSlot` outside the range `0..8` loads as 0.
Reason: a file that an earlier run wrote must load, and the reference gives an internal error for
such a selected slot.

**INV-SAV-003** (`source_suggested`, semantic). A player that joins again has the stacks and the
selected slot that the file holds. The server sends them as INV-SEL-003 and INV-SYN-005 state. See
experiment E10.

## 9. What this document does not state

A later specification states each of these items.

- The menu of each block and of each entity: its entries, its destination lists, and the packet
  that opens it. That specification also owns the game rule `mcrs:spectators_open_menus` of issue
  521.
- The craft: the content of the craft result, a take from it, and the quick move from `M[0]`
  beyond INV-MNU-004. It also states the repeat of a quick move and of a throw on a slot that
  fills again. No slot of the inventory menu fills again without the craft. It also states when
  the server sends the slot of the craft result (INV-SYN-006).
- The items that take a click for themselves, for example a bundle (ITX-JAV-008).
- The pick of a block or of an entity with the pick key. It changes the selected slot and sends
  `set_held_slot` (INV-SEL-003).
- What other players see of the hands and the armour: the `set_equipment` packet.
- The effects of a stack in an armour slot or in a hand: attributes, sounds and the game events.
- The drop of the stacks at the death of a player, and the cursor stack at that time.
- The `clear` command and the `item` command. INV-SLT-002 gives the slots that "each slot of the
  player" names. Issue 521 decides that `clear` empties the cursor slot also.
- The use of an item, and the decrease of a stack when a player places a block (PLC-INV-006).
- The stack requests of Bedrock Edition for the off hand, the armour and the cursor. Their
  container ids map to the slot references of INV-SLT-001 and INV-SLT-002 at the edge.

## 10. Intentional deviations

**D1.**

- **Rule**: INV-SAV-002.
- **Reference**: a player file with a `SelectedItemSlot` outside `0..8` gives an internal error
  when the player joins.
- **Visible to the client**: no. The game does not write such a file.
- **Test**: a differential test must not use such a file.

**D2.**

- **Rule**: INV-SYN-008.
- **Reference**: after a menu close, the reference sends one `set_player_inventory` packet for
  each move into a slot, with the stack after that move. Then it sends one `container_set_slot`
  packet for each such slot, with the last stack. The
  traces `tools/mcrs_minecraft_probe/traces/inventory_e09.json` and
  `tools/mcrs_minecraft_probe/traces/inventory_e11.json` show it, also for the stacks of the craft
  grid.
- **Visible to the client**: no. The last packet of each slot carries the same stack.
- **Test**: a differential test compares the stacks that the client holds, not the packets.

**D3.**

- **Rule**: INV-CRE-006.
- **Reference**: the reference sends nothing for a `set_creative_mode_slot` packet that fails a
  check. The client then shows a stack that the server does not hold.
- **Visible to the client**: yes. The client gets the stack that the server holds for the slot.
  A vanilla client meets this when the server changes its game mode while the packet travels.
- **Test**: a differential test must not compare the clientbound packets after such a packet.

**D4.**

- **Rule**: INV-CLS-003.
- **Reference**: when a player leaves, the cursor stack and the stacks of the craft grid become
  item entities at the player. They do not enter the player inventory.
- **Visible to the client**: yes. Other players see no item entity at the place of the leave. The
  player has the stacks in the player inventory after the next join.
- **Test**: a differential test must not compare the item entities or the player file after a
  leave with a stack in the cursor or in the craft grid.

**D5.**

- **Rule**: INV-SEL-005.
- **Reference**: the hand swap of the reference reads no filter and no slot limit.
- **Visible to the client**: only when a plugin gives `P[n]` or `P[40]` a filter. The reference
  has no such plugin.
- **Test**: a differential test uses no plugin filter.

**D6.**

- **Rule**: INV-CLS-003.
- **Reference**: a dimension change does not close a menu on the server. The cursor stack and the
  stacks of the craft grid stay where they are. The vanilla client sends `container_close` in two
  cases:
  - it shows a menu screen while the player is in a portal. The close then occurs before the
    dimension change.
  - it gets the `respawn` packet while a menu other than the inventory menu is open. The items
    that find no room then become item entities in the new dimension.

  In the other cases, the cursor stack stays on the server until the next `container_close`. An
  example is a `teleport` command while the inventory screen is open.
- **Visible to the client**: yes. After a dimension change, the cursor stack and the stacks of the
  craft grid are in the player inventory. The items that find no room are in the old dimension.
- **Test**: a differential test must not compare the slots, the cursor or the item entities after
  a dimension change with a stack in the cursor or in the craft grid.

**D7.**

- **Rule**: INV-INS-003.
- **Reference**: the reference has no place filter on a main slot or on the off-hand slot. Thus no
  insert of the reference skips a slot.
- **Visible to the client**: only when a plugin gives a main slot or the off-hand slot a place
  filter. The reference has no such plugin.
- **Test**: a differential test uses no plugin filter.

## 11. Unresolved

Not decided by an experiment yet:

- **U2**: INV-SYN-003. The place of the comparison in the tick of the player, relative to the
  pickup and to the packets of other domains.

For the owner:

- **U4**: INV-MNU-004, rows 3 and 4. A quick move of a helmet with `allowed_entities` of
  `minecraft:zombie` from `M[9]` moves no item, also when `M[36]` holds the empty stack. The
  trace `tools/mcrs_minecraft_probe/traces/inventory_e03.json` shows it. A player can expect the
  helmet to go to the hotbar by row 5. Is this result a quirk that mcrs keeps, or a deviation? Until
  the owner decides, the rule
  states the vanilla result.

## 12. Experiments

Each experiment uses the game version of the table at the top of this document. Each one is of
the kind "server" of `inventory-transactions.md`, section 11: a vanilla server with
`online-mode=false` and one scripted client that is an operator. The scripted client records each
clientbound packet of the names that this document uses. `tools/mcrs_minecraft_probe` runs such an
experiment from a scenario file and writes its trace. The commands `item replace entity @s
player.cursor` and `item replace entity @s player.crafting.<n>` fill the cursor and the craft grid.

| Experiment | Steps | Record |
|------------|-------|--------|
| E1 | Run `item replace entity @s` with a different item for `hotbar.0`, `inventory.0`, `armor.feet`, `armor.head` and `weapon.offhand`. Join again. | The index of each stack in `container_set_content`; the `Slot` values and the `equipment` keys of the player file. |
| E2 | In survival mode, click an iron helmet into each armour slot. Click a helmet with `allowed_entities` of `minecraft:zombie` into the head slot. Click a stack of 2 helmets into the head slot. Put a helmet with the curse of binding in the head slot and click it. Repeat the last click in creative mode. | The slot and the cursor after each click. |
| E3 | In the inventory menu, click `M[0]` with stone in the cursor. Put 40 stone in `M[10]` and `M[40]`. Then send a quick move on: 4 oak planks in `M[0]`; a helmet in `M[9]` with an empty head slot, then with a full head slot; a shield in `M[36]`; 64 stone in `M[9]`, in `M[36]`, in `M[1]`, in `M[45]`; a helmet in `M[5]`; a helmet with `allowed_entities` of `minecraft:zombie` in `M[9]`. Send pick up all with oak planks in `M[0]` and `M[9]`. Send a drag with 8 stone over `M[0]`, `M[2]`, `M[5]`, `M[11]` and `M[45]`. Put a helmet with the curse of binding in `M[5]` and send a quick move on it. Send a quick move on an empty slot. Scenario: `tools/mcrs_minecraft_probe/scenarios/inventory_e03.json`. | Each slot after each click. Trace: `tools/mcrs_minecraft_probe/traces/inventory_e03.json`. |
| E4 | Send `set_carried_item` with 3, then with 9, then with -1. Start to eat, then send `set_carried_item` with 4. | The clientbound packets; the main hand that a second client sees; if the connection stays. |
| E5 | With 10 stone in the selected slot, send `player_action` with the status 5, then 4. Send the status 7 with a helmet in the selected slot and a shield in the off hand, then with two empty hands. Repeat each as a spectator. | The slots, the item entities and the clientbound packets. |
| E6 | Send `set_creative_mode_slot` in creative mode: stone to the slots 0, 1, 5, 36, 45 and 46; 65 stone to the slot 36; the empty stack to the slot 36; an item of a feature flag that is off. Repeat the first packet in survival mode, and with a chest open. | The slots of the server after each packet, and the clientbound packets. |
| E7 | Join. Record the first `container_set_content`. Run `give` 3 times. Send a valid click with the state id of the last packet, and one with that state id minus 1. Cause 32768 slot updates. | The state id of each packet; the packets after each click; the state id after the wrap. |
| E8 | Open a chest. Run `item replace entity @s` for `armor.head`, `weapon.offhand`, `player.crafting.0` and `hotbar.0`. Close the chest. Send a click with a correct prediction on `M[36]`. Repeat the click with no prediction. Scenario: `tools/mcrs_minecraft_probe/scenarios/inventory_e08.json`. | The packets while the chest is open and after the close, with their container ids and state ids. Trace: `tools/mcrs_minecraft_probe/traces/inventory_e08.json`. |
| E9 | Hold 10 stone in the cursor of the inventory menu and send `container_close`. Repeat with a full player inventory. Repeat with stone and dirt in the craft grid, and with three items in the cursor and the craft grid and a full player inventory. Send `container_close` with the container id 7. Remove an open chest with `setblock` while the cursor holds stone. Open a chest, hold stone in the cursor and send `container_close` with the container id 9. Scenario: `tools/mcrs_minecraft_probe/scenarios/inventory_e09.json`. | The slots and the item entities after each case. Trace: `tools/mcrs_minecraft_probe/traces/inventory_e09.json`. |
| E10 | Fill each slot of the player inventory, select the slot 5, hold a stack in the cursor, and stop the server. Start it and join. | The player file, and the first `container_set_content` and `set_held_slot` after the join. |
| E11 | Put 64 dirt in each main slot. Keep the armour slots and `P[40]` empty. Hold an iron helmet in the cursor and send `container_close`. Repeat with the helmet in the slot 0 of the craft grid. Put 10 stone in `P[40]`, hold 60 stone and close. Put 10 stone in `P[0]`, `P[5]` and `P[40]`, select `P[5]`, hold 60 stone and close. Scenario: `tools/mcrs_minecraft_probe/scenarios/inventory_e11.json`. | The slots of the player inventory and the item entities after each close. Trace: `tools/mcrs_minecraft_probe/traces/inventory_e11.json`. |
