# Player inventory: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. All captured results of section 14 come
from this version.

This document states the inventory of a player: its slots, the selected hotbar slot, the stacks in
the slots, and what the server sends when a stack changes. It states only what a client, the server
console and the server files show. It does not prescribe an architecture. All rules come from a
vanilla server in offline mode and scripted clients, from the public protocol documentation of
Bedrock Edition, or from the decisions in issue 521. No rule has the label `source_suggested`.

## Parts of this document

Issue 521 is too large for one change. This document grows in four parts, in this order:

1. Slots, stacks, the selected slot, the writes of the server, `set_creative_mode_slot` and pick
   block (sections 1 to 6, prefixes `INV-SLOT`, `INV-STK`, `INV-SEL`, `INV-SET`, `INV-CRE`,
   `INV-PCK`). This part is written.
2. The transaction model for all editions: the atomic operations, the rejection, the state id and
   the resync (sections 7 to 9, prefixes `INV-OP`, `INV-REJ`, `INV-STATE`). This part is written.
3. The click modes of Java Edition and the carried stack, as sequences of the operations of part 2
   (prefix `INV-CLK`).
4. The hand actions (drop, swap of the hands), the insertion of a stack into the inventory, the
   `clear` command, and the storage in the player file (prefixes `INV-HND`, `INV-STO`).

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
- **Eye position**: as `block-breaking.md`, section 0.1, and BRK-RCH-003 define it.
- **Bedrock edge**: the part of mcrs that connects a client of Bedrock Edition.
- **Operation**: one atomic change of the slots of the owner (INV-OP-001).
- **Request**: what a client sends to ask for operations: one `container_click` packet, or one item
  stack request of Bedrock Edition.
- **Room** of a slot for a stack: the slot limit (INV-OP-002) minus the count in the slot, when the
  slot is empty or holds an equal stack. Else 0.
- **Claim**: a slot and a stack in a `container_click` packet. The client states with it the stack
  that it expects in the slot after the click.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 14.
- `documentation`: publicly documented behaviour, for example a report on the public bug tracker.
- `design`: an mcrs choice. The rule states the reason.
- `unresolved`: not decided. Section 12 collects these items.

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

The carried slot has no menu index. See results R2, R3 and R17.

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

Provenance: `black_box_confirmed` for the ids 0 to 5 (results R2, R3 and R17). `documentation` for
the ids 6 and 7. The server never sends the ids 6 and 7 for a player, because it has no such slots.

**INV-SLOT-006** (`black_box_confirmed`, semantic). The player file keeps the slots in these fields.
Part 4 of this document states the full storage.

- `Inventory`: a list with one entry for each main slot that is not empty. An entry has the fields
  `Slot` (byte, the index of the main slot), `id`, `count` and, when the patch is not empty,
  `components`.
- `equipment`: a compound with the keys `head`, `chest`, `legs`, `feet` and `offhand`. A key is
  absent when its slot is empty.
- `SelectedItemSlot`: the selected slot, an int.

See results R4 and R12.

**INV-SLOT-007** (`black_box_confirmed`, semantic). When the owner leaves, the server drops the stack
of the carried slot and the stacks of the craft grid as item entities from the owner. It drops them
also when the main slots have room. After the owner joins again, the carried slot and the craft grid
are empty. Part 4 states the position and the motion of the item entities. See results R12 and R22.
Entry U11 records an open decision about the carried slot.

**INV-SLOT-008** (`documentation`, wire). The Bedrock edge maps the slots of INV-SLOT-001 to the
containers of Bedrock Edition as this table states. The container ids are the values of the
container id enum, and the container names are the values of the container name enum, of the
public Bedrock protocol documentation for version `1.26.60`.

| Slot | Container id, slot | Container name in an item stack request, slot |
|------|--------------------|-----------------------------------------------|
| hotbar slot `n` | 0 (inventory), `n` | 28 (hotbar), `n`; or 12 (hotbar and inventory), `n` |
| main slot `n`, 9 to 35 | 0 (inventory), `n` | 29 (inventory), `n`; or 12 (hotbar and inventory), `n` |
| `head`, `chest`, `legs`, `feet` | 120 (armour), 0, 1, 2, 3 | 6 (armour), 0, 1, 2, 3 |
| off-hand slot | 119 (off hand), 0 | 34 (off hand), 0 |
| carried slot | 124 (player UI), 0 | 59 (cursor), 0 |
| craft grid, menu indices 1 to 4 | 124 (player UI), 28 to 31 | 13 (craft input), 28 to 31 |

The order head, torso, legs, feet of the armour container comes from the armour slot enum of the
documentation. The slots 0 and 28 to 31 of the player UI container come from its slot table.
Confidence: medium for the slot numbers in the containers 0, 12, 28 and 29, which the documentation
does not state. An experiment with a Bedrock client must confirm them when the edge exists (U7).

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

- an added component with no length before its value.

The disconnect reason contains `Failed to decode packet
'serverbound/minecraft:set_creative_mode_slot'`. A count of 0 or less decodes as the empty stack.
See results R7, R8, R9 and R24.

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
- When a menu is open, and the open menu does not show the slot: nothing at once. The owner gets
  `container_set_slot` with the window id 0 for each such slot that changed, at the first of these
  events: the menu closes, or the owner sends `set_creative_mode_slot`. In result R15, a
  `set_creative_mode_slot` request with the chest open sends the window id 0 packets for the slots
  1, 5, 9, 36 and 45, which earlier writes changed. These include the slots 9 and 36, which the
  chest shows and which already got their packets of the chest window. The results test no other
  event.

See results R2, R10, R13 and R15.

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

**INV-SET-007** (`black_box_confirmed`, semantic). When the owner sends `container_close` with the
window id 0, the server moves the stack of the carried slot into the main slots, and then the stacks
of the craft grid. Part 4 states which main slot gets a stack. The owner gets these packets:

1. `set_player_inventory` for each main slot that gets a stack. The packet has the index of the main
   slot (VarInt) and the stack.
2. `container_set_slot` with the window id 0 for each slot that changed, the craft grid included.
3. `set_cursor_item` with the empty stack.

The main slots take as many items of a stack as they have room for. The server drops the remainder
of each stack as an item entity from the owner, as for a drop of the main-hand item, which part 4
states. A stack that the main slots do not take at all sends no `set_player_inventory`.

The move of the craft grid on the close is not a write in the sense of INV-SET-003. The owner gets
no `container_set_slot` for the menu index 0, and the state id grows only for the packets of the
list above. See results R23 and R25.

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

## 6. Pick block

**INV-PCK-001** (`black_box_confirmed`, wire). The serverbound `pick_item_from_block` packet has
these fields, in this order: position (long), include data (boolean). The serverbound
`pick_item_from_entity` packet has these fields, in this order: entity id (VarInt), include data
(boolean). See results R18 and R21.

