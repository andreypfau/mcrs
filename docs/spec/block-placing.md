# Block placing: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. The captured results of section 13 come from
`26.4-snapshot-4`.

This document states how a player places a block in creative mode, and what the server sends for it. It
states only what a client, the server console and the server files show. It does not prescribe an
architecture. Most rules come from a vanilla server in offline mode, with scripted clients. A rule with a
`source_suggested` label comes from the reference implementation. Section 12 gives
the experiments, and section 13 gives the captured results.

Scope: creative mode, and the refusals that other game modes cause. This document does not state the
inventory (`inventory.md`), a full table of block states or the use of an item that is
not a block item.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**, **VarInt**, **offline
  mode**: as `login.md`, section 0.1, defines them.
- **Block cube**, **block corner**, **eye position**: as `block-breaking.md`, section 0.1, defines them.
- **Placer**: the player whose client sends the `use_item_on` packet.
- **Observer**: another player connected to the same dimension.
- **Clicked position**: the position field of the `use_item_on` packet.
- **Face**: the face field of the `use_item_on` packet.
- **Cursor**: the three float fields of the `use_item_on` packet. They are relative to the block corner
  of the clicked position.
- **Adjacent position**: the clicked position, moved one block in the direction of the face.
- **Target position**: the position that receives the new block (PLC-TGT-001).
- **Held item**: the item in the hand that the packet names (PLC-INV-001).
- **Block item**: an item that has a block to place.
- **Sneak**: the placer has sent `player_input` with the bit 32 set.
- **Sequence**: the last field of the `use_item_on` packet.
- **Acknowledgement**: one clientbound `block_changed_ack` packet.
- **Refresh**: one `block_update` for the clicked position and then one `block_update` for the adjacent
  position. Each packet carries the current state of its position.
- **Open teleport**: a teleport that the client has not accepted.
- **Placement**: the server changes the block at the target position.
- **Refusal**: the server does not change a block.
- **Replaceable block**: a block that a placement can overwrite (PLC-TGT-002).
- **Operator**: a player in the operator list of the server. Its permission level is 4 by default.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 13.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent with a
  captured result in section 13.
- `source_suggested`: taken from the reference implementation, with no captured result. Section 12
  attaches an experiment.
- `unresolved`: not decided. Section 11 collects these items, with an experiment for each item.

All the results come from one vanilla server and scripted clients on one machine. A rule that rests on
one observation states this.

### 0.3 ID prefix

Each rule has an ID with the prefix `PLC`.

## 1. The request

**PLC-REQ-001** (`black_box_confirmed`, wire). The `use_item_on` packet has these fields, in this order:
hand (VarInt), position (long), face (VarInt), cursor x, y and z (three floats), inside (boolean), world
border hit (boolean), sequence (VarInt). See result R1.

**PLC-REQ-002** (`black_box_confirmed`, wire). The face values are 0 down, 1 up, 2 north, 3 south, 4 west
and 5 east. See result R2.

**PLC-REQ-003** (`black_box_confirmed`, semantic). The values of inside and world border hit had no
effect in each tested case. See result R3. The tested cases are stone on stone, with each pair of values.

**PLC-REQ-004** (`black_box_confirmed`, semantic). A packet that does not decode closes the connection.
See result R4.

The reference implementation also accepts hand and face values that the vanilla client does not send.
This document leaves their meaning unspecified. Result R4 records them.

**PLC-REQ-005** (`black_box_confirmed`, semantic). The server ignores a request until the client has
loaded. There is no acknowledgement, no block state and no placement. The client has loaded after it
sends `player_loaded`, or about 60 ticks after the player joins. See result R5. This agrees with
BRK-REQ-005.

**PLC-REQ-006** (`source_suggested`, semantic). The server ignores a request when the held item is not
enabled by the enabled features of the world. The placer gets one acknowledgement and nothing else.
Experiment E49.

## 2. A placement

**PLC-PLA-001** (`black_box_confirmed`, semantic). The placer gets, in this order:

1. a refresh;
2. the new state of each changed block;
3. one acknowledgement.

See result R1. The state of step 2 repeats a packet of step 1 when the changed block is at a position of
the refresh. Only the content of the packets is required. A parity test must not count this repeated
packet (kind: unspecified).

**PLC-PLA-002** (`black_box_confirmed`, wire). The server sends `block_update` when one block changes in
a section. It sends `section_blocks_update` when more than one block in a section changes together. See
results R1, R14 and R18.

**PLC-PLA-003** (`black_box_confirmed`, semantic). For one placement, an observer gets, in this order:
`sound`, `swing_animation`, the new block state. An observer does not get an acknowledgement or a
refresh. See result R1. For several placements in one tick, the observer gets each `sound`, then the
block states, then each `swing_animation`. See result R39. This order rests on one observation.

**PLC-PLA-004** (`black_box_confirmed`, semantic). The placer does not get `sound` or `swing_animation`
for a placement. See result R1.

**PLC-PLA-005** (`source_suggested_black_box_confirmed`, wire). The `sound` packet has the place sound
of the placed block, the category value 4, and the centre of the target position as its position. See
results R1 and R6. A block with two parts uses the target position, and not the other part. The sound set
of the placed state gives the sound. The block definition names the sound set
(`mcrs:block_sound_set`), and the `block_sound_set` registry of the data pack holds it. The volume is
`(v + 1) / 2` and the pitch is `p * 0.8`, where `v` and `p` are the volume and the pitch of the sound set.
Each has the default 1.0. A sound set with no place sound sends no `sound`.

**PLC-PLA-006** (`source_suggested_black_box_confirmed`, semantic). An observer gets the `sound` when
the distance from its position to the sound position is less than the range of the sound. The range is
16, or 16 times the volume when the volume is above 1. A sound event with a fixed range uses that range.
A distance of 15.9 gets it and 16.1 does not. See result R7. An observer gets the new block state while
its client holds the chunk.

**PLC-PLA-007** (`source_suggested_black_box_confirmed`, wire). The `swing_animation` packet names the
entity of the placer, the hand (0 for the main hand, and 1 for the off hand), and the swing animation of
the held item. The swing animation is the type and the duration of the `minecraft:swing_animation`
component. The default is the type 1 (`whack`) and the duration 6. See results R1 and R8. The reference
implementation sends the packet two times. A parity test must not count these packets (kind:
unspecified). An observer that does not track the placer does not get it. See result R7.

**PLC-PLA-008** (`black_box_confirmed`, semantic). In creative mode, the held item does not change. The
placer gets no slot packet, and an observer gets no `set_equipment`. See result R9.

**PLC-PLA-009** (`black_box_confirmed`, semantic). A block entity whose client data is not empty sends
`block_entity_data` after the block state and before the acknowledgement. The tested blocks are a sign, a
banner, a skull, a spawner, a structure block, a jigsaw block and a test block. A chest, a hopper and a
bed send no such packet. Each player whose client holds the chunk gets it. See result R10.

**PLC-PLA-010** (`black_box_confirmed`, semantic). A sign and a hanging sign send `open_sign_editor` to
the placer. The placer gets one more `block_update` for the sign before this packet. See result R10.

**PLC-PLA-011** (`black_box_confirmed`, semantic). The server handles a request between two ticks, as
BRK-BRK-007 states. The acknowledgement comes with the block states. The largest delay in result R11 is
45.3 ms.

**PLC-PLA-012** (`black_box_confirmed`, semantic). A later result of the placement comes after the
acknowledgement. A fluid that flows and a block that falls are the tested cases. A change in the same
tick comes before it, in the same packet where the section is the same. See result R12.
`block-simulation.md` states how neighbours update.

## 3. The target position

**PLC-TGT-001** (`black_box_confirmed`, semantic). The target position is the clicked position when the
block there is replaceable. Otherwise the target position is the adjacent position. The block at the
target position must also be replaceable, or the placement is refused. See results R13 and R15.

