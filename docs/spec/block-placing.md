# Block placing: behavioural specification

| Field | Value |
|-------|-------|
| Version | see `assets/minecraft/version.json` |

This document states how a player places a block in creative mode, and what the server sends for it. It
states only what a client, the server console and the server files show. Most rules come from a vanilla server in offline mode, with scripted clients. A rule with a
`source_suggested` label comes from the reference implementation. Section 12 gives
the experiments.

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

- `black_box_confirmed`: shown by an experiment on a vanilla server.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent with an
  experiment on a vanilla server.
- `source_suggested`: taken from the reference implementation, with no experiment result. Section 12
  attaches an experiment.
- `unresolved`: not decided. Section 11 collects these items, with an experiment for each item.

All the results come from one vanilla server and scripted clients on one machine. A rule that rests on
one observation states this.

### 0.3 ID prefix

Each rule has an ID with the prefix `PLC`.

## 1. The request

**PLC-REQ-001** (`black_box_confirmed`, wire). The `use_item_on` packet has these fields, in this order:
hand (VarInt), position (long), face (VarInt), cursor x, y and z (three floats), inside (boolean), world
border hit (boolean), sequence (VarInt).

**PLC-REQ-002** (`black_box_confirmed`, wire). The face values are 0 down, 1 up, 2 north, 3 south, 4 west
and 5 east.

**PLC-REQ-003** (`black_box_confirmed`, semantic). The values of inside and world border hit had no
effect in each tested case. The tested cases are stone on stone, with each pair of values.

**PLC-REQ-004** (`black_box_confirmed`, semantic). A packet that does not decode closes the connection.

The reference implementation also accepts hand and face values that the vanilla client does not send.
This document leaves their meaning unspecified.

**PLC-REQ-005** (`black_box_confirmed`, semantic). The server ignores a request until the client has
loaded. There is no acknowledgement, no block state and no placement. The client has loaded after it
sends `player_loaded`, or about 60 ticks after the player joins. This agrees with
BRK-REQ-005.

**PLC-REQ-006** (`source_suggested`, semantic). The server ignores a request when the held item is not
enabled by the enabled features of the world. The placer gets one acknowledgement and nothing else.
Experiment E49.

## 2. A placement

**PLC-PLA-001** (`black_box_confirmed`, semantic). The placer gets, in this order:

1. a refresh;
2. the new state of each changed block;
3. one acknowledgement. The state of step 2 repeats a packet of step 1 when the changed block is at a position of
the refresh. Only the content of the packets is required. A parity test must not count this repeated
packet (kind: unspecified).

**PLC-PLA-002** (`black_box_confirmed`, wire). The server sends `block_update` when one block changes in
a section. It sends `section_blocks_update` when more than one block in a section changes together.

**PLC-PLA-003** (`black_box_confirmed`, semantic). For one placement, an observer receives only the
packets applicable to it, in this order when present: `sound`, `swing_animation`, the new block
state. An observer does not get an acknowledgement or a refresh. For several placements in one tick,
the observer gets each delivered `sound` and `swing_animation` before the block states
([arm-swing.md](arm-swing.md), `SWG-DST-003`). An observer holding the chunk without tracking the
placer, or a repeat placement blocked by cooldown or the same-tick cap (`SWG-STA-005`, `SWG-STA-004`),
omits `swing_animation`.

**PLC-PLA-004** (`black_box_confirmed`, semantic). The placer does not get `sound` or `swing_animation`
for a placement.

**PLC-PLA-005** (`source_suggested_black_box_confirmed`, wire). The `sound` packet has the place sound
of the placed block, the category value 4, and the centre of the target position as its position. A block with two parts uses the target position, and not the other part. The sound set
of the placed state gives the sound. The block definition names the sound set
(`mcrs:block_sound_set`), and the `block_sound_set` registry of the data pack holds it. The volume is
`(v + 1) / 2` and the pitch is `p * 0.8`, where `v` and `p` are the volume and the pitch of the sound set.
Each has the default 1.0. A sound set with no place sound sends no `sound`.

**PLC-PLA-006** (`source_suggested_black_box_confirmed`, semantic). An observer gets the `sound` when
the distance from its position to the sound position is less than the range of the sound. The range is
16, or 16 times the volume when the volume is above 1. A sound event with a fixed range uses that range.
A distance of 15.9 gets it and 16.1 does not. An observer gets the new block state while
its client holds the chunk.