**INV-PCK-002** (`black_box_confirmed`, semantic). The server ignores `pick_item_from_block`, and
sends nothing, when the distance from the eye position to the nearest point of the block cube is
equal to or more than a limit. The limit is the same as in BRK-RCH-001: the value of the
`minecraft:block_interaction_range` attribute plus 1.0. By default, this is 5.5 in survival mode
and 6.0 in creative mode. With the base value of the attribute set to 8, it is 9.0 in survival mode
and 9.5 in creative mode. At a distance equal to the limit, the server ignores the request. See
results R20 and R33.

The server ignores `pick_item_from_entity` in the same way when the distance from the eye position
to the nearest point of the bounding box of the entity is more than a limit. The limit is the value
of the `minecraft:entity_interaction_range` attribute plus 3.0. The attribute is 3.0 by default, and
creative mode adds 2.0 to it. By default, the limit is 6.0 in survival mode and 8.0 in creative
mode. With the base value of the attribute set to 5, it is 8.0 in survival mode and 10.0 in
creative mode. Result R26 shows the default limits straight and on the diagonal. On the diagonal,
the distance to the centre of the entity is more than the limit, so the limit applies to the
bounding box. At a distance of 6.0 and 8.0, result R33 shows no reply, but the box of that test is
at 0.45 from the centre of the entity, which a binary number does not hold exactly. So a distance
equal to the limit is unspecified for an entity. See results R26 and R33.

**INV-PCK-003** (`black_box_confirmed`, semantic). The picked stack is a stack with the count 1:

- for a block, the item of the block, for example `stone` for stone and `chest` for a chest;
- for an entity, its spawn egg, for example `pig_spawn_egg` for a pig.

In creative mode, when include data is true and the block has a block entity, the picked stack also
has the data of the block entity as components. For a chest with items, this is the
`minecraft:container` component. Outside creative mode, include data has no effect. See result R21.
INV-PCK-008 states the picked item of each block and each entity.

**INV-PCK-004** (`black_box_confirmed`, semantic). A main slot matches when its stack has the item and
the patch of the picked stack. The count has no effect. A `stone` with a `minecraft:custom_name` does
not match a picked `stone`. When more than one main slot matches, the main slot with the lowest index
is the match. See results R18 and R19.

**INV-PCK-005** (`black_box_confirmed`, semantic). The target slot is the first hotbar slot in this
list:

1. the selected slot, when it is empty;
2. the first empty hotbar slot, in the order `selected`, `selected + 1`, and so on, modulo 9;
3. the first hotbar slot, in the same order, whose stack has no enchantment in its
   `minecraft:enchantments` component;
4. the selected slot.

See results R18 and R19.

**INV-PCK-006** (`black_box_confirmed`, semantic). The server then changes the inventory as this table
states, in all game modes that the results tested: creative, survival and adventure.

| Case | Result |
|------|--------|
| The match is a hotbar slot. | The selected slot changes to the match. No stack moves. |
| The match is a main slot from 9 to 35. | The stacks of the match and of the target slot change places. The selected slot changes to the target slot. |
| No match, creative mode. | The stack of the target slot moves to the first empty main slot. When no main slot is empty, the server deletes that stack. The target slot gets the picked stack. The selected slot changes to the target slot. |
| No match, survival mode or adventure mode. | Nothing changes. |

See results R18, R19, R21 and R27.

**INV-PCK-007** (`black_box_confirmed`, semantic). When the block or the entity has no picked item
(INV-PCK-008), the server sends nothing and changes nothing, also in reach. Else, in reach, the owner
first gets `set_held_slot`
with the selected slot, also when the selected slot does not change and nothing moves. Then the
owner gets `container_set_slot` for each main slot that changed: first the main slot that lost the
match or got the old stack, then the target slot. Observers get `set_equipment` as INV-SET-006
states. See results R18 and R21.

**INV-PCK-008** (`black_box_confirmed`, semantic). The picked item of a block, in its default
state, is the item with the name of the block, with these exceptions:

| Block | Picked item |
|-------|-------------|
| a block whose name has `wall_` in it: the wall signs, the wall hanging signs, the wall banners, the wall torches, the wall heads and skulls, the coral wall fans | the item whose name is the block name without `wall_`, for example `oak_sign` for `oak_wall_sign` and `torch` for `wall_torch` |
| `potted_<plant>` | `<plant>`, but `azalea` for `potted_azalea_bush` and `flowering_azalea` for `potted_flowering_azalea_bush` |
| `candle_cake` and each `<colour>_candle_cake` | `cake` |
| `water_cauldron`, `lava_cauldron`, `powder_snow_cauldron` | `cauldron` |
| `melon_stem`, `attached_melon_stem` | `melon_seeds` |
| `pumpkin_stem`, `attached_pumpkin_stem` | `pumpkin_seeds` |
| `wheat`, `beetroots`, `torchflower_crop` | `wheat_seeds`, `beetroot_seeds`, `torchflower_seeds` |
| `carrots`, `potatoes`, `pitcher_crop` | `carrot`, `potato`, `pitcher_pod` |
| `cave_vines`, `cave_vines_plant` | `glow_berries` |
| `kelp_plant`, `twisting_vines_plant`, `weeping_vines_plant` | `kelp`, `twisting_vines`, `weeping_vines` |
| `bamboo_sapling`, `big_dripleaf_stem`, `tall_seagrass` | `bamboo`, `big_dripleaf`, `seagrass` |
| `cocoa`, `sweet_berry_bush` | `cocoa_beans`, `sweet_berries` |
| `redstone_wire`, `tripwire`, `piston_head`, `powder_snow` | `redstone`, `string`, `piston`, `powder_snow_bucket` |
| `decorated_pot` | `decorated_pot` with the `minecraft:pot_decorations` component, also when include data is false |
| `air`, `cave_air`, `void_air`, `water`, `lava`, `fire`, `soul_fire`, `nether_portal`, `end_portal`, `end_gateway`, `moving_piston`, `frosted_ice` | no picked item |

The picked item of an entity is its spawn egg, with these exceptions:

| Entity | Picked item |
|--------|-------------|
| each boat, chest boat and raft; `minecart`, `chest_minecart`, `furnace_minecart`, `hopper_minecart`, `tnt_minecart`, `command_block_minecart`; `armor_stand`, `end_crystal`, `item_frame`, `glow_item_frame`, `painting` | the item with the name of the entity |
| `spawner_minecart` | `minecart` |
| `leash_knot` | `lead` |
| `cushion` | `white_cushion` |
| the projectiles (`arrow`, `spectral_arrow`, `trident`, `egg`, `snowball`, `ender_pearl`, `eye_of_ender`, `experience_bottle`, `splash_potion`, `lingering_potion`, `fireball`, `small_fireball`, `dragon_fireball`, `wither_skull`, `shulker_bullet`, `llama_spit`, `wind_charge`, `breeze_wind_charge`, `ice_ball`, `firework_rocket`); `block_display`, `item_display`, `text_display`, `interaction`, `area_effect_cloud`, `evoker_fangs`, `experience_orb`, `falling_block`, `lightning_bolt`, `ominous_item_spawner`, `tnt`; `giant`, `illusioner`, `mannequin` | no picked item |