The clicked position does not need support: a request on air places the block at that air position. See
result R13. A later rule can refuse the placement. The vanilla client does not send this request in
normal use.

**PLC-TGT-002** (`source_suggested_black_box_confirmed`, semantic). A block is replaceable when its
block definition has the `minecraft:replaceable` component, with these conditions. See result R13. The
block tag `minecraft:replaceable` has the same members in this version, but the server does not read the
tag. A data pack that changes the tag does not change this set. Experiment E50.

- Snow is replaceable only with `layers=1` when the held item is not snow.
- With snow in hand, snow with fewer than 8 layers is replaceable at the adjacent position. At the
  clicked position, it is replaceable only with the face up. PLC-TGT-003 states the merge. This item is
  `source_suggested_black_box_confirmed`, from result R16.
- Water and lava are replaceable at each tested level: 0 and 4 for water, 0 and 2 for lava.
- Light is replaceable only as the adjacent position (PLC-INT-003).

Result R13 tests these members: air, cave air, short grass, fern, dead bush, bush, short dry grass,
water, lava, fire, vine, light, structure void, seagrass, crimson roots, tall grass, leaf
litter with one segment and bubble column. These blocks are not replaceable in result R13: snow with more
than one layer, dandelion, torch, oak sapling, barrier, kelp, nether portal, cobweb, bedrock and stone.

**PLC-TGT-003** (`black_box_confirmed`, semantic). When a block merges with the block in hand, the block
at the clicked position or the adjacent position counts as replaceable. A merge changes the block, and it
is a placement. See result R16.

| Held item | Block | Face | Result |
|-----------|-------|------|--------|
| same slab | bottom slab | up | double slab |
| same slab | bottom slab | side, cursor y 0.7 | double slab |
| same slab | bottom slab | side, cursor y 0.3 | new bottom slab at the adjacent position |
| same slab | bottom slab | down | new top slab at the adjacent position |
| same slab | top slab | down | double slab |
| same slab | top slab | up | new bottom slab at the adjacent position |
| same slab | a slab at the adjacent position, either half | up | double slab |
| other slab | bottom slab | up | new slab at the adjacent position |
| same slab | double slab | up | new slab at the adjacent position |
| snow | snow, 1 or 7 layers | up | one more layer |
| snow | snow, 1 layer | side | new snow at the adjacent position |
| candle | candle, 1 | up | one more candle |
| candle | candle, 4 | up | refresh only |
| sea pickle | sea pickle | up | one more pickle |
| turtle egg | turtle egg | up | one more egg |

**PLC-TGT-004** (`black_box_confirmed`, semantic). A placement on one part of a tall plant removes the
other part. No item drops. The placer and each observer get `level_event` with type 2001 and the old
state of the other part, before the other packets. See result R17.

**PLC-TGT-005** (`black_box_confirmed`, semantic). A placement on a fluid removes the fluid, unless the
placed state holds water (PLC-STA-005). See result R12.

**PLC-TGT-006** (`source_suggested`, semantic). A block of PLC-TGT-002 is not replaceable when the held
item is the item of that block. A fern in hand on a fern places the fern at the adjacent position. The
blocks of PLC-TGT-003 and of PLC-TGT-007 have their own rule. Experiment E51.

**PLC-TGT-007** (`source_suggested_black_box_confirmed`, semantic). A glow lichen and a sculk vein are
replaceable when the held item is another item. A sculk vein has no `minecraft:replaceable` component,
but this rule applies to it too. With their own item in hand, they are replaceable only while a face can
still take a surface. See result R13. Experiment E51.

## 4. Refusals

### 4.1 Refusals that send only an acknowledgement

**PLC-RCH-001** (`black_box_confirmed`, semantic). A request is refused when the distance from the eye
position to the nearest point of the block cube at the clicked position is 6.0 or more. A distance less
than 6.0 is in reach. The placer gets one acknowledgement and no block state. An observer gets nothing.
See result R19. BRK-RCH-001 states the source of the limit. The face has no effect on the result, so a
target position further than 6.0 can receive a block.

**PLC-CUR-001** (`black_box_confirmed`, semantic). A request is refused when a cursor value is below -0.5
or above 1.5. A value that is not a number also refuses it. The placer gets one acknowledgement and no
block state. An observer gets nothing. See result R20. The limits hold to 0.00001: -0.5 and 1.5 pass, and
-0.50001 and 1.50001 do not.

**PLC-CUR-002** (`source_suggested`, semantic). In the refusal of PLC-CUR-001, the server console gets
one warning line. The line names the player, the cursor location and the clicked position. Experiment
E52.

### 4.2 The height limits

**PLC-HGT-001** (`source_suggested_black_box_confirmed`, wire). A clicked position above the height
limit of the dimension is refused. The placer gets `system_chat` in the overlay position, and then one
acknowledgement. The message is the translation key `build.tooHigh` in red, with the highest block `y` of
the dimension as the argument. A clicked position below the height limit does the same with the key
`build.tooLow` and the lowest block `y`. In the overworld, the arguments are 319 and -64. See result R21.

**PLC-HGT-002** (`source_suggested_black_box_confirmed`, wire). A clicked position in the world with an
adjacent position outside the height limits is refused when the block at the clicked position is not
replaceable. The placer gets a refresh and one acknowledgement. The state of the adjacent position in the
refresh is `void_air`. See result R21.

**PLC-HGT-004** (`source_suggested_black_box_confirmed`, wire). The placer also gets the message of
PLC-HGT-001 before the refresh when all these conditions are true. See result R21. Experiment E53.

- The face is up and the clicked `y` is at the upper height limit or above it.
- Or the face is down and the clicked `y` is at the lower height limit or below it.
- The request places no block and the clicked block does not act.
- The held item is a block item or a bucket that holds a fluid, and the item has no cooldown.

The reason of the refusal has no effect. A side face sends no message. Section 10 (DEV-2) states a
difference of the reference.

**PLC-HGT-003** (`black_box_confirmed`, semantic). A block with two parts that would pass the height
limit is refused with a refresh and one acknowledgement, and with no message. See result R21. This rule
rests on one observation: a door with its lower part at `y=319`.

### 4.3 Spawn protection

**PLC-SPN-001** (`black_box_confirmed`, semantic). The conditions of BRK-SPN-001 refuse a request, with
the clicked position as the block position. The target position has no effect. See result R22. In results
R22 the `x` axis, the `z` axis and a diagonal agree with the rule.

**PLC-SPN-002** (`black_box_confirmed`, wire). In this refusal, the placer gets `system_chat` in the
overlay position, a refresh, and one acknowledgement. The message is the translation key
`build.spawn_protection` in red, with the argument `x, y, z` of the clicked position. An observer gets
nothing. See result R22.

**PLC-SPN-003** (`black_box_confirmed`, semantic). This refusal also stops the use of the clicked block
(section 5). A lever stays as it is and a chest does not open. See result R22.

### 4.4 The world border and an open teleport

**PLC-BRD-001** (`black_box_confirmed`, semantic). A request is refused when the block corner of the
clicked position is outside the world border. The block corner is outside when `x` or `z` is below the
lower limit of the border, or is at or above the upper limit. The placer gets a refresh and one
acknowledgement. An observer gets nothing. See result R23. An operator is also subject to this rule.
Section 10 (DEV-1) states a difference of the reference.

**PLC-BRD-002** (`black_box_confirmed`, semantic). The target position can be outside the border when the
clicked position is inside it. The block is placed. See result R23.

**PLC-BRD-003** (`black_box_confirmed`, semantic). A bed with its head outside the border is refused with
a refresh and one acknowledgement. See result R23. This rule rests on one observation.

**PLC-TEL-001** (`black_box_confirmed`, semantic). A request is refused while an open teleport exists
(`teleport-confirmation.md`). The placer gets a refresh and one acknowledgement. An observer gets
nothing. See result R24. Section 10 (DEV-1) states a difference of the reference.