**PLC-PLA-007** (`black_box_confirmed`, wire). The `swing_animation` packet names the placer and the
hand (`0` main, `1` off). It sends the type and duration from `minecraft:interact_animation`
([arm-swing.md](arm-swing.md), `SWG-CMP-002`). Observers that do not track the placer receive no
packet ([arm-swing.md](arm-swing.md), `SWG-DST-001`). Single-packet emission when cooldown and tick
rules allow a swing follows DEV-3 (`design`, `deviation`; [arm-swing.md](arm-swing.md), `SWG-SRC-007`,
`SWG-STA-005`, `SWG-STA-004`).

**PLC-PLA-008** (`black_box_confirmed`, semantic). In creative mode, the held item does not change. The
placer gets no slot packet, and an observer gets no `set_equipment`.

**PLC-PLA-009** (`black_box_confirmed`, semantic). A block entity whose client data is not empty sends
`block_entity_data` after the block state and before the acknowledgement. The tested blocks are a sign, a
banner, a skull, a spawner, a structure block, a jigsaw block and a test block. A chest, a hopper and a
bed send no such packet. Each player whose client holds the chunk gets it.

**PLC-PLA-010** (`black_box_confirmed`, semantic). A sign and a hanging sign send `open_sign_editor` to
the placer. The placer gets one more `block_update` for the sign before this packet.

**PLC-PLA-011** (`black_box_confirmed`, semantic). The server handles a request between two ticks, as
BRK-BRK-007 states. The acknowledgement comes with the block states.

**PLC-PLA-012** (`black_box_confirmed`, semantic). A later result of the placement comes after the
acknowledgement. A fluid that flows and a block that falls are the tested cases. A change in the same
tick comes before it, in the same packet where the section is the same.
`block-simulation.md` states how neighbours update.

## 3. The target position

**PLC-TGT-001** (`black_box_confirmed`, semantic). The target position is the clicked position when the
block there is replaceable. Otherwise the target position is the adjacent position. The block at the
target position must also be replaceable, or the placement is refused.

The clicked position does not need support: a request on air places the block at that air position. A later rule can refuse the placement. The vanilla client does not send this request in
normal use.

**PLC-TGT-002** (`source_suggested_black_box_confirmed`, semantic). A block is replaceable when its
block definition has the `minecraft:replaceable` component, with these conditions. The
block tag `minecraft:replaceable` has the same members in this version, but the server does not read the
tag. A data pack that changes the tag does not change this set. Experiment E50.

- Snow is replaceable only with `layers=1` when the held item is not snow.
- With snow in hand, snow with fewer than 8 layers is replaceable at the adjacent position. At the
  clicked position, it is replaceable only with the face up. PLC-TGT-003 states the merge. This item is
- Water and lava are replaceable at each tested level: 0 and 4 for water, 0 and 2 for lava.
- Light is replaceable only as the adjacent position (PLC-INT-003).

These blocks are replaceable: air, cave air, short grass, fern, dead bush, bush, short dry grass,
water, lava, fire, vine, light, structure void, seagrass, crimson roots, tall grass, leaf
litter with one segment and bubble column. These blocks are not replaceable: snow with more
than one layer, dandelion, torch, oak sapling, barrier, kelp, nether portal, cobweb, bedrock and stone.

**PLC-TGT-003** (`black_box_confirmed`, semantic). When a block merges with the block in hand, the block
at the clicked position or the adjacent position counts as replaceable. A merge changes the block, and it
is a placement.

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
state of the other part, before the other packets.

**PLC-TGT-005** (`black_box_confirmed`, semantic). A placement on a fluid removes the fluid, unless the
placed state holds water (PLC-STA-005).

**PLC-TGT-006** (`source_suggested`, semantic). A block of PLC-TGT-002 is not replaceable when the held
item is the item of that block. A fern in hand on a fern places the fern at the adjacent position. The
blocks of PLC-TGT-003 and of PLC-TGT-007 have their own rule. Experiment E51.

**PLC-TGT-007** (`source_suggested_black_box_confirmed`, semantic). A glow lichen and a sculk vein are
replaceable when the held item is another item. A sculk vein has no `minecraft:replaceable` component,
but this rule applies to it too. With their own item in hand, they are replaceable only while a face can
still take a surface. Experiment E51.