The results do not cover `bubble_column`, which disappears without water, and the entities
`player`, `marker`, `item` and `fishing_bobber`, which a client cannot target or the `summon` command
cannot make. See results R28 and R29.

## 7. Operations

**INV-OP-001** (`design`, semantic). A request asks for one or more operations. These are the
operations:

| Operation | Effect |
|-----------|--------|
| take | Move items from a slot to the carried slot. |
| place | Move items from the carried slot to a slot. |
| swap | Exchange the stacks of two slots. One of the two can be the carried slot. |
| split | Take half of the stack of a slot (INV-OP-005). |
| collect | Fill the carried slot with equal stacks from other slots (INV-OP-007). |
| drop | Move items from a slot or from the carried slot to an item entity (INV-OP-008). |
| take craft result | Craft one time and move the result (INV-OP-009). |
| creative set | Put a new stack into a slot, in creative mode only (INV-OP-010). |

The server does all the operations of one request, or none of them. When one operation cannot
complete, no slot changes, no item entity appears, and the server refuses the request (section 8).
Reason: issue 521 decides this transaction model, so that the clients of Java Edition and of
Bedrock Edition use one model. Bedrock Edition names its request actions `Take`, `Place`, `Swap`,
`Drop`, `Destroy`, `Create` and `CraftCreative`, among others, in the public protocol documentation.

The rules of this section give the effect of each operation. They come from the clicks of Java
Edition in the inventory menu. Part 3 states which click is which sequence of operations.

**INV-OP-002** (`black_box_confirmed`, semantic). The slot limit of a slot for a stack is the maximum
stack size of the stack (INV-STK-002), with these exceptions:

- An armour slot has the limit 1.
- The craft result accepts no stack from the owner.
- An armour slot accepts only a stack that fits it: for example `diamond_helmet` and
  `carved_pumpkin` fit `head`, and `diamond_helmet` does not fit `chest`. A stack that does not fit
  has the room 0.
- The off-hand slot accepts all stacks. This is the behaviour when the game rule
  `mcrs:offhand_accepts_any_item` of issue 521 is true, which is its default value. When it is
  false, entry U7 applies.

See results R30 (cases P, P2 to P6, Q5) and R31 (case K4).

**INV-OP-003** (`black_box_confirmed`, semantic). Take moves `n` items from a slot to the carried
slot. It completes when the slot holds at least `n` items, and the carried slot is empty or holds
an equal stack with room for `n` items. A take from an armour slot or from the off-hand slot is a
take like any other. See result R30 (cases A, B, J6, J7, Q2).

**INV-OP-004** (`black_box_confirmed`, semantic). Place moves `n` items from the carried slot to a
slot. It completes when the carried slot holds at least `n` items and the slot has room for `n`
items. When the slot holds a stack that is not equal, place does not complete; the request uses
swap instead (part 3). See result R30 (cases C, D, E, E2, H, P to P6).

**INV-OP-005** (`black_box_confirmed`, semantic). Split takes `ceil(c / 2)` items of a slot with `c`
items to the empty carried slot. The slot keeps `floor(c / 2)` items. For `c` = 11, the carried
slot gets 6 and the slot keeps 5. For `c` = 10, each gets 5. See results R30 (case B) and R31
(case R4b).

**INV-OP-006** (`black_box_confirmed`, semantic). Swap exchanges the stacks of two slots. It
completes when each slot accepts the stack of the other slot (INV-OP-002), and the two stacks are
not equal. When the two stacks are equal, nothing changes. Swap between a slot and the carried slot
is the same. See results R30 (cases F, G, K, K2, K3, P6) and R31 (case K4).

**INV-OP-007** (`black_box_confirmed`, semantic). Collect moves items of equal stacks into the
carried slot until the carried slot holds its maximum stack size. It completes only when the
carried slot holds a stack, and the slot that the request names is empty. The server takes the
items from the slots in this order:

1. the slots that do not hold their maximum stack size, in the order of the menu index;
2. the slots that hold their maximum stack size, in the same order.

The order of the menu index is ascending for the button 0 and descending for the button 1. See
results R30 (cases O and O2) and R31 (case R6d).

**INV-OP-008** (`black_box_confirmed`, semantic). Drop moves items to one item entity from the
owner. The source is the carried slot or a slot. A drop from a slot completes only when the carried
slot is empty. The count is 1 or the full stack. Part 4 states the position and the motion of the
item entity. See result R30 (cases I, I2, L, L2, L3).

**INV-OP-009** (`black_box_confirmed`, semantic). Take craft result crafts one time. The craft
result gets the result of the recipe of the craft grid, the server removes one item from each
craft grid slot that holds a stack, and the craft result shows the result of the new craft grid.
The result then moves:

- to the carried slot, when it is empty or holds an equal stack with room for the full result;
- to an item entity, for a drop;
- into the main slots, for a quick move (part 3).

Else, nothing changes. A take of part of the result is not possible: the button 1 takes the full
result too. See result R30 (cases Q to Q8).

**INV-OP-010** (`black_box_confirmed`, semantic). Creative set has two forms:

- the `set_creative_mode_slot` request of section 5;
- a copy: the carried slot or a slot gets a copy of a stack, with the maximum stack size as its
  count.

The server refuses creative set when the owner is not in creative mode. See result R30 (cases M,
M2, N3, N4).

**INV-OP-011** (`black_box_confirmed`, semantic). The game mode limits the operations:

- creative mode: all operations.
- survival mode and adventure mode: all operations except creative set.
- spectator mode: no operation. The server refuses each request (INV-REJ-003).

Issue 521 adds the game rule `mcrs:spectators_open_menus` (default true). It acts on open menus,
so a later part states it. See results R30 and R31 (cases R7 to R7c).

## 8. Rejection

**INV-REJ-001** (`design`, semantic). A refused request has one outcome in all editions: no slot
changes, and the server then sends the current stack of each slot that the client can show wrong.
Reason: issue 521 decides this outcome. For Java Edition, these slots are the slots that the
request claims and the carried slot. For Bedrock Edition, the reply is an item stack response with
a status other than `Success`; the status values are in the public protocol documentation.

**INV-REJ-002** (`black_box_confirmed`, semantic). The vanilla server ignores a `container_click`
packet, changes nothing and sends nothing, when:

- the window id is not the window id of the menu that the owner has (0 when no menu is open);
- the slot is not a menu index of the menu and is not -999, for example 46, 200 or -1;
- the button is not valid for the mode, for example the button 5 for the mode 0;
- the slot is -999 and the mode is 1. The results test no other mode with the slot -999, except 0
  and 5;
- a mode 5 step comes without the start step before it, or after a click of another mode. A click
  of another mode that comes between the start step and the end step is ignored too, and the drag
  then places nothing;
- the operations cannot complete, for example a place into an armour slot that does not fit.