### 4.5 Refusals that send a refresh

**PLC-REF-001** (`black_box_confirmed`, semantic). In each case of this table, the placer gets a refresh
and then one acknowledgement. An observer gets nothing.

| Case | Result |
|------|--------|
| The hand is empty, or the held item is not a block item and does not act on the clicked block (section 6, result R35) | R25 |
| The target position does not hold a replaceable block (PLC-TGT-001) | R15 |
| The player is in spectator mode | R26 |
| The player is in adventure mode, and PLC-INV-003 does not allow the placement | R26 |
| The block needs support, and the position does not give it (PLC-SUP-001) | R27 |
| The placed block would overlap an entity (PLC-ENT-001) | R28, for a pig and a player |
| The block is one of PLC-REF-002, and the player is not in creative mode with permission level 2 or more | R10 |
| The other part of the block cannot go in its position (section 8) | R14 |

**PLC-REF-002** (`source_suggested_black_box_confirmed`, semantic). A command block, a chain command
block, a repeating command block, a structure block, a jigsaw block, a test block and a test instance
block need a player in creative mode with the permission level 2 or more. Any other player cannot place
them. See result R10. A barrier, a light, a structure void, a spawner and bedrock have no such limit.
Experiment E44.

**PLC-SUP-001** (`black_box_confirmed`, semantic). Some blocks need support from a neighbour. Result R27
lists the tested pairs. The rule of each block is block data, and section 11 (U2) states what remains
unknown.

**PLC-ENT-001** (`source_suggested`, semantic). A placement is refused when the collision shape of the
placed state overlaps the bounding box of a blocker. Contact alone is not an overlap. A spectator is never
a blocker. These entities are blockers:

- each living entity, for example a player, a pig, a villager, a zombie, a shulker and an armor stand
  that is not a marker;
- primed TNT, an end crystal and a falling block;
- each minecart and each boat.

The other entities are not blockers. Examples are an armor stand that is a marker, an item, an arrow, an
experience orb, a snowball, an item frame, an interaction, a marker, a block display, a leash knot and an
area effect cloud. Results R28 and R29 agree, except for the armor stand, the minecarts, the boats,
the end crystal and the falling block. Section 11 (U13) states this conflict. Experiment E57.

**PLC-ENT-002** (`black_box_confirmed`, semantic). The placer is also a blocker. See result R29. The box
of a player that stands is 0.6 wide and 1.8 high, with the position at the centre of its bottom.

**PLC-ENT-003** (`black_box_confirmed`, semantic). A block with no collision shape has no such refusal. A
torch, a sapling, a rail and a tall plant do not hit a blocker. See results R28 and R29. A bottom slab
under a blocker is placed when the box does not reach into the slab. A top slab with the same blocker is
refused.

**PLC-ENT-004** (`black_box_confirmed`, semantic). Only the part at the target position counts. A pig in
the upper part of a door or in the head of a bed does not refuse the placement. See result R30.

**PLC-ENT-005** (`black_box_confirmed`, semantic). A mob with no health stays a blocker until the server
removes it. See result R30. This rule rests on one observation.

### 4.6 The order of the checks

**PLC-ORD-001** (`source_suggested_black_box_confirmed`, semantic). The server applies the checks in this
order. The first check that refuses the request sets the result.

1. The enabled features of the held item (PLC-REQ-006).
2. Reach (PLC-RCH-001).
3. The cursor (PLC-CUR-001).
4. The height limits of the clicked position (PLC-HGT-001).
5. Spawn protection (PLC-SPN-001).
6. The open teleport (PLC-TEL-001) and the world border (PLC-BRD-001).
7. The use of the clicked block (section 5). A clicked block that the enabled features do not enable
   refuses the request here.
8. The held item: an empty hand, or an item with a cooldown, refuses the request.
9. An item that is not a block item, or a block that the enabled features do not enable.
10. The target position (PLC-TGT-001).
11. The operator blocks (PLC-REF-002).
12. The state for the placement, which includes the other part (section 8) and the face (section 7).
13. Support (PLC-SUP-001).
14. Entities in the way (PLC-ENT-001).
15. The height limits of the target position (PLC-HGT-002).

PLC-HGT-004 is not a check. Its message comes after all the steps.

See results R20, R21, R22, R23, R24 and R31. Steps 2 and 3 send the same packets, so a client cannot see
their order. The same is true of the two checks in step 6, except for the message of DEV-1, and of steps
8 to 14. Experiment E45.

## 5. The use of the clicked block

**PLC-INT-001** (`black_box_confirmed`, semantic). The use of the clicked block comes before a placement.
A chest, a barrel, a furnace, a crafting table, a lever, a button, a door, a trapdoor, a note block, a
sign and a cake act on the request with a block item in hand, and no block is placed. See result R31.
This document does not state what each block does.

**PLC-INT-002** (`source_suggested_black_box_confirmed`, semantic). When the placer sneaks and either hand
holds an item, the clicked block does not act, and the placement goes on. The hand of the request has no
effect: with sneak, an empty main hand and an item in the off hand, the clicked block does not act. With
sneak and two empty hands, the clicked block acts. See result R31. Experiment E54.

**PLC-INT-003** (`black_box_confirmed`, semantic). A light at the clicked position acts for an operator,
and its level changes. For another player, the result is a refresh and one acknowledgement. See result
R32.

**PLC-INT-004** (`black_box_confirmed`, semantic). When the clicked block acts, the placer gets a refresh
and one acknowledgement. Other packets of the block can come before or after the refresh. An observer
gets `swing_animation`. See result R31.

**PLC-INT-005** (`source_suggested_black_box_confirmed`, semantic). In spectator mode, a block with a
screen opens it and no block changes. A chest is the tested case. See result R26. On a portal block, the
spectator goes to the destination of the portal, or gets `spectator.cannot_teleport` in red in the overlay
position when the portal has no destination. Experiment E55.

**PLC-INT-006** (`source_suggested`, semantic). A block that acts without the held item acts only for the
hand value 0. With the hand value 1, a chest does not open, and the placement goes on. A block that acts
on the held item, such as a cake with a candle, acts for each hand. Experiment E54.

## 6. The held item

This section depends on the player inventory. Section 11 (U7) states this dependency.

**PLC-INV-001** (`black_box_confirmed`, semantic). The hand value 0 selects the selected hotbar slot, and
1 selects the off hand. The other hand has no effect on the request: a stick in the main hand does not
stop a block in the off hand, and an empty main hand does not fall back to the off hand. See results R8
and R33.

**PLC-INV-002** (`black_box_confirmed`, semantic). The server handles `set_creative_mode_slot`,
`set_carried_item` and `use_item_on` in the order of arrival, in one burst too. The slot 36 is the first
hotbar slot, and 45 is the off hand. A later request uses the new item. An empty stack in the selected
slot gives PLC-REF-001. See result R33.

**PLC-INV-003** (`black_box_confirmed`, semantic). In adventure mode, the `minecraft:can_place_on`
component must match the block at the clicked position, or the placement is refused. Creative mode
ignores this component. See result R26.

**PLC-INV-004** (`black_box_confirmed`, semantic). The `minecraft:block_state` component sets the named
properties of the placed state. A value that the block does not have is ignored, and the placement uses
the normal state. See result R34.

**PLC-INV-005** (`black_box_confirmed`, semantic). The `minecraft:custom_name` and `minecraft:container`
components go to the block entity of the placed block. See result R34.

**PLC-INV-006** (`black_box_confirmed`, semantic). Outside creative mode, the count of the stack falls by
one. The placer gets `container_set_slot` before the acknowledgement. An observer gets `set_equipment`
after the block state. See result R9. This document states only this observation. A later specification
of the use of an item owns this rule.

