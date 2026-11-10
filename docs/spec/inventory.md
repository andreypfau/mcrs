# Player inventory: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. All captured results of section 10 come
from this version.

This document states the inventory of a player: its slots, the selected hotbar slot, the stacks in
the slots, and what the server sends when a stack changes. It states only what a client, the server
console and the server files show. It does not prescribe an architecture. All rules come from a
vanilla server in offline mode and scripted clients, or from the decisions in issue 521. No rule
has the label `source_suggested`.

## Parts of this document

Issue 521 is too large for one change. This document grows in four parts, in this order:

1. Slots, stacks, the selected slot, the writes of the server, and `set_creative_mode_slot`
   (sections 1 to 5, prefixes `INV-SLOT`, `INV-STK`, `INV-SEL`, `INV-SET`, `INV-CRE`). This part is
   written.
2. The transaction model for all editions: the atomic operations, the rejection, the state id and
   the resync, and the map of the Bedrock slots (prefixes `INV-OP`, `INV-REJ`, `INV-STATE`).
3. The click modes of Java Edition and the carried stack, as sequences of the operations of part 2
   (prefix `INV-CLK`).
4. The hand actions (drop, swap of the hands, pick block), the `clear` command, and the storage in
   the player file (prefixes `INV-HND`, `INV-STO`).

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**, **VarInt**,
  **offline mode**: as `login.md`, section 0.1, defines them.
- **Owner**: the player whose inventory a rule states.
- **Observer**: another player whose client tracks the owner as an entity.
- **Stack**: an item, a count and a component patch (INV-STK-001).
- **Empty stack**: the stack that holds nothing. A slot with the empty stack is an empty slot.
- **Slot**: a place that holds one stack.
- **Main slot**: one of the 36 slots with the indices 0 to 35 (INV-SLOT-001).
- **Hotbar slot**: a main slot with an index from 0 to 8.
- **Armour slot**: one of the slots `feet`, `legs`, `chest` and `head`.
- **Off-hand slot**: the slot of the off hand.
- **Carried slot**: the slot of the stack on the mouse cursor of the owner (INV-SLOT-002).
- **Selected slot**: the hotbar slot that the owner holds in the main hand (INV-SEL-001).
- **Main-hand item**: the stack in the selected slot.
- **Inventory menu**: the menu with the window id 0. The owner has it at all times.
- **Open menu**: a menu with a window id other than 0, for example the menu of a chest.
- **Menu index**: the number of a slot in a menu, as the packets send it.
- **Craft grid**: the four craft input slots of the inventory menu.
- **Craft result**: the output slot of the inventory menu.
- **Equal**: two stacks are equal when INV-STK-005 states so.
- **Write**: the server puts a stack into a slot, for example for a command.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 10.
- `documentation`: publicly documented behaviour, for example a report on the public bug tracker.
- `design`: an mcrs choice. The rule states the reason.
- `unresolved`: not decided. Section 8 collects these items.

All the results come from one vanilla server and scripted clients on one machine. A rule that rests
on one observation states this.

### 0.3 ID prefix

Each rule has an ID with the prefix `INV`.

## 1. Slots

**INV-SLOT-001** (`black_box_confirmed`, semantic). The inventory of a player has these slots:

- 36 main slots with the indices 0 to 35. The indices 0 to 8 are the hotbar slots.
- 4 armour slots: `feet`, `legs`, `chest` and `head`.
- 1 off-hand slot.
- 1 carried slot (INV-SLOT-002).

A player has no slot for `armor.body` and no slot for `saddle`. A command that writes to them fails
with the message `No targets accepted item 1 minecraft:stone into the specified slots`. See results
R2 and R3.

**INV-SLOT-002** (`design`, semantic). The carried slot is a slot of the inventory of the player,
the same as a main slot. An operation that empties all slots of a player also empties the carried
slot. Reason: issue 521 makes the carried stack a slot, as Bedrock Edition does, so that one model
serves both editions. Entry D1 records the reference behaviour of the `clear` command.

**INV-SLOT-003** (`black_box_confirmed`, wire). The inventory menu has 46 slots. The menu index of
each slot is:

| Menu index | Slot |
|------------|------|
| 0 | craft result |
| 1 to 4 | craft grid |
| 5 | `head` |
| 6 | `chest` |
| 7 | `legs` |
| 8 | `feet` |
| 9 to 35 | main slots 9 to 35 |
| 36 to 44 | hotbar slots 0 to 8 |
| 45 | off-hand slot |

The carried slot has no menu index. See results R2 and R3. No result shows the index 7. The order
of the armour slots in the table comes from the indices 5, 6 and 8, which the results show.

**INV-SLOT-004** (`black_box_confirmed`, semantic). The slot names of the `item` command select
these slots of a player:

| Slot name | Slot |
|-----------|------|
| `hotbar.0` to `hotbar.8` | main slots 0 to 8 |
| `inventory.0` to `inventory.26` | main slots 9 to 35 |
| `container.0` to `container.35` | main slots 0 to 35 |
| `armor.feet`, `armor.legs`, `armor.chest`, `armor.head` | the armour slots |
| `weapon.mainhand` | the selected slot |
| `weapon.offhand` | the off-hand slot |
| `player.cursor` | the carried slot |
| `player.crafting.0` to `player.crafting.3` | the craft grid, menu indices 1 to 4 |

See results R2 and R10.

**INV-SLOT-005** (wire). The slot ids of the `set_equipment` packet are:

| Id | Slot |
|----|------|
| 0 | main hand (the selected slot) |
| 1 | off-hand slot |
| 2 | `feet` |
| 3 | `legs` |
| 4 | `chest` |
| 5 | `head` |
| 6 | body |
| 7 | saddle |

Provenance: `black_box_confirmed` for the ids 0, 1, 2, 4 and 5 (results R2 and R3). `documentation`
for the ids 3, 6 and 7. A player never sends the ids 6 and 7, because it has no such slots.

**INV-SLOT-006** (`black_box_confirmed`, semantic). The player file keeps the slots in these fields.
Part 4 of this document states the full storage.

- `Inventory`: a list with one entry for each main slot that is not empty. An entry has the fields
  `Slot` (byte, the index of the main slot), `id`, `count` and, when the patch is not empty,
  `components`.
- `equipment`: a compound with the keys `head`, `chest`, `legs`, `feet` and `offhand`. A key is
  absent when its slot is empty.
- `SelectedItemSlot`: the selected slot, an int.

See results R4 and R12.

## 2. Stacks

**INV-STK-001** (`black_box_confirmed`, semantic). A stack has an item, a count, and a patch of data
components. The patch adds components to the default components of the item, or removes them. The
empty stack has no item and the count 0. See results R4 and R11.

**INV-STK-002** (`black_box_confirmed`, semantic). The count of a stack that is not empty must be
from 1 to the maximum stack size of the stack. The maximum stack size is the value of the
`minecraft:max_stack_size` component of the stack, after the patch. See results R5 and R7.

| Item | Maximum stack size |
|------|--------------------|
| `stone` | 64 |
| `ender_pearl` | 16 |
| `diamond_sword` | 1 |
| `stone` with `minecraft:max_stack_size` 99 in the patch | 99 |

A write by the `item` command with a larger count fails. The reply is `minecraft:stone can only
stack up to 64`, with the item id and the maximum. A count above 99 in the command fails as a
syntax error: `Integer must not be more than 99: found 100`. See result R5.

**INV-STK-003** (`black_box_confirmed`, wire). A clientbound packet encodes a stack in this order:

1. count (VarInt). The value 0 is the empty stack, and nothing follows.
2. item id (VarInt), the protocol id of the item in the registry `minecraft:item`.
3. the number of added components (VarInt).
4. the number of removed components (VarInt).
5. each added component: the id of its type (VarInt), then its value.
6. each removed component: the id of its type (VarInt).

The value of an added component has no length before it. See result R11.

**INV-STK-004** (`black_box_confirmed`, wire). The stack in `set_creative_mode_slot` has the
form of INV-STK-003, with one difference: a VarInt with the length of the value in bytes comes
before the value of each added component. The server must close the connection when the stack does
not decode. These stacks do not decode:

- a count above 99;
- a component value outside its range, for example `minecraft:max_stack_size` 100.

The disconnect reason contains `Failed to decode packet
'serverbound/minecraft:set_creative_mode_slot'`. A count of 0 or less decodes as the empty stack.
See results R7, R8 and R9.

**INV-STK-005** (`black_box_confirmed`, semantic). Two stacks are equal when they have the same
item, the same count and the same patch. The order of the components in the patch has no effect.
Two empty stacks are equal. See result R11.

## 3. The selected slot

**INV-SEL-001** (`black_box_confirmed`, semantic). The selected slot is a hotbar slot, from 0 to 8.
A player that joins for the first time has the selected slot 0. The main-hand item is the stack in
the selected slot. The block breaking (`block-breaking.md`) and block placing specifications use
this stack. See results R1, R2 and R6.

**INV-SEL-002** (`black_box_confirmed`, semantic). The serverbound `set_carried_item` packet has one
field, the slot (short). A value from 0 to 8 changes the selected slot to that value. The server
sends nothing to the owner. When the old and the new main-hand items are not equal, each observer
gets `set_equipment` with the id 0 and the new main-hand item. When they are equal, the observers
get nothing. See result R6.

**INV-SEL-003** (`black_box_confirmed`, semantic). The server ignores `set_carried_item` with a
value outside 0 to 8. The selected slot does not change, the server sends nothing, and the
connection stays open. See result R6, which tested the values 9, -1 and 100.

**INV-SEL-004** (`black_box_confirmed`, semantic). When a player joins, the server sends
`set_held_slot` with the selected slot. It sends the packet after `login` and before the first
`player_position`. The selected slot of a player that joins again is the value in the player file.
See results R1 and R12.

## 4. Writes by the server

**INV-SET-001** (`black_box_confirmed`, semantic). When a player joins, the server sends
`container_set_content` for the inventory menu, after `set_held_slot`. It has the window id 0, the
state id 1, 46 stacks in the order of INV-SLOT-003, and the carried stack. See results R1 and R12.

**INV-SET-002** (`black_box_confirmed`, semantic). After a write, the owner gets one of these
packets, if INV-SET-004 does not prevent it:

- For the carried slot: `set_cursor_item` with the stack. The packet has no window id and no state
  id.
- When no menu is open: `container_set_slot` with the window id 0, a state id (INV-SET-005), the
  menu index of INV-SLOT-003, and the stack.
- When a menu is open, and the open menu shows the slot: `container_set_slot` with the window id
  and the menu index of the open menu. The open menu of a chest with 27 slots shows the main slots
  9 to 35 at the menu indices 27 to 53, and the hotbar slots at 54 to 62.
- When a menu is open, and the open menu does not show the slot: nothing while the menu stays open.
  After the menu closes, the owner gets `container_set_slot` with the window id 0 for each such
  slot that changed.

See results R2, R10 and R13.

**INV-SET-003** (`black_box_confirmed`, semantic). A write to a slot of the craft grid also
calculates the craft result again. The owner gets `container_set_slot` for the menu index 0 before
the packet for the grid slot. While a menu is open, the owner gets the packets for the menu index 0
at once, and the packet for the grid slot as INV-SET-002 states. The number of packets for the menu
index 0 is unspecified. See results R2, R10 and R15.

**INV-SET-004** (`black_box_confirmed`, semantic). When a write puts a stack that is equal to the
stack in the slot, nobody gets a packet. See result R11.

**INV-SET-005** (`black_box_confirmed`, semantic). Each `container_set_slot` has the state id of its
window. The server adds 1 to the state id of a window for each `container_set_slot` of that window.
The inventory menu and an open menu each have their own state id. An open menu starts at the state
id 1, in its `container_set_content`. Part 2 of this document states the state id in full. See
results R2 and R10.

**INV-SET-006** (`black_box_confirmed`, semantic). When the main-hand item, the stack in the
off-hand slot or the stack in an armour slot changes to a stack that is not equal, each observer
gets `set_equipment` with the slot id of INV-SLOT-005 and the new stack. A change of the count only
is a change. A write to a hotbar slot that is not the selected slot sends nothing to the observers.
Observers get the packet also while the owner has an open menu. See results R2, R11 and R13.