In the last case only, the server sends the corrections of INV-STATE-003, when the claims of the
client are wrong. For the other cases, entry D3 states the reply of mcrs. See result R31.

**INV-REJ-003** (`black_box_confirmed`, semantic). When the owner is in spectator mode, the server
changes nothing and sends `container_set_content` with the window id 0 (INV-STATE-002). See result
R31 (cases R7, R7b).

**INV-REJ-004** (`black_box_confirmed`, wire). A `container_click` packet with more than 128 claims
does not decode. The server disconnects the client. 128 claims decode. See results R31 (case R10)
and R32 (case X4).

**INV-REJ-005** (`black_box_confirmed`, semantic). A mode value of 7 acts as the mode 0. Result R31
(case R9) shows this one value only.

## 9. State id and resync

**INV-STATE-001** (`black_box_confirmed`, wire). The `container_click` packet has these fields, in
this order:

1. window id (VarInt);
2. state id (VarInt);
3. slot (16-bit signed integer);
4. button (byte);
5. mode (VarInt);
6. the claims: a count (VarInt), then for each claim the slot (16-bit signed integer) and a hashed
   stack;
7. the hashed stack of the carried slot.

A hashed stack is a boolean, false for the empty stack. When it is true, these fields follow: item
id (VarInt), count (VarInt), the added components (a count, then for each a component type id as
a VarInt and a hash as a 32-bit integer), and the removed components (a count, then for each a
component type id as a VarInt). The results use only stacks with no components. The hash of a
component value is unresolved (U12). See results R30 and R31.

**INV-STATE-002** (`black_box_confirmed`, semantic). The server keeps a state id for the inventory
menu. It adds 1 to the state id before it sends each `container_set_slot` or
`container_set_content` with the window id 0, and puts the new value in the packet. After a join,
the first `container_set_content` has the state id 1. A request that causes no packet does not
change the state id. See results R22, R23, R25 and R31 (cases R2, R2b). The value after 32767 is
unresolved (U13).

**INV-STATE-003** (`black_box_confirmed`, semantic). When the state id of a `container_click` packet
is equal to the state id of the server, the server does the operations, and then compares:

- each slot that the operations changed, and each slot with a claim, with the stack that the
  client expects: the claim when there is one, else the stack before the click;
- the carried slot with the hashed stack of the carried slot of the packet.

For each slot that differs, the owner gets `container_set_slot` with the window id 0. When the
carried slot differs, the owner gets `set_cursor_item`. When all the claims are correct, the server
sends nothing. A claim for a slot that is not a menu index has no effect. See result R31 (cases R2,
R3, R3b, R4, R4b, R8e).

**INV-STATE-004** (`black_box_confirmed`, semantic). When the state id of a `container_click` packet
is not equal to the state id of the server, the server does the operations as INV-STATE-003 states,
and then sends `container_set_content` with the window id 0: all the slots of the menu and the
carried stack. It sends no `container_set_slot` for the request. The state id of the packet can be
less or more than the state id of the server. See result R31 (cases R1, R1b).

## 10. Later parts

Parts 3 and 4 add their sections here. Until then, these behaviours have no rule: the mapping of
each click mode of `container_click` to operations, the quick move, the drag, `container_close` of
an open menu, the drop of the main-hand item, the swap of the hands, the insertion of a stack into
the inventory, and the full player file. Results R30 to R32 already record the clicks that part 3
uses.

## 11. Intentional deviations

**D1.**

- **Rule**: INV-SLOT-002.
- **Reference**: the `clear` command does not remove the stack in the carried slot (public bug
  report MC-147499).
- **Visible to the client**: yes. After `clear`, the cursor of the player is empty in mcrs.
- **Test**: a differential test must not run `clear` while the carried slot holds a stack.

**D2.**

- **Rule**: INV-CRE-002 and INV-CRE-004. When the server refuses a request with a slot field from 1
  to 45, mcrs must send the owner the stack of that slot, as INV-SET-002 states. When an open menu
  does not show the slot, mcrs sends `container_set_slot` with the window id 0 for the slot at
  once, and does not wait for the events of INV-SET-002. Reason: issue 521
  decides that a refused request has one outcome in all editions: refused, and the current state
  follows. Part 2 states this outcome in full.
- **Reference**: the server sends nothing. The client keeps the stack that it predicted, and the
  slot on the client is wrong until the next write to it.
- **Visible to the client**: yes. The slot shows the stack of the server again.
- **Test**: a differential test must not send a refused `set_creative_mode_slot` request.

**D3.**

- **Rule**: INV-REJ-001 and INV-REJ-002. When mcrs ignores a `container_click` packet, it sends the
  stack of each slot that the packet claims and that is a menu index, and the carried stack. Reason:
  issue 521 decides that a refused request has one outcome in all editions.
- **Reference**: the vanilla server sends nothing. The client keeps its prediction until the next
  write to the slot.
- **Visible to the client**: yes. The slots show the stacks of the server again.
- **Test**: a differential test must not send a `container_click` packet that INV-REJ-002 ignores.

## 12. Unresolved

- **U7**: the slot numbers of INV-SLOT-008 that the documentation does not state, and the game rule
  `mcrs:offhand_accepts_any_item` of issue 521. The game rule acts on a move into the off-hand slot,
  so part 2 owns it.
- **U12**: the hash of a component value in a hashed stack (INV-STATE-001).
- **U13**: the state id after the value 32767 (INV-STATE-002).
- **U11**: the carried slot when the player leaves. INV-SLOT-007 states the Java Edition behaviour:
  the server drops the stack. The Bedrock server keeps the stack in the player data. Issue 521 makes
  the carried stack a slot, but does not decide whether mcrs keeps it when the player leaves.