**PLC-INV-007** (`black_box_confirmed`, semantic). The block of an item can have another name. `redstone`
places `redstone_wire`, and `string` places `tripwire`. See results R27 and R35.

**PLC-INV-008** (`source_suggested`, semantic). The `minecraft:block_entity_data` component goes to the
block entity of the placed block when its `id` is the type of that block entity. For a command block, a
lectern, a sign, a hanging sign, a spawner and a trial spawner, the player must also be in creative mode
with the permission level 2 or more. After this, the other components of the item go to the block entity,
so the default `minecraft:container` component of a chest replaces `Items`. See result R34. Experiment
E44.

**PLC-INV-009** (`source_suggested`, semantic). When a placement is refused and the held block item has
the `minecraft:consumable` component, the server starts the use of the item, as the use of the item in
the air does. Experiment E56.

An item such as a flint and steel, an armor stand, a spawn egg or a painting also acts through this
packet. It is not a block item, and this document does not state it. See result R35.

## 7. The state of the placed block

This section gives only the tested families. Section 11 (U3) states what remains unknown. The direction
words are the direction in which the placer looks. The rules and the rows that result R36 shows are
`black_box_confirmed`, kind semantic. The other rows of PLC-STA-004, and the pane and the redstone wire of
PLC-STA-006, are `unresolved`. Section 11 (U3) holds them.

**PLC-STA-001**. Direction from the yaw of the placer.

| Result | Tested blocks |
|--------|---------------|
| `facing` is the direction of the look | stairs, door, bed, fence gate |
| `facing` is the opposite direction | furnace, chest, trapdoor, carved pumpkin, repeater, glazed terracotta, lectern |
| `facing` is the direction after one quarter turn clockwise (south gives west) | anvil |

**PLC-STA-002**. Direction from the pitch of the placer. A pitch below -45 is up and a pitch above 45 is
down. Otherwise the direction is the horizontal direction of the look. Result R36 tests these limits only
at yaw -90.

| Result | Tested blocks |
|--------|---------------|
| `facing` is the opposite of the direction | piston, dispenser |
| `facing` is the direction | observer |

**PLC-STA-003**. The sixteen rotation values come from the yaw. A sign and a banner use
`floor((yaw + 180) * 16 / 360 + 0.5) mod 16`. A skull uses `floor(yaw * 16 / 360 + 0.5) mod 16`.

**PLC-STA-004**. The state from the face and the cursor.

| Block | Face up | Face down | Side face |
|-------|---------|-----------|-----------|
| Slab, stairs | bottom half | top half | top half when cursor y is above 0.5, otherwise bottom half |
| Log, iron chain | `axis=y` | `axis=y` | the axis of the face |
| End rod, amethyst cluster | `facing=up` | `facing=down` | `facing` is the face |
| Lever, button | `face=floor`, `facing` is the look | `face=ceiling`, `facing` is the look | `face=wall`, `facing` is the face |
| Torch | `torch` | refused | `wall_torch`, `facing` is the face |
| Sign, banner | floor form | refused | wall form, `facing` is the face |
| Skull | floor form | floor form | wall form, `facing` is the face |
| Hopper | `facing=down` | `facing=down` | `facing` is the opposite of the face |
| Ladder, tripwire hook | refused | refused | `facing` is the face |
| Lantern | `hanging=false` | `hanging=true` | refused |
| Bell | `attachment=floor`, `facing` is the look | `attachment=ceiling`, `facing` is the look | `attachment=single_wall`, `facing` is the opposite of the face |
| Vine, glow lichen | see R36 | the surface is on the side of the clicked block | the surface is on the side of the clicked block |
| Pressure plate, rail, redstone, scaffolding | placed | refused | refused |

**PLC-STA-005**. A state with a `waterlogged` property holds water when the block at the target position
is a water source. Water at a level that is not a source, and lava, give `waterlogged=false`. A block
with no such property removes the water. A double slab has `waterlogged=false`, also when it comes from a
slab that holds water.

**PLC-STA-006**. A connected block takes its connections in the placement, and the neighbour updates in
the same tick. A fence, a pane and redstone wire are the tested cases. The placer and the observer get
both changes in one `section_blocks_update`. Stairs take their shape from a neighbour, and the neighbour
stays.

**PLC-STA-007**. A door or a trapdoor next to a powered block has `powered=true` and `open=true`.

## 8. Blocks that have more than one part

**PLC-PRT-001** (`black_box_confirmed`, semantic). A door, a bed and a tall plant place both parts in one
placement. There is one `sound`, at the target position. Both changes reach each player in one
`section_blocks_update` when they are in the same section. Otherwise each change has its own
`block_update`. See results R14 and R18. The order of the two parts in the packet is unspecified.

**PLC-PRT-002** (`black_box_confirmed`, semantic). The target position takes the lower part of a door or
a tall plant, and the foot of a bed. The other part goes above it, or in the direction of the look for a
bed. See results R14 and R36.

**PLC-PRT-003** (`black_box_confirmed`, semantic). The position of the other part must hold a replaceable
block. Otherwise the result is a refresh and one acknowledgement. See result R14.

**PLC-PRT-004** (`black_box_confirmed`, semantic). Support, as result R14 shows:

| Block | Tested support |
|-------|----------------|
| Door | Stone, dirt, clay and glass pass. A bottom slab and no block refuse. |
| Tall grass, sunflower, pitcher plant | Dirt passes. Stone and clay refuse. |
| Small dripleaf | Clay passes. Stone and dirt refuse. |
| Bed | No support is necessary, at the foot or at the head. |

**PLC-PRT-005** (`black_box_confirmed`, semantic). The hinge of a door comes from its neighbours and the
cursor. See result R37. Result R37 tests only a door that faces south. Section 11 (U3) states what
remains unknown.

**PLC-PRT-006** (`black_box_confirmed`, semantic). A chest makes a pair with a neighbour chest of the
same `facing`. Both chests change, in one `section_blocks_update`: the new chest has `type=left` and the
old chest has `type=right`, for the tested placement. A chest with another `facing` stays `single`. When
the placer sneaks, the new chest stays `single`, unless the clicked block is the neighbour chest. See
result R38.

## 9. Several requests and the sequence

**PLC-SEQ-001** (`black_box_confirmed`, semantic). Requests that the server handles between two ticks get
one acknowledgement. It carries the highest sequence of those requests, whichever came last. See result
R39. A refused request counts too.

**PLC-SEQ-002** (`black_box_confirmed`, semantic). Each request gets its own refresh. Each placement gets
its own `sound` and `swing_animation` for an observer. Block states that change in one tick use
PLC-PLA-002. See result R39.

**PLC-SEQ-003** (`black_box_confirmed`, semantic). Requests about 200 ms apart each get their own
acknowledgement. See result R39.

**PLC-SEQ-004** (`black_box_confirmed`, semantic). A break and a placement in one tick share one
acknowledgement, with the highest sequence. See result R39. This rule rests on one observation.

**PLC-SEQ-005** (`black_box_confirmed`, wire). The sequences 0, 1, 2147483646 and 2147483647 are valid.
See result R39.

**PLC-SEQ-006** (`source_suggested_black_box_confirmed`, semantic). A negative sequence closes the
connection with the translation key `disconnect.packetError`. The block is not placed, because the server
checks the sequence before it handles the request. See result R4. A break handles the request before it
checks the sequence, so BRK-SEQ-005 breaks the block. Experiment E46.

## 10. Intentional deviations

**DEV-1.**

- **Rule**: PLC-BRD-001, PLC-TEL-001.
- **Reference**: the placer also gets `system_chat` in the overlay position, with `build.tooHigh` and the
  argument 319, at any height.
- **Visible to the client**: yes. The player sees a height message that is not true.
- **Test**: a differential test ignores this one `system_chat`.

**DEV-2.**

- **Rule**: PLC-HGT-004.
- **Reference**: at the upper height limit with the face up, the placer gets the message two times. In
  each other case of PLC-HGT-004, the placer gets it one time.