## 5. The creative slot request

**INV-CRE-001** (`black_box_confirmed`, wire). The serverbound `set_creative_mode_slot` packet has
these fields, in this order: slot (short), stack (INV-STK-004). See result R3.

**INV-CRE-002** (`black_box_confirmed`, semantic). The server accepts the request only when the
owner is in creative mode. In survival mode, adventure mode and spectator mode, the slot does not
change and no item drops. The vanilla server then sends nothing; entry D2 states the reply of mcrs.
See result R14.

**INV-CRE-003** (`black_box_confirmed`, semantic). The slot field is a menu index of the inventory
menu (INV-SLOT-003), also when another menu is open. The server acts on it as follows:

| Slot field | Result |
|------------|--------|
| 1 to 45 | The slot gets the stack, when INV-CRE-004 accepts it. The empty stack empties the slot. |
| 0 | Nothing changes. |
| 46 and more | Nothing changes. |
| less than 0 | INV-CRE-005. |

See results R3 and R9.

**INV-CRE-004** (`black_box_confirmed`, semantic). The server accepts the stack when it is empty, or
when its count is not more than its maximum stack size (INV-STK-002). Otherwise, the slot does not
change. See result R5, which tested `stone` with the counts 65 and 99, and two `diamond_sword`.
The vanilla server sends nothing for a refused stack; entry D2 states the reply of mcrs.

**INV-CRE-005** (`black_box_confirmed`, semantic). When the slot field is less than 0 and the stack
is not empty, the server drops the stack as an item entity from the owner. No slot changes. Part 4
states the position and the motion of the item entity. When the stack is empty, nothing occurs.
Result R16 found no limit on the number of drops: 40 requests in one burst gave 40 item entities.
See results R9 and R16.

**INV-CRE-006** (`black_box_confirmed`, semantic). After an accepted request, the owner gets these
packets, and no others:

- When no menu is open: nothing for the slot that changed. A change in the craft grid sends the
  craft result as INV-SET-003 states.
- When a menu is open: `container_set_slot` of the open menu for the slot, if the open menu shows
  it. The owner also gets `container_set_slot` with the window id 0 for each slot that changed
  while the menu was open (INV-SET-002).

Observers get `set_equipment` as INV-SET-006 states. See results R3 and R15.

## 6. Later parts

Parts 2, 3 and 4 add their sections here. Until then, these behaviours have no rule: the
`container_click` packet, `container_close`, the drop of the main-hand item, the swap of the hands,
pick block, and the full player file.

## 7. Intentional deviations

**D1.**

- **Rule**: INV-SLOT-002.
- **Reference**: the `clear` command does not remove the stack in the carried slot (public bug
  report MC-147499).
- **Visible to the client**: yes. After `clear`, the cursor of the player is empty in mcrs.
- **Test**: a differential test must not run `clear` while the carried slot holds a stack.

**D2.**

- **Rule**: INV-CRE-002 and INV-CRE-004. When the server refuses a request with a slot field from 1
  to 45, mcrs must send the owner the stack of that slot, as INV-SET-002 states. Reason: issue 521
  decides that a refused request has one outcome in all editions: refused, and the current state
  follows. Part 2 states this outcome in full.
- **Reference**: the server sends nothing. The client keeps the stack that it predicted, and the
  slot on the client is wrong until the next write to it.
- **Visible to the client**: yes. The slot shows the stack of the server again.
- **Test**: a differential test must not send a refused `set_creative_mode_slot` request.

## 8. Unresolved

- **U1**: the server changes the selected slot, for example for pick block. No result of this part
  shows `set_held_slot` after the join. Part 4 owns pick block.
- **U2**: the clientbound `set_player_inventory` packet. No tested case sent it. Part 4 can find the
  case, for example pick block.
- **U3**: the craft grid when the player leaves, and in the player file. Part 3 owns the craft grid.
- **U4**: the carried stack when the player leaves. In result R12, the stack of the carried slot was
  gone after the join, and an item entity was near the player. Part 4 owns this.