## 4. Refusals

### 4.1 Refusals that send only an acknowledgement

**PLC-RCH-001** (`black_box_confirmed`, semantic). A request is refused when the distance from the eye
position to the nearest point of the block cube at the clicked position is 6.0 or more. A distance less
than 6.0 is in reach. The placer gets one acknowledgement and no block state. An observer gets nothing. BRK-RCH-001 states the source of the limit. The face has no effect on the result, so a
target position further than 6.0 can receive a block.

**PLC-CUR-001** (`black_box_confirmed`, semantic). A request is refused when a cursor value is below -0.5
or above 1.5. A value that is not a number also refuses it. The placer gets one acknowledgement and no
block state. An observer gets nothing. The limits hold to 0.00001: -0.5 and 1.5 pass, and
-0.50001 and 1.50001 do not.

**PLC-CUR-002** (`source_suggested`, semantic). In the refusal of PLC-CUR-001, the server console gets
one warning line. The line names the player, the cursor location and the clicked position. Experiment
E52.

### 4.2 The height limits

**PLC-HGT-001** (`source_suggested_black_box_confirmed`, wire). A clicked position above the height
limit of the dimension is refused. The placer gets `system_chat` in the overlay position, and then one
acknowledgement. The message is the translation key `build.tooHigh` in red, with the highest block `y` of
the dimension as the argument. A clicked position below the height limit does the same with the key
`build.tooLow` and the lowest block `y`. In the overworld, the arguments are 319 and -64.

**PLC-HGT-002** (`source_suggested_black_box_confirmed`, wire). A clicked position in the world with an
adjacent position outside the height limits is refused when the block at the clicked position is not
replaceable. The placer gets a refresh and one acknowledgement. The state of the adjacent position in the
refresh is `void_air`.

**PLC-HGT-004** (`source_suggested_black_box_confirmed`, wire). The placer also gets the message of
PLC-HGT-001 before the refresh when all these conditions are true. Experiment E53.

- The face is up and the clicked `y` is at the upper height limit or above it.
- Or the face is down and the clicked `y` is at the lower height limit or below it.
- The request places no block and the clicked block does not act.
- The held item is a block item or a bucket that holds a fluid, and the item has no cooldown.

The reason of the refusal has no effect. A side face sends no message. Section 10 (DEV-2) states a
difference of the reference.

**PLC-HGT-003** (`black_box_confirmed`, semantic). A block with two parts that would pass the height
limit is refused with a refresh and one acknowledgement, and with no message. This rule
rests on one observation: a door with its lower part at `y=319`.

### 4.3 Spawn protection

**PLC-SPN-001** (`black_box_confirmed`, semantic). The conditions of BRK-SPN-001 refuse a request, with
the clicked position as the block position. The target position has no effect.

**PLC-SPN-002** (`black_box_confirmed`, wire). In this refusal, the placer gets `system_chat` in the
overlay position, a refresh, and one acknowledgement. The message is the translation key
`build.spawn_protection` in red, with the argument `x, y, z` of the clicked position. An observer gets
nothing.

**PLC-SPN-003** (`black_box_confirmed`, semantic). This refusal also stops the use of the clicked block
(section 5). A lever stays as it is and a chest does not open.

### 4.4 The world border and an open teleport

**PLC-BRD-001** (`black_box_confirmed`, semantic). A request is refused when the block corner of the
clicked position is outside the world border. The block corner is outside when `x` or `z` is below the
lower limit of the border, or is at or above the upper limit. The placer gets a refresh and one
acknowledgement. An observer gets nothing. An operator is also subject to this rule.
Section 10 (DEV-1) states a difference of the reference.

**PLC-BRD-002** (`black_box_confirmed`, semantic). The target position can be outside the border when the
clicked position is inside it. The block is placed.

**PLC-BRD-003** (`black_box_confirmed`, semantic). A bed with its head outside the border is refused with
a refresh and one acknowledgement. This rule rests on one observation.

**PLC-TEL-001** (`black_box_confirmed`, semantic). A request is refused while an open teleport exists
(`teleport-confirmation.md`). The placer gets a refresh and one acknowledgement. An observer gets
nothing. Section 10 (DEV-1) states a difference of the reference.

### 4.5 Refusals that send a refresh

**PLC-REF-001** (`black_box_confirmed`, semantic). In each case of this table, the placer gets a refresh
and then one acknowledgement. An observer gets nothing.