- **Visible to the client**: no. The second message replaces the first in the overlay position.
- **Test**: a differential test counts one message.

## 11. Unresolved

Each item names the experiment that decides it. A closed item keeps its number, and the number is not
used again. The same applies to experiments.

- **U2**: the support rule of each block. Experiment E41.
- **U3**: the state of each block family, the hinge of a door with another `facing`, and the block
  families that this document does not test. This includes the rows of PLC-STA-004 that result R36 does
  not show. A block that attaches to a neighbour, such as a lantern or a torch, can try other directions
  in the order of the look when the clicked face does not hold it. So a refusal in PLC-STA-004 can
  depend on the neighbours. Experiment E42.
- **U4**: `pale_hanging_moss`. Result R13 replaced it, but the block definition has no
  `minecraft:replaceable` component, and the reference does not replace it. The moss had no support in the
  experiment, and it possibly broke before the request. Experiment E43.
- **U7**: the player inventory (issue 521). Each placement needs the held item, and so do sections 3, 5,
  6, 7 and 8. Without it, a server
  can state only the refusals of sections 4.1 to 4.4, the refresh and the acknowledgement of section 9.
  `inventory.md` states what `set_creative_mode_slot` accepts (INV-CRE-003, INV-CRE-004). This document
  tests only the slots 36 and 45, and a stack with no components.
- **U9**: the box of a player that sneaks, swims or has another pose. All experiments used a player that
  stands. Experiment E47.
- **U10**: can_place_on when the clicked block is replaceable. Result R26 tests only stone. Experiment
  E48.
- **U11**: a request at a position in reach in a chunk that is not loaded.
- **U12**: a `moving_piston` at the clicked position changes to air in the one tested case. This looks
  like an accident, and this document does not state it.
- **U13**: result R28 places a block on an armor stand, a minecart, a boat, an end crystal and a falling
  block. The reference refuses these placements (PLC-ENT-001). Experiment E57.

## 12. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`, `level-type=minecraft:flat` and `network-compression-threshold=-1`. Use
`spawn-protection=0`, except where a step states another value. Set the game rule `spawn_mobs` to false.
Connect one scripted client as the placer and one as the observer, both in creative mode. Put the placer
at `(0.5, -60, 0.5)` with yaw 0, and the observer at `(8.5, -60, 8.5)`, except where a step states
another position. Give each client the item with `item replace`. Before each step, set the blocks with
`setblock`, and read the packets that arrive after the step.

A mob that `kill` removes stays a blocker for about one second. Use a new position for each mob.

| Experiment | Steps | Result |
|------------|-------|--------|
| E1 | Send `use_item_on` on the top of stone at `(3, -61, 0)`, three times with a clean area. | R1 |
| E2 | Send each of the faces 0 to 5 on stone at `(3, -58, 0)`. | R2 |
| E3 | Send each pair of values for inside and world border hit. | R3 |
| E4 | Send the face values 6, 7 and -1, the hand values 2 and -1, inside as 2, the sequence -1 and a short packet. Use a new client for each. | R4 |
| E5 | Rejoin with an item in hand and without `player_loaded`. Send a request. Send `player_loaded` and send it again. Rejoin without it and send one request each 500 ms. | R5 |
| E6 | Place the blocks of R6. | R6 |
| E7 | Put the observer 10, 15, 15.9, 16.1, 17, 20, 30, 40 and 60 blocks from the sound on `x`. | R7 |
| E8 | Hold stone in the main hand and dirt in the off hand, then an empty main hand. Send the hand values 0 and 1. | R8 |
| E9 | Hold a stack of 5 stone in creative mode. Repeat in survival mode. Read the count. | R9 |
| E10 | Place the blocks of R10, without `op`, and then with `op` for the operator blocks. | R10 |
| E11 | Send a request at a different time in the tick, 10 times. Measure the time to the first packet. | R11 |
| E12 | Put a water source next to the target position, set a redstone lamp next to it, and place a block in a water source. Place sand in the air. | R12 |
| E13 | Set each block of R13 at `(3, -60, 0)`. Send a request on that block. Send a request from below with the block as the adjacent position. | R13 |
| E14 | Place each block of R14 with each ground and each obstacle. | R14 |
| E15 | Set stone and bedrock as the adjacent position. | R15 |
| E16 | Do each row of the table in PLC-TGT-003. | R16 |
| E17 | Place tall grass on dirt. Send a request for stone on each part. Send a request with each part as the adjacent position. | R17 |
| E18 | Place a door at `y=-49`, and a bed with its foot at `z=15`. | R18 |
| E19 | Move the placer along `x` from a block in the row of the eye position. Send a request at each distance, with the faces west and east. | R19 |
| E20 | Send the cursor values of R20. | R20 |
| E21 | Put the placer at `y=316`. Send the requests of R21. Put the placer at `y=-62`. Send the requests of R21. | R21 |
| E22 | Use `spawn-protection=16`, `setworldspawn 0 -60 0` and another player as an operator. Send the requests of R22. | R22 |
| E23 | Set `worldborder center 0 0` and `worldborder set 20`. Send the requests of R23. | R23 |
| E24 | Hold back `accept_teleportation`. Teleport the placer. Send a request. Send the accept and send the request again. | R24 |
| E25 | Hold each item of R25. | R25 |
| E26 | Send the requests of R26 in adventure, spectator and survival mode. | R26 |
| E27 | Place each pair of R27. | R27 |
| E28 | Summon each entity of R28 at the target position, each at a new position, two times. | R28 |
| E29 | Move the placer to each position of R29. | R29 |
| E30 | Summon a pig in each position of R30. `kill` a pig and place at once, then 1.5 s later. | R30 |
| E31 | Set each block of R31. Send a request with stone and with an empty hand. Repeat with sneak. | R31 |
| E32 | Set a light. Send a request without and with `op`. | R32 |
| E33 | Send the bursts of R33. | R33 |
| E34 | Hold each stack of R34. Read the block with `data get block`. | R34 |
| E35 | Hold each item of R35. | R35 |
| E36 | Do each row of the tables in section 7. Use yaw 0, 90, 180 and -90, and pitch -90, -46, -44, 0, 44, 46 and 90. | R36 |
| E37 | Place a door with each condition of R37. | R37 |
| E38 | Place a chest with each condition of R38. | R38 |
| E39 | Send the bursts of R39. | R39 |

Nobody has run experiments E41 to E57 yet.

| Experiment | Steps | Item |
|------------|-------|------|
| E41 | Place each block item on stone, dirt, sand, glass and a bottom slab, on each face. | U2 |
| E42 | Place each block item at each yaw and pitch of E36, on each face, with each cursor half. Place a door with each `facing`. | U3 |
| E43 | Attach `pale_hanging_moss` under leaves. Send a request on it with dirt. | U4 |
| E44 | Set an operator level of 1, 2 and 4 in `ops.json`. Place the blocks of PLC-REF-002 in creative and survival mode. Place a chest with `minecraft:block_entity_data` and an empty `minecraft:container`, and a sign with `minecraft:block_entity_data`, at each level. | PLC-REF-002, PLC-INV-008 |
| E45 | Send a request above the height limit in the area of spawn protection. Hold a stick on a position with an entity. Place a block with no support on a block that is not replaceable. | PLC-ORD-001 |
| E46 | Send the sequence -1 for a break and for a placement, three times each, with a clean area each time. Read the block. | PLC-SEQ-006 |
| E47 | Set each pose. Do the steps of E29. | U9 |
| E48 | In adventure mode, hold an item with `can_place_on` for dirt. Send a request on short grass over dirt, and on stone next to dirt. | U10 |
| E49 | Start a world without the experimental features. Hold an item of such a feature with `item replace`. Send a request. | PLC-REQ-006 |
| E50 | Load a data pack that removes `short_grass` from the tag `minecraft:replaceable` and adds `dandelion`. Send the requests of E13 on both. | PLC-TGT-002 |
| E51 | Hold a fern on a fern, short grass on short grass, glow lichen on glow lichen with one face, and stone on sculk vein. | PLC-TGT-006, PLC-TGT-007 |
| E52 | Send a cursor value of 9. Read the server console. | PLC-CUR-002 |
| E53 | At `y=319`, send a request with face up and a pig at the target. Send it with face north. Send it with a stick. | PLC-HGT-004 |
| E54 | Sneak with an empty main hand and dirt in the off hand. Send hand 0 on a chest. Without sneak, send hand 1 with dirt on a chest. | PLC-INT-002, PLC-INT-006 |
| E55 | In spectator mode, send a request on a nether portal and on an end portal. | PLC-INT-005 |
| E56 | Hold glow berries. Send a request on stone with no space above. | PLC-INV-009 |
| E57 | Do E28 again with each entity at the centre of the target position. Read the entity position with `data get entity` before the request. | U13, PLC-ENT-001 |