- **U5**: the menu index 7 and the equipment id 3 (`legs`). No result shows them.
- **U6**: a serverbound stack with added components and no length before each value. No experiment
  sent one.
- **U7**: the map of the Bedrock slots to the slots of INV-SLOT-001, and the game rule
  `mcrs:offhand_accepts_any_item` of issue 521. Part 2 owns them.

## 9. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`, `level-type=minecraft:flat` and `network-compression-threshold=-1`. Set the game
rule `spawn_mobs` to false. Connect one scripted client as the owner and one as the observer, 3
blocks apart. Read the packets that arrive after each step, and read the player file with
`data get entity`.

| Experiment | Steps | Result |
|------------|-------|--------|
| E1 | Join a new player. Read `SelectedItemSlot`. | R1 |
| E2 | With the owner in survival mode, write a stack with `item replace` to each slot name of INV-SLOT-004, and to `armor.body` and `saddle`. | R2 |
| E3 | Write `stone` with the counts 64, 65, 99 and 100, `diamond_sword` with the count 2, and `ender_pearl` with the counts 16 and 17. | R5 |
| E4 | Send `set_carried_item` with 1, 3, 3, 0, 8, 9, -1 and 100. Put the same item in two hotbar slots and select one, then the other. | R6 |
| E5 | Leave and join again. Read `SelectedItemSlot`. | R12 |
| E6 | In creative mode, send `set_creative_mode_slot` with 3 `dirt` for the slot fields 36, 0, 1, 4, 5, 8, 9, 44, 45, 46 and 100. Read `Inventory` and `equipment`. | R3 |
| E7 | Send `stone` with the counts 64, 65 and 99, and 2 `diamond_sword`. Send 99 `stone` with `minecraft:max_stack_size` 99 in the patch. | R5, R7 |
| E8 | With a new client each time, send the count 100, the count 0, and `minecraft:max_stack_size` 100. | R8, R9 |
| E9 | Put 5 `stone` in the selected slot. Send the count 0, then put it back and send the count -1. | R9 |
| E10 | With the slot field -1 and -2, send a stack. Send an empty stack with -1. | R9 |
| E11 | Do E6 for the slot field 36 in survival mode, adventure mode and spectator mode. | R14 |
| E12 | Open a chest with `use_item_on`. Do E2 for `hotbar.0`, `inventory.0`, `armor.head`, `weapon.offhand`, `player.cursor`, `player.crafting.0` and `container.0`. | R10 |
| E13 | Write 5 `stone`, then 6 `stone`, to the selected slot. Write `diamond_sword`, then `diamond_sword[damage=10]` two times, then the same sword with a `custom_name`, with the components in the two orders. | R11 |
| E14 | With a chest open, write `armor.head`. Wait 3 ticks. Close the chest with `container_close`, then write `armor.feet`. | R13 |
| E15 | With a chest open, switch to creative mode and send `set_creative_mode_slot` for the slot field 37. | R15 |
| E16 | With a new creative player for each case, far from other players, send `set_creative_mode_slot` with the slot field -1 and one `diamond_sword`: 5, 10, 11, 15 and 40 times in one burst, 40 times one in each tick, 30 times one in two ticks, 8 times one each second, and 15 times in one burst then 10 times one each second. Count the item entities. | R16 |

## 10. Captured results

Each packet is shortened to its name and the fields that matter. `stone x5` is a stack of 5 `stone`.
`P` is the owner and `W` is the observer.

**R1.** A new player.

```text
P: login
   set_held_slot 0
   player_position
   container_set_content win 0 state 1, 46 empty slots, carried empty
SelectedItemSlot: 0
```

**R2.** Writes with `item replace`, owner in survival mode, no open menu, selected slot 0.