| Case |
|------|
| The hand is empty, or the held item is not a block item and does not act on the clicked block (section 6) |
| The target position does not hold a replaceable block (PLC-TGT-001) |
| The player is in spectator mode |
| The player is in adventure mode, and PLC-INV-003 does not allow the placement |
| The block needs support, and the position does not give it (PLC-SUP-001) |
| The placed block would overlap an entity (PLC-ENT-001) |
| The block is one of PLC-REF-002, and the player is not in creative mode with permission level 2 or more |
| The other part of the block cannot go in its position (section 8) |

**PLC-REF-002** (`source_suggested_black_box_confirmed`, semantic). A command block, a chain command
block, a repeating command block, a structure block, a jigsaw block, a test block and a test instance
block need a player in creative mode with the permission level 2 or more. Any other player cannot place
them. A barrier, a light, a structure void, a spawner and bedrock have no such limit.
Experiment E44.

**PLC-SUP-001** (`black_box_confirmed`, semantic). Some blocks need support from a neighbour. The rule of each block is block data, and section 11 (U2) states what remains
unknown. `block-support.md` states the support test of some blocks (SUP-RUN-002).

**PLC-ENT-001** (`source_suggested`, semantic). A placement is refused when the collision shape of the
placed state overlaps the bounding box of a blocker. Contact alone is not an overlap. A spectator is never
a blocker. These entities are blockers:

- each living entity, for example a player, a pig, a villager, a zombie, a shulker and an armor stand
  that is not a marker;
- primed TNT, an end crystal and a falling block;
- each minecart and each boat.

The other entities are not blockers. Examples are an armor stand that is a marker, an item, an arrow, an
experience orb, a snowball, an item frame, an interaction, a marker, a block display, a leash knot and an
area effect cloud. Section 11 (U13) states the conflict for the armor stand, the minecarts, the boats,
the end crystal and the falling block. Experiment E57.

**PLC-ENT-002** (`black_box_confirmed`, semantic). The placer is also a blocker. The box
of a player that stands is 0.6 wide and 1.8 high, with the position at the centre of its bottom.

**PLC-ENT-003** (`black_box_confirmed`, semantic). A block with no collision shape has no such refusal. A
torch, a sapling, a rail and a tall plant do not hit a blocker. A bottom slab
under a blocker is placed when the box does not reach into the slab. A top slab with the same blocker is
refused.

**PLC-ENT-004** (`black_box_confirmed`, semantic). Only the part at the target position counts. A pig in
the upper part of a door or in the head of a bed does not refuse the placement.

**PLC-ENT-005** (`black_box_confirmed`, semantic). A mob with no health stays a blocker until the server
removes it. This rule rests on one observation.

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

PLC-HGT-004 is not a check. Its message comes after all the steps. Steps 2 and 3 send the same packets, so a client cannot see
their order. The same is true of the two checks in step 6, except for the message of DEV-1, and of steps
8 to 14. Experiment E45.

## 5. The use of the clicked block

**PLC-INT-001** (`black_box_confirmed`, semantic). The use of the clicked block comes before a placement.
A chest, a barrel, a furnace, a crafting table, a lever, a button, a door, a trapdoor, a note block, a
sign and a cake act on the request with a block item in hand, and no block is placed.
This document does not state what each block does.

**PLC-INT-002** (`source_suggested_black_box_confirmed`, semantic). When the placer sneaks and either hand
holds an item, the clicked block does not act, and the placement goes on. The hand of the request has no
effect: with sneak, an empty main hand and an item in the off hand, the clicked block does not act. With
sneak and two empty hands, the clicked block acts. Experiment E54.

**PLC-INT-003** (`black_box_confirmed`, semantic). A light at the clicked position acts for an operator,
and its level changes. For another player, the result is a refresh and one acknowledgement.

**PLC-INT-004** (`black_box_confirmed`, semantic). When the clicked block acts, the placer gets a
refresh and one acknowledgement. Other packets of the block can come before or after the refresh.
Observers that track the actor receive `swing_animation` when cooldown and tick rules allow a swing
([arm-swing.md](arm-swing.md), `SWG-SRC-006`, `SWG-DST-001`), with single-packet emission under
DEV-3 (`design`, `deviation`; `SWG-SRC-007`, `SWG-STA-005`, `SWG-STA-004`).