## 13. Captured results

Each packet is shortened to its name and the fields that matter. A state name stands for its state id.
`refresh` is two `block_update` packets, and `ack` is `block_changed_ack`. An observer line of a
placement omits `swing_animation`, except in R1 and R39. R1 shows its place in the order.

**R1.** The placer sends seq 101 on the top of stone at `(3,-61,0)`, with stone in the main hand. The
result is the same in three runs.

```text
placer:   block_update (3,-61,0) stone
          block_update (3,-60,0) stone
          block_update (3,-60,0) stone
          block_changed_ack seq 101
observer: sound block.stone.place category 4 at (3.5,-59.5,0.5) volume 1.0 pitch 0.8
          swing_animation bytes 01 00 01 06 (two times)
          block_update (3,-60,0) stone
```

The first value of `swing_animation` is the entity id of the placer. It is 1 here.

**R2.** The clicked position is `(3,-58,0)`.

| Face | Target position |
|------|-----------------|
| 0 | `(3,-59,0)` |
| 1 | `(3,-57,0)` |
| 2 | `(3,-58,-1)` |
| 3 | `(3,-58,1)` |
| 4 | `(2,-58,0)` |
| 5 | `(4,-58,0)` |

**R3.** With the values inside and world border hit at (true, false), (false, true) and (true, true), the
packets are the same as R1. With inside true and stone in the adjacent position, the result is a refresh
and an acknowledgement.

**R4.** The clicked position is `(3,-61,0)`. Each row is one observation.

| Value | Result |
|-------|--------|
| face 6 | acts like face 0: refresh with `(3,-62,0)`, no block |
| face 7 | acts like face 1: stone placed |
| face -1 | acts like face 5: refresh with `(4,-61,0)`, no block |
| hand 2 | stone placed from the main hand |
| hand -1 | stone placed from the main hand |
| inside as the byte 2 | stone placed |
| sequence -1 | `disconnect` with `disconnect.packetError`; no block |
| a packet that stops after the face | `disconnect` with a decode message; no block |

**R5.**

```text
no player_loaded, at once:   nothing; block stays air
then player_loaded, again:   refresh; block_update stone; block_changed_ack seq 10
no player_loaded, one request each 500 ms after the join:
  0.5 s to 3.0 s: nothing
  3.5 s: refresh; block_update stone; block_changed_ack
```

**R6.**

| Block | Sound | Volume | Pitch |
|-------|-------|--------|-------|
| stone, redstone, barrier, light, structure block | `block.stone.place` | 1.0 | 0.8 |
| oak planks, oak door, red bed, torch, oak slab | `block.wood.place` | 1.0 | 0.8 |
| sand | `block.sand.place` | 1.0 | 0.8 |
| dirt | `block.gravel.place` | 1.0 | 0.8 |
| glass | `block.glass.place` | 1.0 | 0.8 |
| sapling, tall grass | `block.grass.place` | 1.0 | 0.8 |
| hopper, rail, turtle egg | `block.metal.place` | 1.0 | 1.2 |
| anvil | `block.anvil.place` | 0.65 | 0.8 |
| wheat seeds, pitcher plant | `item.crop.plant` | 1.0 | 0.8 |

**R7.**

| Observer distance on `x` | Observer gets |
|--------------------------|---------------|
| 10, 15, 15.9 | `sound`, `swing_animation`, `block_update` |
| 16.1, 17, 20, 30, 40 | `swing_animation`, `block_update` |
| 60 | `block_update` |

The view distance of the server is 4.

**R8.**

```text
main stone, off dirt, hand 1: dirt placed; swing_animation bytes 01 01 01 06
main stone, off dirt, hand 0: stone placed; swing_animation bytes 01 00 01 06
main empty, off dirt, hand 1: dirt placed
main empty, off dirt, hand 0: refresh; block_changed_ack; no block
```

**R9.**

```text
creative, 5 stone: block placed; no slot packet; the count stays 5
survival, 5 stone: placer: refresh; block_update; container_set_slot slot 36 count 4; block_changed_ack
                   observer: sound; block_update; set_equipment
adventure, 1 stone: placer: refresh; block_update; container_set_slot slot 36 empty; block_changed_ack
```

**R10.**

```text
oak_sign, no op:
  placer:   block_update oak_sign; open_sign_editor; refresh; block_update; block_entity_data;
            block_changed_ack
  observer: sound; block_update; block_entity_data
white_banner, skeleton_skull, spawner: refresh; block_update; block_entity_data; block_changed_ack
chest, hopper, red_bed: no block_entity_data
command_block, chain_command_block, repeating_command_block, structure_block, jigsaw, test_block,
test_instance_block, no op: refresh; block_changed_ack; block stays air; observer gets nothing
barrier, light, structure_void, spawner, bedrock, no op: placed
op: command_block placed; structure_block, jigsaw, test_block placed, with block_entity_data
```

**R11.** The delay in ms to the first packet: 9.1 45.3 16.7 26.4 23.9 7.6 26.8 33.9 27.8 10.2. In each
run, all four packets arrive together.

**R12.**

```text
stone at (3,-60,0) in a water source:
  refresh; block_update stone; block_changed_ack;
  then block_update and section_blocks_update with air at the four neighbours
stone next to a water source at (4,-60,0):
  refresh; block_update stone; block_changed_ack; then section_blocks_update with flowing water
sand placed in the air at (4,-57,0):
  refresh; block_update sand; block_changed_ack;
  50 ms later add_entity; block_update air at (4,-57,0)
redstone_block next to redstone_lamp:
  refresh; section_blocks_update with redstone_block and lit lamp; block_changed_ack
```

**R13.** `clicked` is a request on the block. `adjacent` is a request from the block below. The held item
is dirt. Each `yes` gives the packets of R1, with dirt at the block position. Each `no` of `clicked`
places the dirt at `(3,-59,0)`. Each `no` of `adjacent` gives a refresh and an acknowledgement.

| Block | clicked | adjacent |
|-------|---------|----------|
| air, cave_air, short_grass, fern, dead_bush, bush, short_dry_grass | yes | yes |
| snow, layers 1 | yes | yes |
| water, levels 0 and 4; lava, levels 0 and 2 | yes | yes |
| fire, vine, structure_void, seagrass, crimson_roots | yes | yes |
| tall_grass, one part alone, leaf_litter with 1 segment, bubble_column | yes | yes |
| glow_lichen, sculk_vein (PLC-TGT-007); pale_hanging_moss (U4) | yes | yes |
| light | no: refresh only (R32) | yes |
| snow, layers 2 and 8 | no | no |
| dandelion, torch, oak_sapling, barrier, kelp, nether_portal, cobweb | no | no |

**R14.** The target position is `(3,-60,0)`. `pair` is `section_blocks_update` with both parts.