```text
hotbar.0 stone 5         P: container_set_slot win 0 state 2 slot 36 stone x5
                         W: set_equipment [0 stone x5]
hotbar.3 dirt 2          P: container_set_slot win 0 state 3 slot 39 dirt x2      W: nothing
inventory.0 stone 7      P: container_set_slot win 0 state 4 slot 9 stone x7      W: nothing
inventory.26 dirt 1      P: container_set_slot win 0 state 5 slot 35 dirt x1      W: nothing
armor.head diamond_helmet
                         P: container_set_slot win 0 state 6 slot 5
                         W: set_equipment [5 diamond_helmet]
armor.chest diamond_chestplate
                         P: container_set_slot win 0 state 7 slot 6
                         W: set_equipment [4 diamond_chestplate]
weapon.offhand shield    P: container_set_slot win 0 state 8 slot 45              W: set_equipment [1 shield]
player.cursor stone 3    P: set_cursor_item stone x3                              W: nothing
player.crafting.0 dirt 4 P: container_set_slot win 0 state 9 slot 0 empty
                            container_set_slot win 0 state 10 slot 0 empty
                            container_set_slot win 0 state 11 slot 1 dirt x4
weapon.mainhand diamond_sword
                         P: container_set_slot win 0 state 12 slot 36             W: set_equipment [0 diamond_sword]
armor.body stone, saddle stone:
  "No targets accepted item 1 minecraft:stone into the specified slots"; no packets
```

**R3.** `set_creative_mode_slot` with 3 `dirt`, owner in creative mode, no open menu.

```text
slot 36   Inventory Slot 0b; P: nothing; W: set_equipment [0 dirt x3]
slot 0    no change; P: nothing
slot 1    P: container_set_slot win 0 state 2 slot 0 empty
slot 4    P: container_set_slot win 0 state 3 slot 0 empty
slot 5    equipment head; P: nothing; W: set_equipment [5 dirt x3]
slot 8    equipment feet; P: nothing; W: set_equipment [2 dirt x3]
slot 9    Inventory Slot 9b; P: nothing
slot 44   Inventory Slot 8b; P: nothing
slot 45   equipment offhand; P: nothing; W: set_equipment [1 dirt x3]
slot 46   no change
slot 100  no change
```

**R4.** The player file after R2.

```text
Inventory: [{count: 1, Slot: 0b, id: "minecraft:diamond_sword"}, {count: 2, Slot: 3b, id: "minecraft:dirt"},
            {count: 7, Slot: 9b, id: "minecraft:stone"}, {count: 1, Slot: 35b, id: "minecraft:dirt"}]
equipment: {head: {count: 1, id: "minecraft:diamond_helmet"}, chest: {count: 1, id: "minecraft:diamond_chestplate"},
            offhand: {count: 1, id: "minecraft:shield"}}
SelectedItemSlot: 0
```

**R5.** Counts.

```text
item replace ... stone 64          accepted
item replace ... stone 65, 99      "minecraft:stone can only stack up to 64"
item replace ... stone 100         "Integer must not be more than 99: found 100"
item replace ... diamond_sword 2   "minecraft:diamond_sword can only stack up to 1"
item replace ... ender_pearl 16    accepted
item replace ... ender_pearl 17    "minecraft:ender_pearl can only stack up to 16"
set_creative_mode_slot slot 36:
  stone x64                        accepted
  stone x65, stone x99             slot keeps stone x64; P: nothing
  diamond_sword x2                 slot keeps stone x64; P: nothing
```

**R6.** `set_carried_item`, in this order. Hotbar: 0 `diamond_sword`, 1 `iron_ingot x3`, 3
`diamond_sword`, 8 empty.

```text
1    SelectedItemSlot 1; P: nothing; W: set_equipment [0 iron_ingot x3]
3    SelectedItemSlot 3; P: nothing; W: set_equipment [0 diamond_sword]
3    SelectedItemSlot 3; P: nothing; W: nothing
0    SelectedItemSlot 0; P: nothing; W: nothing        (equal swords)
8    SelectedItemSlot 8; P: nothing; W: set_equipment [0 empty]
9    SelectedItemSlot 8; P: nothing; W: nothing; connection open
-1   the same
100  the same
```

**R7.** `set_creative_mode_slot` slot 36 with 99 `stone` and the added component
`minecraft:max_stack_size` 99, with a length before the value.

```text
Inventory: [{components: {"minecraft:max_stack_size": 99}, count: 99, Slot: 0b, id: "minecraft:stone"}]
```