## 13. Experiments

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
| E17 | Write `diamond_leggings` to `armor.legs`. | R17 |
| E18 | Set stone 2 blocks in front of the owner. For each case of R18, prepare the inventory with `item replace` and `set_carried_item`, and send `pick_item_from_block` on the stone. Send it also on a block 40 blocks away. | R18 |
| E19 | Do E18 for each case of R19. | R19 |
| E20 | Set stone at (8,-59,0). Put the owner at `x` 2.6, 2.4, 2.1 and 1.9, with `z` 0.5, in survival mode and in creative mode. Send `pick_item_from_block` on the stone. | R20 |
| E21 | Set a chest with 2 `diamond` in it. Pick it in creative mode with include data true and false, and in survival mode with a `chest` in main slot 9. Summon a pig with no AI and send `pick_item_from_entity` in creative mode, in survival mode with 2 `pig_spawn_egg` in main slot 20, and in survival mode with no egg. | R21 |
| E22 | In survival mode, write `dirt 4` to `player.crafting.0`, `stone 2` to `player.crafting.3` and `iron_ingot 3` to `player.cursor`. Leave. Read the item entities. Join again. Do it again with 36 `diamond_sword` in the main slots. | R22 |
| E23 | Write `dirt 4` to `player.crafting.0` and `iron_ingot 3` to `player.cursor`, with empty main slots. Send `container_close` with the window id 0. | R23 |
| E24 | Send `set_creative_mode_slot` with 5 `stone` and the added component `minecraft:max_stack_size` 10, with no length before the value. | R24 |
| E25 | In survival mode, fill the main slots and the craft grid as R25 states, and send `container_close` with the window id 0. Read the item entities and the main slots. | R25 |
| E26 | Summon a pig with no AI and no gravity at the eye height. Move it to each gap of R26, along the x axis and on the diagonal. Send `pick_item_from_entity`. | R26 |
| E27 | Fill the main slots as R27 states and send `pick_item_from_block` on stone 2 blocks away. | R27 |
| E28 | In creative mode with an empty inventory, for each block of the block registry, set the block in its default state 2 blocks in front of the owner, empty hotbar slot 0 with `set_creative_mode_slot`, send `pick_item_from_block` with include data false, and record the stack of `container_set_slot`. For each entity type, summon the entity with no AI and no gravity and send `pick_item_from_entity`. Do each case that gives no stack again with longer waits, and with the support block that the block needs. | R28 |
| E29 | Pick `decorated_pot` in creative mode with include data false and true. | R29 |
| E30 | In survival mode, unless the case states another mode, prepare the slots with `item replace` and send each `container_click` of R30, with the current state id, no claims and the empty carried stack. Read the packets after each click, the `Inventory` field and the item entities. | R30 |
| E31 | Do E30 for each case of R31, with the state id and the claims that the case states. Do the cases that can disconnect at the end, each with a new client. | R31 |
| E32 | Do E30 for each case of R32. | R32 |
| E33 | Do E20 at the distances of R33, with the default attribute and with `attribute invp minecraft:block_interaction_range base set 8`. Do E26 at the gaps of R33, with the default attribute and with `attribute invp minecraft:entity_interaction_range base set 5`. | R33 |

## 14. Captured results

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

**R17.**

```text
P: container_set_slot win 0 slot 7 diamond_leggings
W: set_equipment [3 diamond_leggings]
```

**R18.** Pick block on stone 2 blocks away. `sel` is the selected slot before the request.

```text
creative, empty, sel 0              P: set_held_slot 0; container_set_slot slot 36 stone x1
                                    W: set_equipment [0 stone x1]
creative, stone x3 in hotbar 4      P: set_held_slot 4; W: set_equipment [0 stone x3]; no slot changes
creative, stone x3 in main 20, sel 0 empty
                                    P: set_held_slot 0; container_set_slot slot 20 empty; slot 36 stone x3
creative, stone x3 in main 20, sword in hotbar 0, sel 0
                                    P: set_held_slot 1; container_set_slot slot 20 empty; slot 37 stone x3
creative, no stone, sword in hotbar 0, dirt in hotbar 1, sel 0
                                    P: set_held_slot 2; container_set_slot slot 38 stone x1
creative, no stone, hotbar 0-8 full of dirt, sel 3
                                    P: set_held_slot 3; container_set_slot slot 9 dirt x4; slot 39 stone x1
survival, no stone, sword in hotbar 0
                                    P: set_held_slot 0; W: nothing; no slot changes
survival, stone x3 in hotbar 4      P: set_held_slot 4; W: set_equipment [0 stone x3]
survival, stone x3 in main 20, sword in hotbar 0, sel 0
                                    P: set_held_slot 1; container_set_slot slot 20 empty; slot 37 stone x3
survival, stone x3 in main 20, hotbar full of dirt, sel 2
                                    P: set_held_slot 2; container_set_slot slot 20 dirt x3; slot 38 stone x3
survival, stone x3 in main 20, stone x2 in hotbar 6
                                    P: set_held_slot 6; no slot changes
adventure, stone x3 in hotbar 4     P: set_held_slot 4
creative, empty, include data true  P: set_held_slot 0; container_set_slot slot 36 stone x1
a block 40 blocks away              P: nothing
```

**R19.** Pick block, choice of the slot. `E` is a `diamond_sword` with sharpness 1.

```text
creative, hotbar dirt except 2 and 7 empty, sel 5      set_held_slot 7; stone x1 in hotbar 7
creative, hotbar dirt, E in hotbar 3, sel 3            set_held_slot 4; dirt of hotbar 4 to main 9;
                                                       stone x1 in hotbar 4
creative, hotbar dirt, E in hotbar 3 to 5, sel 3       set_held_slot 6; dirt of hotbar 6 to main 9
creative, E in all hotbar slots, sel 3                 set_held_slot 3; E of hotbar 3 to main 9
creative, stone x5 in main 12, hotbar full of dirt, sel 3
                                                       set_held_slot 3; main 12 dirt x1; hotbar 3 stone x5
survival, stone x5 in main 12 and x7 in main 25, sword in hotbar 0, sel 0
                                                       set_held_slot 1; main 12 empty; hotbar 1 stone x5
survival, stone[custom_name='x'] x3 in hotbar 4        set_held_slot 0; no slot changes
creative, stone[custom_name='x'] x3 in hotbar 4        set_held_slot 0; stone x1 in hotbar 0
survival, hotbar dirt except 2 and 7 empty, stone x3 in main 20, sel 5
                                                       set_held_slot 7; main 20 empty; hotbar 7 stone x3
survival, empty hotbar, stone x3 in main 20, sel 5     set_held_slot 5; main 20 empty; hotbar 5 stone x3
```

**R20.** Pick block on stone at (8,-59,0). The distance is from the eye position to the face of the
block.

```text
survival  5.4: set_held_slot 0      5.6, 5.9, 6.1: nothing
creative  5.4, 5.6, 5.9: set_held_slot 0; stone x1 in hotbar 0      6.1: nothing
```

**R21.**

```text
chest with 2 diamond, creative, include data true:
  P: set_held_slot 0; container_set_slot slot 36 chest x1 with 1 added component
  Inventory: {components: {"minecraft:container": [{item: {count: 2, id: "minecraft:diamond"}, slot: 0}]},
              count: 1, Slot: 0b, id: "minecraft:chest"}
the same, include data false:   chest x1 with no components
survival, include data true, chest in main 9:
  P: set_held_slot 0; container_set_slot slot 9 empty; slot 36 chest x1 with no components
pig, creative:                  P: set_held_slot 0; container_set_slot slot 36 pig_spawn_egg x1
pig, survival, pig_spawn_egg x2 in main 20:
                                P: set_held_slot 0; container_set_slot slot 20 empty; slot 36 pig_spawn_egg x2
pig, survival, no egg:          P: set_held_slot 0; no slot changes
```

**R22.** The owner leaves at (0.5, -60, 0.5), with the yaw -90.

```text
item entities: iron_ingot x3, dirt x4, stone x2, each at x 1.875, y -60
join again:    container_set_content win 0 state 1, all slots empty, carried empty
36 diamond_sword in the main slots: the same item entities, iron_ingot x3 and dirt x4
```

**R23.** `container_close` with the window id 0, empty main slots.