```text
oak_door on stone:
  placer:   block_update (3,-61,0) stone; block_update (3,-60,0) lower; pair; block_changed_ack
  observer: sound; pair
oak_door, iron_door on stone, dirt, clay, glass: placed
oak_door on bottom slab: refresh; ack
oak_door, clicked air at (3,-57,0): refresh; ack
tall_grass, sunflower, pitcher_plant on dirt: placed; on stone, clay: refresh; ack
small_dripleaf on clay: placed; on stone, dirt: refresh; ack
red_bed on stone, dirt, clay: placed, the head at (3,-60,1)
red_bed, clicked air at (3,-57,0): placed, with no support
red_bed, no block under the head: placed
oak_door, stone or torch at the upper part: refresh; ack
tall_grass, stone at the upper part: refresh; ack
red_bed, stone at the head: refresh; ack
oak_door, short_grass or water at the upper part: placed; the part replaces it
red_bed, short_grass on dirt at the head: placed; the part replaces it
```

**R15.**

```text
stone in hand, stone at the adjacent position: refresh with both stone; ack; observer gets nothing
dirt in hand, bedrock at the adjacent position: refresh with stone and bedrock; ack
```

**R16.** The results are the table of PLC-TGT-003. Each merge sends:

```text
oak_slab on bottom oak_slab at (3,-60,0), face up:
  placer:   block_update (3,-60,0) double; block_update (3,-59,0) air;
            block_update (3,-60,0) double; block_changed_ack
  observer: sound block.wood.place at (3.5,-59.5,0.5); block_update double
```

**R17.** Tall grass stands on dirt at `(3,-60,0)` and `(3,-59,0)`. The data of `level_event` is the old
state. Each case has one run.

```text
stone on the lower part, clicked or adjacent:
  placer:   level_event type 2001 (3,-59,0) data upper; refresh; section_blocks_update (3,-59,0) air
            and (3,-60,0) stone; block_changed_ack
  observer: level_event type 2001 (3,-59,0) data upper; sound; section_blocks_update
  no item drops; lower part is stone; upper part is air
stone on the upper part, clicked or adjacent:
  placer:   level_event type 2001 (3,-60,0) data lower; refresh; section_blocks_update (3,-59,0) stone
            and (3,-60,0) air; block_changed_ack
  no item drops; upper part is stone; lower part is air
```

**R18.**

```text
oak_door, lower at y=-49, upper at y=-48:
  placer:   block_update (3,-50,0) stone; block_update (3,-49,0) lower; block_update (3,-49,0) lower;
            block_update (3,-48,0) upper; block_changed_ack
  observer: sound; block_update lower; block_update upper
red_bed, foot at z=15, head at z=16:
  placer:   block_update (3,-50,15) stone; block_update (3,-49,15) foot; block_update (3,-49,16) head;
            block_update (3,-49,15) foot; block_changed_ack
```

**R19.** The distance is the distance on `x` from the eye position to the block cube at the clicked
position.

| Distance | Face west | Face east |
|----------|-----------|-----------|
| 5.5, 5.75, 5.9, 5.99 | placed | placed |
| 6.0, 6.01, 6.1, 6.25 | `block_changed_ack` only | `block_changed_ack` only |

**R20.**

```text
cursor (0,0,0), (1,1,1), (1.5,.5,.5), (-.5,.5,.5): placed
cursor x 1.50001, 1.6, -.50001, -.6; y 1.6, -.6; z 1.6, -.6; x NaN; x infinity: block_changed_ack only
in the area of spawn protection, cursor (9,9,9): block_changed_ack only
```

**R21.** `chat high` is `system_chat {translate: "build.tooHigh", with: [319], color: "red"}` in the
overlay position. `chat low` has `build.tooLow` and -64.

```text
clicked y=319 stone, face up:      chat high; chat high; refresh with void_air at y=320; ack
clicked y=320 or 321:              chat high; ack
clicked y=319 air, face down:      placed at y=319
clicked y=318 stone, door, face up: refresh; ack; no message
clicked y=-64 stone, face down:    chat low; refresh with void_air at y=-65; ack
clicked y=-65 or -66:              chat low; ack
clicked y=321, cursor (9,9,9):     ack
clicked y=321, out of reach:       ack
clicked y=321, open teleport:   chat high; ack
```

**R22.** The world spawn is at `(0,-60,0)`, with `spawn-protection=16`. `chat` is
`system_chat {translate: "build.spawn_protection", with: ["x, y, z"], color: "red"}` in the overlay
position, with the clicked position.

```text
clicked x=14, 15, 16, face up:    chat; refresh; ack; observer gets nothing
clicked x=17, 18, face up:        placed
clicked x=16, face east, target x=17: chat; refresh; ack
clicked x=17, face west, target x=16: placed
at (12,16), (16,16), (16,12):     chat; refresh; ack
at (12,17), (17,12):              no chat of this rule
another player, an operator, x=14: placed
no operator on the server, x=14:  placed
lever and chest at the clicked position: chat; refresh; ack; the lever stays; no screen opens
open teleport in the area:     chat; refresh; ack
outside the border, in the area:  chat; refresh; ack
```

**R23.** The commands report the centre as `0.50, 0.50` and the width as 20, so the limits are -9.5 and
10.5. `chat high` is the message of R21. The reference sends it (DEV-1).

```text
clicked x or z = -8, -9, 9, 10, face up: placed
clicked x or z = -10, -11, 11, face up:  chat high; refresh; ack; observer gets nothing
clicked x=11, face west, target x=10:    chat high; refresh; ack
clicked x=10, face east, target x=11:    placed (two runs)
lever at x=11:                           chat high; refresh; ack; the lever stays
bed, foot at x=10, head at x=11:         refresh; ack; no message
an operator with spawn protection, outside the border: chat high; refresh; ack
```

**R24.**

```text
accept held back:    chat high; refresh; ack; observer gets nothing
after the accept:    placed
chest at the clicked position, accept held back: chat high; refresh; ack; no screen opens
accept held back, cursor (9,9,9): ack
```

**R25.** Stick, apple, diamond sword, water bucket, oak boat, end crystal and an empty hand: refresh,
`block_changed_ack`, no block.

**R26.**

```text
adventure, stone, no component:                         refresh; ack
adventure, stone, can_place_on stone, on stone:         placed
adventure, dirt, can_place_on dirt, on stone:           refresh; ack
creative, stone, can_place_on dirt, on stone:           placed
spectator, stone, on stone:                             refresh; ack
spectator, stone, on chest:                             open_screen; container_set_content; refresh; ack
survival, stone, on stone:                              placed
```

**R27.**

| Block | Passes | Refuses |
|-------|--------|---------|
| oak sapling | dirt | stone |
| dandelion | grass block | stone |
| cactus | sand | stone |
| wheat seeds | farmland | stone |
| torch, face up | glass, stone | bottom slab |
| redstone, face up | stone | oak fence |
| lily pad | none tested | stone, water |
| sugar cane | none tested | stone |
| ladder | side of stone | top of stone |

**R28.** Each row is two runs at new positions, with stone for the target position.

```text
refused:  pig, villager, zombie, shulker, primed tnt
          player, not a spectator (one observation)
placed:   armor_stand, armor_stand with Marker, minecart, chest_minecart, furnace_minecart,
          tnt_minecart, hopper_minecart, oak_boat, acacia_chest_boat, item, arrow, experience_orb,
          end_crystal, snowball, falling_block, item_frame, interaction, marker, block_display,
          leash_knot, area_effect_cloud
          spectator player (one observation)
torch with a pig or a player in the position: placed
```

**R29.** The target position is the block from `(3,-60,0)` to `(4,-59,1)`. The placer is at `z=0.5`.