**PLC-INT-005** (`source_suggested_black_box_confirmed`, semantic). In spectator mode, a block with a
screen opens it and no block changes. A chest is the tested case. On a portal block, the
spectator goes to the destination of the portal, or gets `spectator.cannot_teleport` in red in the overlay
position when the portal has no destination. Experiment E55.

**PLC-INT-006** (`source_suggested`, semantic). A block that acts without the held item acts only for the
hand value 0. With the hand value 1, a chest does not open, and the placement goes on. A block that acts
on the held item, such as a cake with a candle, acts for each hand. Experiment E54.

## 6. The held item

This section depends on the player inventory. Section 11 (U7) states this dependency.

**PLC-INV-001** (`black_box_confirmed`, semantic). The hand value 0 selects the selected hotbar slot, and
1 selects the off hand. The other hand has no effect on the request: a stick in the main hand does not
stop a block in the off hand, and an empty main hand does not fall back to the off hand.

**PLC-INV-002** (`black_box_confirmed`, semantic). The server handles `set_creative_mode_slot`,
`set_carried_item` and `use_item_on` in the order of arrival, in one burst too. The slot 36 is the first
hotbar slot, and 45 is the off hand. A later request uses the new item. An empty stack in the selected
slot gives PLC-REF-001.

**PLC-INV-003** (`black_box_confirmed`, semantic). In adventure mode, the `minecraft:can_place_on`
component must match the block at the clicked position, or the placement is refused. Creative mode
ignores this component.

**PLC-INV-004** (`black_box_confirmed`, semantic). The `minecraft:block_state` component sets the named
properties of the placed state. A value that the block does not have is ignored, and the placement uses
the normal state.

**PLC-INV-005** (`black_box_confirmed`, semantic). The `minecraft:custom_name` and `minecraft:container`
components go to the block entity of the placed block.

**PLC-INV-006** (`black_box_confirmed`, semantic). Outside creative mode, the count of the stack falls by
one. The placer gets `container_set_slot` before the acknowledgement. An observer gets `set_equipment`
after the block state. This document states only this observation. A later specification
of the use of an item owns this rule.

**PLC-INV-007** (`black_box_confirmed`, semantic). The block of an item can have another name. `redstone`
places `redstone_wire`, and `string` places `tripwire`.

**PLC-INV-008** (`source_suggested`, semantic). The `minecraft:block_entity_data` component goes to the
block entity of the placed block when its `id` is the type of that block entity. For a command block, a
lectern, a sign, a hanging sign, a spawner and a trial spawner, the player must also be in creative mode
with the permission level 2 or more. After this, the other components of the item go to the block entity,
so the default `minecraft:container` component of a chest replaces `Items`. Experiment
E44.

**PLC-INV-009** (`source_suggested`, semantic). When a placement is refused and the held block item has
the `minecraft:consumable` component, the server starts the use of the item, as the use of the item in
the air does. Experiment E56.

An item such as a flint and steel, an armor stand, a spawn egg or a painting also acts through this
packet. It is not a block item, and this document does not state it.

## 7. The state of the placed block

This section gives only the tested families. Section 11 (U3) states what remains unknown. The direction
words are the direction in which the placer looks. The rules and the rows that an experiment shows are
`black_box_confirmed`, kind semantic. The other rows of PLC-STA-004, and the pane and the redstone wire of
PLC-STA-006, are `unresolved`. Section 11 (U3) holds them.

**PLC-STA-001**. Direction from the yaw of the placer.

| Result | Tested blocks |
|--------|---------------|
| `facing` is the direction of the look | stairs, door, bed, fence gate |
| `facing` is the opposite direction | furnace, chest, trapdoor, carved pumpkin, repeater, glazed terracotta, lectern |
| `facing` is the direction after one quarter turn clockwise (south gives west) | anvil |

**PLC-STA-002**. Direction from the pitch of the placer. A pitch below -45 is up and a pitch above 45 is
down. Otherwise the direction is the horizontal direction of the look.

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
| Vine, glow lichen | see PLC-STA-004 | the surface is on the side of the clicked block | the surface is on the side of the clicked block |
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
`block_update`. The order of the two parts in the packet is unspecified.

**PLC-PRT-002** (`black_box_confirmed`, semantic). The target position takes the lower part of a door or
a tall plant, and the foot of a bed. The other part goes above it, or in the direction of the look for a
bed.