```text
P: set_player_inventory slot 0 iron_ingot x3
   set_player_inventory slot 1 dirt x4
   container_set_slot win 0 state 42 slot 1 empty
   container_set_slot win 0 state 43 slot 36 iron_ingot x3
   container_set_slot win 0 state 44 slot 37 dirt x4
   set_cursor_item empty
no item entity
```

**R24.**

```text
P: disconnect; the reason ends with
   "Failed to decode packet 'serverbound/minecraft:set_creative_mode_slot'"
```

**R25.** `container_close` with the window id 0, in survival mode.

```text
36 diamond_sword in the main slots, dirt 4 in craft grid slot 1, iron_ingot 3 in the carried slot:
  P: add_entity item; add_entity item
     container_set_slot win 0 state 42 slot 1 empty
     set_cursor_item empty
  item entities: iron_ingot x3, dirt x4
35 diamond_sword, dirt 62 in main slot 35, dirt 4 in craft grid slot 1, dirt 3 in the carried slot:
  P: set_player_inventory slot 35 dirt x64
     add_entity item; add_entity item
     container_set_slot win 0 state 119 slot 1 empty
     container_set_slot win 0 state 120 slot 35 dirt x64
     set_cursor_item empty
  item entities: dirt x1 and dirt x4, which then merge to dirt x5
```

**R26.** Pick entity on a pig. The pig has no gravity and its bounding box contains the eye height.
`gap` is the distance from the eye position to the nearest point of the bounding box. The
diagonal pig is at the same gap along the line of yaw -45.

```text
survival, main slot 20 has pig_spawn_egg x2:
  gap 1.9 to 5.9, straight and diagonal: set_held_slot 0; slot 20 empty; slot 36 pig_spawn_egg x2
  gap 6.1 and 7.9, straight and diagonal: nothing
creative, empty inventory:
  gap 1.9 to 7.9, straight and diagonal: set_held_slot 0; slot 36 pig_spawn_egg x1
  gap 8.1 and 8.4, straight and diagonal: nothing
```

**R27.** Pick block on stone in creative mode, no match. Hotbar slot `n` has dirt `n + 1`, the
main slots 9 to 35 have a `diamond_sword`, the selected slot is 3.

```text
P: set_held_slot 3; container_set_slot slot 39 stone x1
Inventory after: hotbar 3 stone x1, no dirt x4 anywhere, main slots unchanged, no item entity
```

**R28.** Bulk pick, game version 26.4-snapshot-7. Of 1311 blocks, 1147 give the item with the name of
the block. The other blocks and the entities give the results of INV-PCK-008. A block or entity of
the last table row gives no packet. The full list is in the notes of the experiment, outside this
repository.

**R29.**

```text
include data false and true:
  P: set_held_slot 0; container_set_slot slot 36 decorated_pot x1 with 1 added component
  Inventory: {components: {"minecraft:pot_decorations": {}}, count: 1, Slot: 0b, id: "minecraft:decorated_pot"}
```

In R30 to R32, `slot` is a menu index of the inventory menu, `cursor` is `set_cursor_item`, and
`after` gives the slots of the `Inventory` field (0 to 8 hotbar, 9 to 35 main) and the item entities.
In R30, each click has the current state id, no claims and the empty carried stack, so the server
sends a correction for each slot that changed and for the carried slot. `diamond_sword` stacks that
fill the slots are not listed.

**R30.** Clicks, one case per setup.