| Placer position | Block | Result |
|-----------------|-------|--------|
| `x=2.69`, `x=2.7`, `x=4.3` | stone | placed |
| `x=2.71`, `x=4.29`, `x=3.5` | stone | refused |
| `x=3.5`, `y=-59`, `y=-58.99`, `y=-61.8`, `y=-61.81` | stone | placed |
| `x=3.5`, `y=-59.01`, `y=-61.79` | stone | refused |
| `x=3.5`, `y=-59.5`, `y=-59.49` | bottom slab | placed |
| `x=3.5`, `y=-59.51` | bottom slab | refused |
| `x=3.5`, `y=-59.5`, `y=-59.51`, `y=-59.49` | top slab | refused |
| `x=3.5`, `y=-60` | oak slab, oak fence | refused |
| `x=3.5`, `y=-60` | sapling, rail, torch | placed |

**R30.** Two runs. The pig has no AI and no gravity.

```text
pig in the lower part of a door: refused
pig in the upper part of a door: placed
pig in the foot of a bed:        refused
pig in the head of a bed:        placed
pig in either part of a sunflower: placed
pig above a bottom slab, at y=-59.5: placed; with a top slab: refused
pig killed at once before the request: refused; 1.5 s later: placed
```

**R31.** One run for each row. `act` is the result for stone without sneak, for an empty hand, and for an
empty hand with sneak. `sneak` is the result for stone with sneak.

| Clicked block | act | sneak |
|---------------|-----|-------|
| chest | `sound`, `open_screen`, `container_set_content`, refresh, `block_event`, ack | placed above |
| barrel | `sound`, `open_screen`, `container_set_content`, refresh, `block_update`, ack | placed above |
| furnace | `open_screen`, `container_set_content`, `container_set_data`, refresh, ack | placed above |
| crafting table | `open_screen`, `container_set_content`, refresh, ack | placed above |
| lever | `sound`, refresh, `block_update`, ack | placed above |
| door, trapdoor, button | refresh, `block_update`, ack | placed above |
| note block | refresh, `block_update`, `sound`, `block_event`, ack | placed above |
| cake | refresh, `block_update`, ack | placed above |
| oak sign | `block_update`, `open_sign_editor`, `swing_animation`, refresh, ack | placed above |

In each row with `act`, an observer gets `swing_animation`. No block appears at the adjacent position.

**R32.**

```text
dirt on light, no op:  refresh; ack; the light stays
dirt on light, op:     swing_animation; refresh; block_update light[level=0]; ack
light on light, no op: refresh; ack
```

**R33.** Each line of a burst has no wait between the packets.

```text
set_creative_mode_slot 36 stone; 150 ms; use_item_on: stone placed; observer: set_equipment first
set_creative_mode_slot 40 dirt; set_carried_item 4; use_item_on: dirt placed
set_carried_item 0; use_item_on: stone placed
set_creative_mode_slot 45 dirt; set_carried_item 5; use_item_on hand 1: dirt placed
set_carried_item 0; set_creative_mode_slot 36 empty; use_item_on: refresh; ack
main stick, off dirt: use_item_on hand 0; use_item_on hand 1: refresh; refresh; block_update dirt; ack
```

**R34.**

```text
oak_slab[block_state={type:"top"}]:                         oak_slab type=top
oak_stairs[block_state={facing:"east",half:"top"}]:         oak_stairs facing=east half=top
oak_slab[block_state={type:"nope"}]:                        oak_slab type=bottom
chest[custom_name="Box"]:                                   CustomName: "Box"
chest[container=[{slot:0,item:{id:"minecraft:diamond",count:2}}]]: Items with 2 diamonds
chest[block_entity_data={id:"minecraft:chest",Items:[...]}], no op and op: Items empty
oak_sign[block_entity_data={...,is_waxed:1b}], no op:       is_waxed 0b
spawner[block_entity_data={...,Delay:7s}], no op:           Delay 13s
```

**R35.**

```text
string on stone:            tripwire placed; sound block.stone.place
sculk_vein on stone, north: sculk_vein south=true placed
flint_and_steel on stone:   sound item.flintandsteel.use; refresh; block_update fire; ack
armor_stand, pig_spawn_egg on stone: add_entity; sound; refresh; ack
painting on stone, north:   sound; add_entity; refresh; ack
```

**R36.** All tested with the placer at `(0.5,-60,0.5)`.

```text
yaw 0 is south, 90 west, 180 north, -90 east.
oak_stairs, oak_door, red_bed, oak_fence_gate: facing = look
furnace, chest, oak_trapdoor, carved_pumpkin, repeater, white_glazed_terracotta, lectern:
  facing = opposite of look
anvil: yaw south gives west, west gives north, north gives east, east gives south
piston, dispenser, yaw -90: pitch -90, -46: down; -44, 0, 44: west; 46, 90: up
observer, yaw -90:          pitch -90, -46: up; -44, 0, 44: east; 46, 90: down
oak_sign, white_banner rotation: yaw 0: 8; 11: 8; 12: 9; 23: 9; 45: 10; 180: 0; -90: 4
skeleton_skull rotation:         yaw 0: 0; 11: 0; 12: 1; 23: 1; 45: 2; 180: 8; -90: 12
slab, cursor y 0.49 and 0.5, face west: bottom; 0.51 and 0.9: top
oak_stairs, cursor y 0.4, face west: bottom; 0.6: top
oak_log: face 0, 1: y; 2, 3: z; 4, 5: x
iron_chain: face up: y; east: x
vine: face down: up=true; face north: south=true; face south: north=true; face west: east=true;
      face east: west=true; face up: refused
glow_lichen: face up: down=true; other faces as vine
oak_hanging_sign: face down: oak_hanging_sign rotation 8; side: oak_wall_hanging_sign; face up: refused
fence next to fence: section_blocks_update with both, each connected to the other
stairs next to stairs: shape=inner_left
oak_slab in a water source: waterlogged=true; in water level 4 or lava: waterlogged=false
oak_stairs, oak_fence, lantern, chest, oak_sign, oak_trapdoor in a water source:
  waterlogged=true
stone, torch, oak_door in a water source: the source is gone, and neighbours update
oak_slab on a bottom slab that holds water: double, waterlogged=false
oak_door, oak_trapdoor next to redstone_block: powered=true, open=true
```

**R37.** The door faces south. The placer sends a request on the top of stone. The cursor is
`(0.5, 0.5, 0.5)` when the table does not state another value.

| Condition | Hinge |
|-----------|-------|
| nothing near | left |
| cursor `x=0.2` | right |
| cursor `x=0.8` | left |
| cursor `z=0.2` or `z=0.8` | left |
| stone to the east, in both rows | left |
| stone to the west, in both rows | right |
| door to the east, hinge left | right |
| door to the east, hinge right | right |
| door to the west, hinge left | left |

**R38.** The placer looks south, so a new chest has `facing=north`. The neighbour is to the east.

```text
neighbour facing north:         new chest type=left; neighbour type=right; section_blocks_update
neighbour facing south:         new chest type=single
click on the neighbour, no sneak: the neighbour opens
sneak, click on the ground:     new chest type=single
sneak, click on the neighbour:  new chest type=left; neighbour type=right
```

**R39.**

```text
(3,-61,0) seq 20, then (4,-61,0) seq 21:
  placer:   refresh; refresh; section_blocks_update with both; block_changed_ack seq 21
  observer: sound; sound; section_blocks_update; swing_animation four times
seq 31, then seq 30: the same; block_changed_ack seq 31
one request three times, seq 96:
  placer:   three refreshes; block_update; block_changed_ack seq 96
  observer: one sound; one block_update; swing_animation two times
seq 40 placed, then seq 41 with a bad cursor: block_changed_ack seq 41
seq 50, then 200 ms later seq 51: each has its own block_changed_ack
seq 0, 1, 2147483646, 2147483647: placed; block_changed_ack with that value
player_action start on stone seq 60, then use_item_on on the same position seq 61:
  placer:   refresh; block_update stone; block_changed_ack seq 61
  observer: level_event type 2001 data stone; sound; block_update stone
```