**R8.** `set_creative_mode_slot` slot 36 with a count of 100, and with 1 `stone` and
`minecraft:max_stack_size` 100.

```text
P: disconnect; the reason ends with
   "Failed to decode packet 'serverbound/minecraft:set_creative_mode_slot'"
```

**R9.**

```text
slot 36, count 0, slot holds stone x5      slot empty; P: nothing; W: set_equipment [0 empty]
slot 36, count -1, slot holds stone x5     slot empty; P: nothing; W: set_equipment [0 empty]
slot -1, stone x5                          an item entity; no slot changes
slot -2, dirt x3                           an item entity; no slot changes
slot -1, empty stack                       nothing
```

**R10.** Writes with a chest of 27 slots open (window 1), owner in survival mode.

```text
open chest                P: open_screen win 1; container_set_content win 1 state 1, 63 slots
hotbar.0 stone 5          P: container_set_slot win 1 state 2 slot 54 stone x5
inventory.0 dirt 2        P: container_set_slot win 1 state 3 slot 27 dirt x2
armor.head                P: nothing
weapon.offhand            P: nothing
player.cursor stone 3     P: set_cursor_item stone x3
player.crafting.0 dirt 4  P: container_set_slot win 0 state 16 slot 0 empty (two times)
container.0 stone 9       P: container_set_slot win 1 state 4 slot 54 stone x9
item replace block <chest> container.1 dirt 6
                          P: container_set_slot win 1 state 5 slot 1 dirt x6
```

**R11.** Writes to the selected slot.

```text
stone 5                                 P: container_set_slot slot 36 stone x5; W: set_equipment
stone 6                                 P: container_set_slot slot 36 stone x6; W: set_equipment [0 stone x6]
diamond_sword                           P: container_set_slot; W: set_equipment
diamond_sword[damage=10]                P: container_set_slot; W: set_equipment
  clientbound stack bytes: 01 b108 01 00 03 0a
  (count 1, item 1073, 1 added, 0 removed, type 3 minecraft:damage, value 10)
diamond_sword[damage=10] again          P: nothing; W: nothing
diamond_sword[custom_name='x',damage=10]
                                        P: container_set_slot; W: set_equipment
diamond_sword[damage=10,custom_name='x']
                                        P: nothing; W: nothing
```

**R12.** The owner leaves and joins again. Before it left, the selected slot was 3 and the carried
slot held `stone x3`.

```text
P: login
   set_held_slot 3
   player_position
   container_set_content win 0 state 1; the main, armour and off-hand slots as before; carried empty
   add_entity of an item entity
SelectedItemSlot: 3
```

**R13.** A chest is open. `armor.head` gets `diamond_helmet`.

```text
P: nothing, also for 3 ticks after
W: set_equipment [5 diamond_helmet] at once
container_close win 1:
P: container_set_slot win 0 state 4 slot 5 diamond_helmet
armor.feet diamond_boots after the close:
P: container_set_slot win 0 state 5 slot 8 diamond_boots
```

**R14.** `set_creative_mode_slot` slot 36 with 3 `stone`, and slot -1 with 3 `stone`.

```text
survival:   no change, no item entity; P: nothing; W: nothing
adventure:  no change; P: nothing
spectator:  no change; P: nothing
```

**R15.** After R10, with the chest open, the owner is in creative mode and sends
`set_creative_mode_slot` slot 37 with 2 `stone`.

```text
P: container_set_slot win 0 state 18 slot 1 dirt x4
   container_set_slot win 0 state 19 slot 5 diamond_helmet
   container_set_slot win 0 state 20 slot 9 dirt x2
   container_set_slot win 0 state 21 slot 36 stone x9
   container_set_slot win 0 state 22 slot 45 shield
   container_set_slot win 1 state 6 slot 55 stone x2
Inventory: main slot 1 holds stone x2
```

**R16.** Drops with the slot field -1, a new player for each case.

```text
5, 10, 11, 15, 40 in one burst        5, 10, 11, 15, 40 item entities
40, one each tick                     40
30, one each two ticks                30
8, one each second                    8
15 in one burst, then 10 at 1 s       25
```