**PLC-PRT-003** (`black_box_confirmed`, semantic). The position of the other part must hold a replaceable
block. Otherwise the result is a refresh and one acknowledgement.

**PLC-PRT-004** (`black_box_confirmed`, semantic). Support:

| Block | Tested support |
|-------|----------------|
| Door | Stone, dirt, clay and glass pass. A bottom slab and no block refuse. |
| Tall grass, sunflower, pitcher plant | Dirt passes. Stone and clay refuse. |
| Small dripleaf | Clay passes. Stone and dirt refuse. |
| Bed | No support is necessary, at the foot or at the head. |

**PLC-PRT-005** (`black_box_confirmed`, semantic). The hinge of a door comes from its neighbours and the
cursor. Section 11 (U3) states what
remains unknown.

**PLC-PRT-006** (`black_box_confirmed`, semantic). A chest makes a pair with a neighbour chest of the
same `facing`. Both chests change, in one `section_blocks_update`: the new chest has `type=left` and the
old chest has `type=right`, for the tested placement. A chest with another `facing` stays `single`. When
the placer sneaks, the new chest stays `single`, unless the clicked block is the neighbour chest.

## 9. Several requests and the sequence

**PLC-SEQ-001** (`black_box_confirmed`, semantic). Requests that the server handles between two ticks get
one acknowledgement. It carries the highest sequence of those requests, whichever came last. A refused request counts too.

**PLC-SEQ-002** (`black_box_confirmed`, semantic). Each request gets its own refresh. Each placement
gets its own `sound` for an observer. Under DEV-3 (`design`, `deviation`), a tracking observer gets
one `swing_animation` only when [arm-swing.md](arm-swing.md) (`SWG-SRC-007`, `SWG-STA-005`,
`SWG-STA-004`) starts a swing. Block states that change in one tick use `PLC-PLA-002`.

**PLC-SEQ-003** (`black_box_confirmed`, semantic). Requests about 200 ms apart each get their own
acknowledgement.

**PLC-SEQ-004** (`black_box_confirmed`, semantic). A break and a placement in one tick share one
acknowledgement, with the highest sequence. This rule rests on one observation.

**PLC-SEQ-005** (`black_box_confirmed`, wire). The sequences 0, 1, 2147483646 and 2147483647 are valid.

**PLC-SEQ-006** (`source_suggested_black_box_confirmed`, semantic). A negative sequence closes the
connection with the translation key `disconnect.packetError`. The block is not placed, because the server
checks the sequence before it handles the request. A break handles the request before it
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

**DEV-3.**

- **Rule**: PLC-PLA-007, PLC-INT-004, PLC-SEQ-002.
- **Reference**: vanilla sends two `swing_animation` packets per recipient when `use_item_on` starts a
  swing. Both accepted placements in the frozen-world probe start swings
  ([arm-swing.md](arm-swing.md), `SWG-E3`). `SWG-U1` records the open question about repeat triggers.
- **Reason**: [arm-swing.md](arm-swing.md) (`SWG-DEV-001`, `SWG-DEV-002`) removes duplicate packets
  and caps swings at one per actor per tick (`#906`).
- **Visible to the client**: yes, only through the raw packet count (`1` instead of `2`, or `0` on a
  repeat).
- **Bug reference**: none.
- **Test**: for two accepted placements in one tick by a placer with no active cooldown and no prior
  swing in that tick, a tracking observer present throughout both placements checks `1`
  `swing_animation` packet on the first placement and `0` on the repeat on `mcrs`, and `2` then `2`
  on vanilla.

## 11. Unresolved

Each item names the experiment that decides it.

- **U2**: the support rule of each block. Experiment E41. Part 1 of `block-support.md` states the support
  test of the blocks of its section 5.
- **U3**: the state of each block family, the hinge of a door with another `facing`, and the block
  families that this document does not test. This includes the rows of PLC-STA-004 that no experiment shows. A block that attaches to a neighbour, such as a lantern or a torch, can try other directions
  in the order of the look when the clicked face does not hold it. So a refusal in PLC-STA-004 can
  depend on the neighbours. Experiment E42.
- **U4**: `pale_hanging_moss`. An experiment replaced it, but the block definition has no
  `minecraft:replaceable` component, and the reference does not replace it. The moss had no support in the
  experiment, and it possibly broke before the request. Experiment E43.
