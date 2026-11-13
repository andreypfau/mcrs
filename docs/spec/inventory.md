# Player inventory: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| World version | `5127` |

Source of these values: `assets/minecraft/version.json`.

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
  **drag**: as `inventory-transactions.md`, section 0.1, defines them.
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

### 0.2 Provenance

The labels are those of `inventory-transactions.md`, section 0.2. A rule states its kind after the
label (`README.md`, section "Semantics, not accidents").

No experiment of section 12 has a recorded run. Each one is a proposal.

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
| `INV-CLS` | the cursor stack and the craft grid when a menu closes or the player leaves |
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
| 40 | off hand | 99 | INV-SLT-005 |

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

**INV-SLT-005** (`design`, semantic). The off-hand slot has no take filter. Its place filter
reads the game rule `mcrs:offhand_accepts_any_item`. The default value of the rule is true. When
the rule is true, the place filter permits each stack. When the rule is false, the place filter
permits only a stack whose item is in the item tag `mcrs:offhand_allowed`. A change of the rule
does not move a stack that the slot holds. Reason: issue 521 keeps the behaviour of Java Edition
as the default, and a tag lets a data pack change the set (section 11, U1).

**INV-SLT-006** (`source_suggested`, semantic). The place filter of the slot of the craft result
refuses each stack. Its take filter and its content belong to the specification of the craft
(section 9). Until that specification exists, the slot holds the empty stack. See experiment E3.

**INV-SLT-007** (`design`, semantic). The filters of this section apply to a request. They do not
apply to the `set` operation (INV-CRE-001) or to a command. Reason: a command and the creative
inventory must be able to fill each slot.

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

**INV-MNU-002** (`source_suggested`, semantic). The inventory menu exists from the join of the
player until the player leaves. No packet opens it. The open menu is the inventory menu when the
player has no other menu open. The inventory menu is always valid for the player (ITX-JAV-001,
check 3). See experiment E7.

**INV-MNU-003** (`source_suggested`, semantic). The button of the swap click names a slot of the
player inventory (ITX-JAV-010). The buttons `0..8` name `P[0]` through `P[8]`. The button 40 names
`P[40]`. This rule applies to each menu. See experiment E10 of `inventory-transactions.md`.

**INV-MNU-004** (`source_suggested`, semantic). A quick move in the inventory menu has the
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
refuses, the click moves no item. Deviation D4 of `inventory-transactions.md` does not show in
this menu. See experiment E3.

**INV-MNU-005** (`source_suggested`, semantic). In the inventory menu, pick up all cannot take from
`M[0]` (ITX-JAV-011). The inventory menu permits a drag to each entry (ITX-JAV-017). See
experiments E11 and E14 of `inventory-transactions.md`.

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
stack that the server holds. See experiment E5.

**INV-SEL-005** (`design`, semantic). The `player_action` packet with the status 7 gives the
request `swap(P[n], P[40])`, where `n` is the selected slot. When the two slots hold the empty
stack, the server does nothing. When the request applies and the player uses an item, that use
stops. The server ignores the packet of a spectator. Reason: a request is the only path that
changes a slot, so the filters of the two slots apply with no other rule. See experiment E5.

**INV-SEL-006** (`design`, client agreement). When the request of INV-SEL-005 has a rejection,
the client view of `P[n]` and of `P[40]` becomes not known. The comparison of INV-SYN-003 then
sends the two stacks. Reason: the client showed the exchange before the answer came
(ITX-ATM-007). The place filter of INV-SLT-005 is the only cause of such a rejection.

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
| the cursor stack and the craft grid at a menu close or at a leave (INV-CLS-001, INV-CLS-003) | an item entity, IEN-SPN-001 |

**INV-INS-003** (`design`, semantic). The insert rule reads no place filter. This includes the
place filter of the off-hand slot (INV-SLT-005). Reason: the insert adds only to a stack that the
slot holds now, and the main slots have no filter.

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
that the server does not hold. See deviation D3.

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

**INV-SYN-006** (`source_suggested`, client agreement). The server compares only the open menu.
While another menu is open, a change of an entry of the inventory menu that the open menu does
not have sends no packet. Such entries are the armour slots, the off-hand slot and the craft grid.
The first comparison after that menu closes sends each such entry that changed. See experiment E8.

**INV-SYN-007** (`source_suggested`, client agreement). When a menu closes, the inventory menu
takes the client view of each slot that the two menus share. Thus the server does not send the
main slots again after a close. The state id of the inventory menu does not change at a close.
See experiment E8.

**INV-SYN-008** (`design`, unspecified). The server can also send a `set_player_inventory` packet
for a slot of the player inventory. The packet has a VarInt, the index of INV-SLT-001, and a wire
stack. It carries no state id. A parity test must not compare if a change arrives in this packet
or in a `container_set_slot` packet. Reason: the vanilla client shows the same stack for each of
the two.

## 7. The close of a menu, and the leave

**INV-CLS-001** (`source_suggested`, semantic). The serverbound `container_close` packet closes the
open menu. The server does not compare the container id of the packet. The server then does these
steps:

1. The cursor stack enters the player inventory by the insert rule (INV-INS-001). The items that
   find no room become one item entity (IEN-SPN-001). The cursor then holds the empty stack.
2. When the open menu is the inventory menu, each stack of the craft grid does step 1 also. The
   order is from the slot 0 to the slot 3. The craft result then holds the empty stack.
3. The open menu becomes the inventory menu.

The inventory menu continues to exist after the packet. IEN-SPN-006 states the same result for the
cursor stack. See experiment E9.

**INV-CLS-002** (`source_suggested`, semantic). The server sends no packet as an answer to
`container_close`. The comparison of INV-SYN-003 sends the slots that step 1 and step 2 changed.
See experiment E9.
When the server closes a menu itself, it sends a clientbound `container_close` packet with the
container id of that menu. It then does the steps of INV-CLS-001. The server never closes the
inventory menu.

**INV-CLS-003** (`design`, semantic). When a player leaves the server, the server does the steps
of INV-CLS-001 for the open menu and then for the inventory menu. It does them before it writes
the player file. Thus the cursor stack and the stacks of the craft grid are in the player
inventory of the file. Only the items that find no room become item entities. Reason: a leave
then has the path of a menu close, and the stacks stay with the player. See deviation D4.

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
  fills again. No slot of the inventory menu fills again without the craft.
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
- **Reference**: after a menu close, the reference sends each slot that the cursor stack entered
  two times: in a `set_player_inventory` packet and then in a `container_set_slot` packet.
- **Visible to the client**: no. The two packets carry the same stack.
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
- **Visible to the client**: only when `mcrs:offhand_accepts_any_item` is false. The reference
  has no such rule.
- **Test**: a differential test uses the default value of the rule.

## 11. Unresolved

For the owner:

- **U1**: INV-SLT-005. The entries of the item tag `mcrs:offhand_allowed`. They must be the items
  that Bedrock Edition permits in the off hand. No artefact of this repository lists them. This
  point does not stop the implementation for Java Edition: with the default value of the rule,
  the server does not read the tag. The work on the Bedrock edge gives the list.

Not decided by an experiment yet:

- **U2**: INV-SYN-003. The place of the comparison in the tick of the player, relative to the
  pickup and to the packets of other domains.

## 12. Experiments

Each experiment uses the game version of the table at the top of this document. Each one is of
the kind "server" of `inventory-transactions.md`, section 11: a vanilla server with
`online-mode=false` and one scripted client that is an operator. The scripted client records each
clientbound packet of the names that this document uses.

| Experiment | Steps | Record |
|------------|-------|--------|
| E1 | Run `item replace entity @s` with a different item for `hotbar.0`, `inventory.0`, `armor.feet`, `armor.head` and `weapon.offhand`. Join again. | The index of each stack in `container_set_content`; the `Slot` values and the `equipment` keys of the player file. |
| E2 | In survival mode, click an iron helmet into each armour slot. Click a helmet with `allowed_entities` of `minecraft:zombie` into the head slot. Click a stack of 2 helmets into the head slot. Put a helmet with the curse of binding in the head slot and click it. Repeat the last click in creative mode. | The slot and the cursor after each click. |
| E3 | In the inventory menu, click `M[0]` with stone in the cursor. Send a quick move on: a helmet in `M[9]` with an empty head slot, then with a full head slot; a shield in `M[36]`; 64 stone in `M[9]`, in `M[36]`, in `M[1]`, in `M[45]`; a helmet in `M[5]`. Put 40 stone in `M[10]` and `M[40]` first. | Each slot after each click. |
| E4 | Send `set_carried_item` with 3, then with 9, then with -1. Start to eat, then send `set_carried_item` with 4. | The clientbound packets; the main hand that a second client sees; if the connection stays. |
| E5 | With 10 stone in the selected slot, send `player_action` with the status 5, then 4. Send the status 7 with a helmet in the selected slot and a shield in the off hand, then with two empty hands. Repeat each as a spectator. | The slots, the item entities and the clientbound packets. |
| E6 | Send `set_creative_mode_slot` in creative mode: stone to the slots 0, 1, 5, 36, 45 and 46; 65 stone to the slot 36; the empty stack to the slot 36; an item of a feature flag that is off. Repeat the first packet in survival mode, and with a chest open. | The slots of the server after each packet, and the clientbound packets. |
| E7 | Join. Record the first `container_set_content`. Run `give` 3 times. Send a valid click with the state id of the last packet, and one with that state id minus 1. Cause 32768 slot updates. | The state id of each packet; the packets after each click; the state id after the wrap. |
| E8 | Open a chest. Run `item replace entity @s armor.head` and `item replace entity @s hotbar.0`. Close the chest. | The packets while the chest is open and after the close, with their container ids and state ids. |
| E9 | Hold 10 stone in the cursor of the inventory menu and send `container_close`. Repeat with a full player inventory. Repeat with stone in the craft grid. | The slots and the item entities after each case. |
| E10 | Fill each slot of the player inventory, select the slot 5, hold a stack in the cursor, and stop the server. Start it and join. | The player file, and the first `container_set_content` and `set_held_slot` after the join. |