```text
A take all:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  after: Inventory empty
B take half:
  slot 36 button 1 mode 0 -> slot 36 stone x5; cursor stone x6
  after: Inventory 0:stone x5
C place all:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 0 mode 0 -> slot 37 stone x10
  after: Inventory 1:stone x10
D place one:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 1 mode 0 -> slot 37 stone x1; cursor stone x9
  after: Inventory 1:stone x1
E merge:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 0 mode 0 -> slot 37 stone x64; cursor stone x6
  after: Inventory 1:stone x64
E2 merge right:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 1 mode 0 -> slot 37 stone x61; cursor stone x9
  after: Inventory 1:stone x61
F swap left:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 0 mode 0 -> slot 37 stone x10; cursor dirt x5
  after: Inventory 1:stone x10
G swap right:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 1 mode 0 -> slot 37 stone x10; cursor dirt x5
  after: Inventory 1:stone x10
H full target:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 0 mode 0 -> cursor stone x10
  slot 37 button 1 mode 0 -> cursor stone x10
  after: Inventory 1:stone x64
H2 take with right onto same:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 1 mode 0 -> slot 37 stone x8; cursor stone x9
  after: Inventory 1:stone x8
I drop all:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 0 mode 0 -> item entity
  after: Inventory empty; item entities stone x10
I2 drop one:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 1 mode 0 -> item entity; cursor stone x9
  after: Inventory empty; item entities stone x1
I3 outside with empty cursor:
  slot -999 button 0 mode 0 -> nothing
  after: Inventory 0:stone x10
J shift hotbar to main:
  slot 36 button 0 mode 1 -> slot 9 stone x10; slot 36 empty
  after: Inventory 9:stone x10
J2 shift main to hotbar:
  slot 9 button 0 mode 1 -> slot 9 empty; slot 36 stone x10
  after: Inventory 0:stone x10
J3 shift merge:
  slot 36 button 0 mode 1 -> slot 12 stone x64; slot 20 stone x36; slot 36 empty
  after: Inventory 12:stone x64, 20:stone x36
J4 shift helmet:
  slot 36 button 0 mode 1 -> slot 5 diamond_helmet x1; slot 36 empty
  after: Inventory empty
J5 shift shield:
  slot 36 button 0 mode 1 -> slot 36 empty; slot 45 shield x1
  after: Inventory empty
J6 shift from armor:
  slot 5 button 0 mode 1 -> slot 5 empty; slot 9 diamond_helmet x1
  after: Inventory 9:diamond_helmet x1
J7 shift from offhand:
  slot 45 button 0 mode 1 -> slot 9 stone x5; slot 45 empty
  after: Inventory 9:stone x5
J8 shift right button:
  slot 36 button 1 mode 1 -> slot 9 stone x10; slot 36 empty
  after: Inventory 9:stone x10
K swap hotbar 0:
  slot 9 button 0 mode 2 -> slot 9 stone x10; slot 36 dirt x5
  after: Inventory 0:dirt x5, 9:stone x10
K2 swap offhand:
  slot 9 button 40 mode 2 -> slot 9 stone x3; slot 45 dirt x5
  after: Inventory 9:stone x3
K3 swap empty hotbar:
  slot 9 button 4 mode 2 -> slot 9 empty; slot 40 dirt x5
  after: Inventory 4:dirt x5
K4 swap helmet slot with dirt:
  slot 5 button 0 mode 2 -> nothing
  after: Inventory 0:dirt x5
K5 swap button 9:
  slot 9 button 9 mode 2 -> nothing
  after: Inventory 9:dirt x5
L throw one:
  slot 36 button 0 mode 4 -> item entity; slot 36 stone x9
  after: Inventory 0:stone x9; item entities stone x1
L2 throw stack:
  slot 36 button 1 mode 4 -> item entity; slot 36 empty
  after: Inventory empty; item entities stone x10
L3 throw with cursor:
  slot 37 button 0 mode 0 -> slot 37 empty; cursor dirt x3
  slot 36 button 0 mode 4 -> cursor dirt x3
  after: Inventory 0:stone x10
M clone survival:
  slot 36 button 2 mode 3 -> nothing
  after: Inventory 0:stone x10
M2 clone creative:
  slot 36 button 2 mode 3 -> cursor stone x64
  after: Inventory 0:stone x10
N drag left:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 0 mode 5 -> cursor stone x10
  slot 37 button 1 mode 5 -> cursor stone x10
  slot 38 button 1 mode 5 -> cursor stone x10
  slot 39 button 1 mode 5 -> cursor stone x10
  slot -999 button 2 mode 5 -> slot 37 stone x3; slot 38 stone x3; slot 39 stone x3; cursor stone x1
  after: Inventory 1:stone x3, 2:stone x3, 3:stone x3
N2 drag right:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 4 mode 5 -> cursor stone x10
  slot 37 button 5 mode 5 -> cursor stone x10
  slot 38 button 5 mode 5 -> cursor stone x10
  slot 39 button 5 mode 5 -> cursor stone x10
  slot -999 button 6 mode 5 -> slot 37 stone x1; slot 38 stone x1; slot 39 stone x1; cursor stone x7
  after: Inventory 1:stone x1, 2:stone x1, 3:stone x1
N3 drag middle survival:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 8 mode 5 -> cursor stone x10
  slot 37 button 9 mode 5 -> cursor stone x10
  slot 38 button 9 mode 5 -> cursor stone x10
  slot -999 button 10 mode 5 -> cursor stone x10
  after: Inventory empty
N4 drag middle creative:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 8 mode 5 -> cursor stone x10
  slot 37 button 9 mode 5 -> cursor stone x10
  slot 38 button 9 mode 5 -> cursor stone x10
  slot -999 button 10 mode 5 -> slot 37 stone x64; slot 38 stone x64
  after: Inventory 1:stone x64, 2:stone x64
N5 drag onto filled:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 0 mode 5 -> cursor stone x10
  slot 37 button 1 mode 5 -> cursor stone x10
  slot 38 button 1 mode 5 -> cursor stone x10
  slot 39 button 1 mode 5 -> cursor stone x10
  slot -999 button 2 mode 5 -> slot 37 stone x5; slot 38 stone x64; cursor stone x1
  after: Inventory 1:stone x5, 2:stone x64, 3:dirt x1
N6 drag one slot:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 0 mode 5 -> cursor stone x10
  slot 37 button 1 mode 5 -> cursor stone x10
  slot -999 button 2 mode 5 -> slot 37 stone x10
  after: Inventory 1:stone x10
O collect:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 36 button 0 mode 6 -> slot 9 empty; slot 10 stone x60; slot 37 empty; cursor stone x64
  after: Inventory 10:stone x60
O2 collect other slot:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 0 mode 6 -> cursor stone x10
  after: Inventory 1:stone x20
P dirt to helmet:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor dirt x5
  slot 5 button 0 mode 0 -> cursor dirt x5
  after: Inventory empty
P2 dirt to offhand:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor dirt x5
  slot 45 button 0 mode 0 -> slot 45 dirt x5
  after: Inventory empty
P3 helmet to chest slot:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor diamond_helmet x1
  slot 6 button 0 mode 0 -> cursor diamond_helmet x1
  after: Inventory empty
P4 helmet to head:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor diamond_helmet x1
  slot 5 button 0 mode 0 -> slot 5 diamond_helmet x1
  after: Inventory empty
P5 pumpkin to head:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor carved_pumpkin x3
  slot 5 button 0 mode 0 -> slot 5 carved_pumpkin x1; cursor carved_pumpkin x2
  after: Inventory empty
P6 two helmets:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor diamond_helmet x1
  slot 5 button 0 mode 0 -> slot 5 diamond_helmet x1
  slot 37 button 0 mode 0 -> slot 37 empty; cursor diamond_helmet x1
  slot 5 button 0 mode 0 -> cursor diamond_helmet x1
  after: Inventory empty
Q craft take:
  slot 0 button 0 mode 0 -> slot 0 oak_planks x4; slot 1 oak_log x2; cursor oak_planks x4
  after: Inventory empty
Q2 craft take twice:
  slot 0 button 0 mode 0 -> slot 0 oak_planks x4; slot 1 oak_log x2; cursor oak_planks x4
  slot 0 button 0 mode 0 -> slot 0 oak_planks x4; slot 1 oak_log x1; cursor oak_planks x8
  after: Inventory empty
Q3 craft shift:
  slot 0 button 0 mode 1 -> slot 0 oak_planks x4; slot 0 oak_planks x4; slot 0 empty; slot 1 empty; slot 44 oak_planks x12
  after: Inventory 8:oak_planks x12
Q4 craft take onto other cursor:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor dirt x2
  slot 0 button 0 mode 0 -> cursor dirt x2
  after: Inventory empty
Q5 craft place into result:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor dirt x2
  slot 0 button 0 mode 0 -> cursor dirt x2
  after: Inventory empty
Q6 craft right take:
  slot 0 button 1 mode 0 -> slot 0 oak_planks x4; slot 1 oak_log x2; cursor oak_planks x4
  after: Inventory empty
Q7 craft throw:
  slot 0 button 0 mode 4 -> slot 0 oak_planks x4; item entity; slot 1 oak_log x2
  after: Inventory empty; item entities oak_planks x4
Q8 craft place log in grid:
  slot 36 button 1 mode 0 -> slot 36 oak_log x1; cursor oak_log x2
  slot 1 button 1 mode 0 -> slot 0 oak_planks x4; slot 1 oak_log x1; cursor oak_log x1
  after: Inventory 0:oak_log x1
```

**R31.** State id, claims and refused clicks. `server` is the state id of the server before the click.