- **U7**: the player inventory. Each placement needs the held item, and so do sections 3, 5,
  6, 7 and 8. Without it, a server
  can state only the refusals of sections 4.1 to 4.4, the refresh and the acknowledgement of section 9.
  `inventory.md` states what `set_creative_mode_slot` accepts (INV-CRE-003, INV-CRE-004). This document
  tests only the slots 36 and 45, and a stack with no components.
- **U9**: the box of a player that sneaks, swims or has another pose. All experiments used a player that
  stands. Experiment E47.
- **U10**: can_place_on when the clicked block is replaceable. Only stone is tested. Experiment
  E48.
- **U11**: a request at a position in reach in a chunk that is not loaded.
- **U12**: a `moving_piston` at the clicked position changes to air in the one tested case. This looks
  like an accident, and this document does not state it.
- **U13**: an experiment places a block on an armor stand, a minecart, a boat, an end crystal and a falling
  block. The reference refuses these placements (PLC-ENT-001). Experiment E57.

## 12. Experiments

Use a vanilla server of the game version of `assets/minecraft/version.json`, with
`online-mode=false`, `level-type=minecraft:flat` and `network-compression-threshold=-1`. Use
`spawn-protection=0`, except where a step states another value. Set the game rule `spawn_mobs` to false.
Connect one scripted client as the placer and one as the observer, both in creative mode. Put the placer
at `(0.5, -60, 0.5)` with yaw 0, and the observer at `(8.5, -60, 8.5)`, except where a step states
another position. Give each client the item with `item replace`. Before each step, set the blocks with
`setblock`, and read the packets that arrive after the step.

A mob that `kill` removes stays a blocker for about one second. Use a new position for each mob.

| Experiment | Steps | Item |
|------------|-------|------|
| E41 | Place each block item on stone, dirt, sand, glass and a bottom slab, on each face. | U2 |
| E42 | Place each block item at yaw 0, 90, 180 and -90, and at pitch -90, -46, -44, 0, 44, 46 and 90, on each face, with each cursor half. Place a door with each `facing`. | U3 |
| E43 | Attach `pale_hanging_moss` under leaves. Send a request on it with dirt. | U4 |
| E44 | Set an operator level of 1, 2 and 4 in `ops.json`. Place the blocks of PLC-REF-002 in creative and survival mode. Place a chest with `minecraft:block_entity_data` and an empty `minecraft:container`, and a sign with `minecraft:block_entity_data`, at each level. | PLC-REF-002, PLC-INV-008 |
| E45 | Send a request above the height limit in the area of spawn protection. Hold a stick on a position with an entity. Place a block with no support on a block that is not replaceable. | PLC-ORD-001 |
| E46 | Send the sequence -1 for a break and for a placement, three times each, with a clean area each time. Read the block. | PLC-SEQ-006 |
| E47 | Set each pose. Send a request whose block overlaps the box of the player. | U9 |
| E48 | In adventure mode, hold an item with `can_place_on` for dirt. Send a request on short grass over dirt, and on stone next to dirt. | U10 |
| E49 | Start a world without the experimental features. Hold an item of such a feature with `item replace`. Send a request. | PLC-REQ-006 |
| E50 | Load a data pack that removes `short_grass` from the tag `minecraft:replaceable` and adds `dandelion`. Send a request on both. | PLC-TGT-002 |
| E51 | Hold a fern on a fern, short grass on short grass, glow lichen on glow lichen with one face, and stone on sculk vein. | PLC-TGT-006, PLC-TGT-007 |
| E52 | Send a cursor value of 9. Read the server console. | PLC-CUR-002 |
| E53 | At `y=319`, send a request with face up and a pig at the target. Send it with face north. Send it with a stick. | PLC-HGT-004 |
| E54 | Sneak with an empty main hand and dirt in the off hand. Send hand 0 on a chest. Without sneak, send hand 1 with dirt on a chest. | PLC-INT-002, PLC-INT-006 |
| E55 | In spectator mode, send a request on a nether portal and on an end portal. | PLC-INT-005 |
| E56 | Hold glow berries. Send a request on stone with no space above. | PLC-INV-009 |
| E57 | Put each entity at the centre of the target position, and send a request. Read the entity position with `data get entity` before the request. | U13, PLC-ENT-001 |