```text
R1 stale state:
  slot 36 button 0 mode 0 [state 0 (server 3), claims 36=empty, carried stone x10] -> container_set_content state 4 slots all empty carried stone x10
  after: Inventory empty
R1b state +5:
  slot 36 button 0 mode 0 [state 11 (server 6), claims 36=empty, carried stone x10] -> container_set_content state 7 slots all empty carried stone x10
  after: Inventory empty
R2 exact prediction:
  slot 36 button 0 mode 0 [claims 36=empty, carried stone x10] -> nothing
  slot 37 button 0 mode 0 [claims 37=stone x10, carried empty] -> nothing
  after: Inventory 1:stone x10
R2b next server write after silent click:
  slot 36 button 0 mode 0 [claims 36=empty, carried stone x10] -> nothing
  after: Inventory empty
R2b, then item replace hotbar.5 with dirt -> slot 41 dirt x1 state 13
R3 false claim other slot:
  slot 36 button 0 mode 0 [claims 36=empty, 40=diamond x1, carried stone x10] -> slot 40 empty
  after: Inventory empty
R3b false claim only, no change:
  slot -999 button 0 mode 0 [claims 40=diamond x1, carried empty] -> slot 40 empty
  after: Inventory 0:stone x10
R4 refused click, client predicted success:
  slot 36 button 0 mode 0 [claims 36=empty, carried dirt x5] -> nothing
  slot 5 button 0 mode 0 [claims 5=dirt x5, carried empty] -> slot 5 empty; cursor dirt x5
  after: Inventory empty
R4b wrong count prediction:
  slot 36 button 1 mode 0 [claims 36=stone x6, carried stone x4] -> slot 36 stone x5; cursor stone x5
  after: Inventory 0:stone x5
R5 window 5:
  slot 36 button 0 mode 0 [window 5] -> nothing
  after: Inventory 0:stone x10
R5b window 1 no menu:
  slot 36 button 0 mode 0 [window 1] -> nothing
  after: Inventory 0:stone x10
R6 mode 0 button 5:
  slot 36 button 5 mode 0 -> nothing
  after: Inventory 0:stone x10
R6b mode 1 button 2:
  slot 36 button 2 mode 1 -> nothing
  after: Inventory 0:stone x10
R6c mode 4 button 2:
  slot 36 button 2 mode 4 -> item entity; slot 36 empty
  after: Inventory empty; item entities stone x10
R6d mode 6 button 1:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 36 button 1 mode 6 -> slot 37 empty; cursor stone x15
  after: Inventory empty
R6e drag without start:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot 37 button 1 mode 5 -> cursor stone x10
  slot -999 button 2 mode 5 -> cursor stone x10
  after: Inventory empty
R6f drag interrupted by click:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 0 mode 5 -> cursor stone x10
  slot 37 button 1 mode 5 -> cursor stone x10
  slot 38 button 0 mode 0 -> cursor stone x10
  slot -999 button 2 mode 5 -> cursor stone x10
  after: Inventory empty
R6g drag right slot twice:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 4 mode 5 -> cursor stone x10
  slot 37 button 5 mode 5 -> cursor stone x10
  slot 37 button 5 mode 5 -> cursor stone x10
  slot -999 button 6 mode 5 -> slot 37 stone x1; cursor stone x9
  after: Inventory 1:stone x1
R7 spectator take:
  slot 36 button 0 mode 0 -> container_set_content state 62 slots {36: stone x10} carried empty
  after: Inventory 0:stone x10
R7b spectator throw:
  slot 36 button 1 mode 4 -> container_set_content state 66 slots {36: stone x10} carried empty
  after: Inventory 0:stone x10
R7c adventure take:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  after: Inventory empty
R8 slot 46:
  slot 46 button 0 mode 0 -> nothing
  after: Inventory 0:stone x10
R8b slot -1:
  slot -1 button 0 mode 0 -> nothing
  after: Inventory 0:stone x10
R8c slot -1 mode 4:
  slot -1 button 0 mode 4 -> nothing
  after: Inventory 0:stone x10
R8d slot -999 mode 1:
  slot -999 button 0 mode 1 -> nothing
  after: Inventory 0:stone x10
R8e changed slot 60:
  slot 36 button 0 mode 0 [claims 60=stone x1, carried stone x10] -> slot 36 empty
  after: Inventory empty
R9 mode 7:
  slot 36 button 0 mode 7 -> slot 36 empty; cursor stone x10
  after: Inventory empty
R9b slot 200:
  slot 200 button 0 mode 0 -> nothing
  after: Inventory empty
R10 129 changed slots:
  slot -999 button 0 mode 0 [129 empty claims] -> disconnect
  after: Inventory empty
```

**R32.** Drag with few items, quick move with no room, and 128 claims.

```text
X1 left drag 2 over 3:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x2
  slot -999 button 0 mode 5 -> cursor stone x2
  slot 37 button 1 mode 5 -> cursor stone x2
  slot 38 button 1 mode 5 -> cursor stone x2
  slot 39 button 1 mode 5 -> cursor stone x2
  slot -999 button 2 mode 5 -> slot 37 stone x1; slot 38 stone x1
  after: Inventory 1:stone x1, 2:stone x1
X2 right drag 2 over 3:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x2
  slot -999 button 4 mode 5 -> cursor stone x2
  slot 37 button 5 mode 5 -> cursor stone x2
  slot 38 button 5 mode 5 -> cursor stone x2
  slot 39 button 5 mode 5 -> cursor stone x2
  slot -999 button 6 mode 5 -> slot 37 stone x1; slot 38 stone x1
  after: Inventory 1:stone x1, 2:stone x1
X5 left drag 10 over 4:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x10
  slot -999 button 0 mode 5 -> cursor stone x10
  slot 37 button 1 mode 5 -> cursor stone x10
  slot 38 button 1 mode 5 -> cursor stone x10
  slot 39 button 1 mode 5 -> cursor stone x10
  slot 40 button 1 mode 5 -> cursor stone x10
  slot -999 button 2 mode 5 -> slot 37 stone x2; slot 38 stone x2; slot 39 stone x2; slot 40 stone x2; cursor stone x2
  after: Inventory 1:stone x2, 2:stone x2, 3:stone x2, 4:stone x2
X6 left drag 3 over 4:
  slot 36 button 0 mode 0 -> slot 36 empty; cursor stone x3
  slot -999 button 0 mode 5 -> cursor stone x3
  slot 37 button 1 mode 5 -> cursor stone x3
  slot 38 button 1 mode 5 -> cursor stone x3
  slot 39 button 1 mode 5 -> cursor stone x3
  slot 40 button 1 mode 5 -> cursor stone x3
  slot -999 button 2 mode 5 -> slot 37 stone x1; slot 38 stone x1; slot 39 stone x1
  after: Inventory 1:stone x1, 2:stone x1, 3:stone x1
X3 shift no room:
  slot 36 button 0 mode 1 -> nothing
  after: Inventory 0:stone x10
X7 shift partial room:
  slot 36 button 0 mode 1 -> slot 35 stone x64; slot 36 stone x6
  after: Inventory 0:stone x6, 35:stone x64
X4 128 changed:
  slot -999 button 0 mode 0 [128 empty claims] -> nothing
  after: Inventory empty
```

**R33.** Pick reach at the limit and with a changed attribute. For a block, the distance is from the
eye position to the face of the block. For an entity, the gap is as in R26.

```text
block, default attribute:      survival 5.4 reply; 5.5, 5.6 nothing
                               creative 5.9 reply; 6.0, 6.1 nothing
block, attribute base 8:       survival 8.9 reply; 9.0, 9.1 nothing
                               creative 9.4 reply; 9.5, 9.6 nothing
entity, attribute base 5:      survival 7.9 reply; 8.1 nothing
                               creative 9.9 reply; 10.1 nothing
entity, default attribute:     survival 6.0 nothing; creative 8.0 nothing
```
