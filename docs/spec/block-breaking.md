# Block breaking: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. The captured results of section 11 come
from `26.4-snapshot-4`. In `26.4-snapshot-4` and `26.4-snapshot-5`, the progress of a delayed break
did not grow (BRK-SRV-009). Results R32, R40 and R45 show that older behaviour where they state it.

This document states how a player breaks a block in creative mode and in survival mode, and what
the server sends for it. It states only what a client, the server console and the server files
show. It does not prescribe an architecture. Most rules come from a vanilla server in offline mode.
A rule with a `source_suggested` label comes from the reference implementation. Section 10 gives the
experiments, and section 11 gives the captured results.

Scope: creative mode and survival mode, for a client of Java Edition. Section 7 states survival
mode. This document does not state block placement, the loot table of each block, or the hit
reactions of blocks.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**, **VarInt**,
  **offline mode**: as `login.md`, section 0.1, defines them.
- **Breaker**: the player whose client sends the `player_action` packet.
- **Observer**: another player connected to the same dimension.
- **Action**: the first field of the `player_action` packet.
- **Start action**: an action with the value 0.
- **Sequence**: the last field of the `player_action` packet.
- **Acknowledgement**: one clientbound `block_changed_ack` packet.
- **Block cube**: the cube from the block position `(x, y, z)` to `(x + 1, y + 1, z + 1)`.
- **Block corner**: the point `(x, y, z)` of the block position.
- **Eye position**: the position of the player, with 1.62 added to `y`.
- **Break**: the server changes the block at the position to air.
- **Refusal**: the block at the position does not change.
- **Abort action**, **finish action**, **face change action**: an action with the value 2, 3 and 1.
- **Dig**: the record that a start action makes in survival mode (BRK-SRV-001). A dig has a
  position, a face and a start tick. A player has at most one dig.
- **Tick count**: the number of server ticks that started after the server handled the start action
  of the dig. The first tick after the start has the tick count 1.
- **Rate**: the progress of one tick for a block, a player and the inputs of one tick (BRK-PRG-002).
- **Progress**: a number from 0 that grows while the dig continues (BRK-PRG-001). At 1 the block can
  break.
- **Stage**: `floor(progress × 10)`.
- **Harvest**: the player can harvest a block when the block does not need the correct tool for
  drops, or when the main-hand item is a correct tool for the block (BRK-PRG-003).
- **Survival break**: a break that a dig, or a start action with a rate of 1 or more, causes.
- **Main-hand item**: the item stack in the selected hotbar slot.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 11.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a captured result in section 11.
- `source_suggested`: taken from the reference implementation, with no captured result. Section 10
  attaches an experiment.
- `unresolved`: not decided. Section 9 collects these items.

All the results come from one vanilla server and scripted clients on one machine. A rule that rests
on one observation states this.

### 0.3 ID prefix

Each rule has an ID with the prefix `BRK`.

## 1. The request

**BRK-REQ-001** (`black_box_confirmed`, wire). The `player_action` packet has these fields, in this
order: action (VarInt), position (long), face (byte), sequence (VarInt). See result R1.

**BRK-REQ-002** (`black_box_confirmed`, semantic). In creative mode, only the start action can break
a block. See results R1 and R2.

| Action | Result in creative mode |
|--------|-------------------------|
| 0 | The block breaks, unless section 3 refuses it. |
| 1 | An acknowledgement only. |
| 2 | An acknowledgement to the breaker. An observer near the block gets `block_destruction` with progress 255 (BRK-REQ-006). |
| 3 | An acknowledgement only. |

Actions 4 to 8 do not act on a block. This document does not state them.

**BRK-REQ-003** (`black_box_confirmed`, wire). An action of 9 closes the connection, because the
packet does not decode. The block does not change. See result R3. This rule rests on one observation.

**BRK-REQ-004** (`black_box_confirmed`, semantic). The face has no effect on a break. The values 0, 1,
2, 5 and 9 all break the block. See results R1 and R4.

**BRK-REQ-005** (`source_suggested`, semantic). The server ignores a request, and sends no
acknowledgement, until the client has loaded. The client has loaded after it sends `player_loaded`, or
60 ticks after the player joins or respawns. See experiment E28.

**BRK-REQ-006** (`source_suggested`, semantic). For action 2, an observer gets `block_destruction` when
the distance from its position to the block corner is less than 32. The breaker does not get it. See
experiment E29.

The reference implementation keeps the position of the last break progress, which is `(0, 0, 0)` at
first. For action 2 at another position, it also sends `block_destruction` with progress 255 for the
kept position. Action 3 at the kept position can break the block there. This document leaves these two
results unspecified.

## 2. A break

**BRK-BRK-001** (`black_box_confirmed`, semantic). The breaker gets the new block state and then one
acknowledgement. See result R1.

**BRK-BRK-002** (`black_box_confirmed`, wire). The server sends `block_update` when one block changes
in a section. It sends `section_blocks_update` when more than one block in a section changes together.
See results R1, R6 and R12.

**BRK-BRK-003** (`black_box_confirmed`, semantic). An observer gets `level_event` with type 2001 and
the state id of the old block as its data, and then the new block state. An observer does not get an
acknowledgement. See result R1.

**BRK-BRK-004** (`black_box_confirmed`, semantic). The breaker does not get `level_event` for a break
of one block. See result R1. The breaker does get it for the parts of section 4, as results R12 and
R13 show.

**BRK-BRK-005** (`black_box_confirmed`, semantic). A creative break does not send
`block_destruction`. See result R1.

**BRK-BRK-006** (`source_suggested_black_box_confirmed`, semantic). An observer in the same dimension
gets the `level_event` when the distance from its position to the block corner is less than 64. An
observer gets the new block state while its client holds the chunk. See result R10.

**BRK-BRK-007** (`source_suggested_black_box_confirmed`, semantic). The server handles a request
between two ticks. In the next tick, it sends the changed block states, and then the acknowledgement.
The server sends the `block_update` of a refusal when it handles the request. In result R9, the largest
delay to the first packet is 49.9 ms.

**BRK-BRK-008** (`black_box_confirmed`, semantic). Bedrock, barrier, end portal frame, structure void,
light, moving piston, nether portal and end portal break like any other block. See result R5.

**BRK-BRK-009** (`source_suggested_black_box_confirmed`, semantic). A start action on fire or soul fire
extinguishes it. The breaker and each observer get `level_event` with type 1009, and then the new block
state. They do not get type 2001. See result R5. Result R5 tests only fire.

## 3. Refusals

### 3.1 Reach

**BRK-RCH-001** (`source_suggested_black_box_confirmed`, semantic). A start action is refused when the
distance from the eye position to the nearest point of the block cube is 6.0 or more. A distance less
than 6.0 is in reach. See results R7 and R8. The breaker gets one acknowledgement and no block state. An
observer gets nothing.

The limit is the value of the `minecraft:block_interaction_range` attribute, plus 1.0. The attribute is
4.5 by default, and creative mode adds 0.5 to it.

**BRK-RCH-002** (`black_box_confirmed`, semantic). The distance uses the block cube, and not the
shape of the block. A bottom slab has the same limit as a full block when the breaker is above it. See
result R8.

**BRK-RCH-003** (`source_suggested_black_box_confirmed`, semantic). The eye position is the position of
the player plus the eye height of its current pose. The eye height is 1.62 when the player stands,
1.27 when it crouches, and 0.4 when it crawls or swims. The player crouches while its last
`player_input` has the sneak flag and it does not fly. It crawls when a block above it leaves no room
to stand or crouch. See result R47. The result bounds the crouch height to 0.85 to 1.43, and the crawl
height to 0.19 to 0.85.

### 3.2 Refusals that send the block state

**BRK-REF-001** (`black_box_confirmed`, semantic). In each case of this table, the breaker gets
`block_update` with the current state of the block, and then one acknowledgement. An observer gets
nothing where the table states it.

| Case | Result | Observer |
|------|--------|----------|
| The selected hotbar slot holds an item that cannot break blocks (BRK-REF-002) | R11 | Gets nothing, R11 |
| The player is in adventure mode or spectator mode | R14 | Gets nothing, R14 |
| The position is outside the world border | R15 | Gets nothing, BRK-ORD-001 |
| The position is above the height limit, and in reach | R15 | Gets nothing, BRK-ORD-001 |

For the position above the height limit, the state id in the packet is 18655, and not 0.

**BRK-REF-002** (`source_suggested_black_box_confirmed`, semantic). The item in the selected hotbar slot
refuses a break in two cases:

- The `minecraft:tool` component of the item has `can_destroy_blocks_in_creative` set to false.
- The item is the debug stick.

In result R11, all swords, the trident and the mace have this component value. An axe, a hoe, shears, a
spear, a pickaxe, a stick and an empty hand do not refuse a break. Only the selected hotbar slot counts.
The other hotbar slots and the off hand have no effect. See result R11.

For an operator of level 2 or higher, the debug stick also selects a block property and sends a
message. This document does not state that behaviour.

**BRK-REF-003** (`black_box_confirmed`, semantic). A command block, a structure block and a jigsaw
block refuse a break when the player is not an operator. An operator breaks them. See result R16. In a
refusal, the breaker gets `block_update` with the current state two times. For a structure block and a
jigsaw block, the breaker then gets `block_entity_data`. The acknowledgement comes last. BRK-REF-004
states the operator level.

**BRK-REF-004** (`source_suggested`, semantic). The refusal of BRK-REF-003 also applies to a chain
command block, a repeating command block, a test block and a test instance block. A player with an
operator level of 2 or higher breaks these blocks. The server sends the second `block_update`, and the
`block_entity_data`, to each player whose client holds the chunk. The observer is one of these players.
A test block and a test instance block also send `block_entity_data`. See experiment E30.

### 3.3 Refusals that send only an acknowledgement

**BRK-ACK-001** (`black_box_confirmed`, semantic). In each case of this table, the breaker gets one
acknowledgement and no block state.

| Case | Result |
|------|--------|
| The block is out of reach (BRK-RCH-001) | R7 |
| The position is air | R17 |
| The position holds water or lava, at a level from 0 to 8 | R18, R25 |
| The position is below the height limit | R15 |

**BRK-ACK-002** (`black_box_confirmed`, semantic). For a start action on air, an observer gets
`level_event` with type 2001 and data 0. An observer gets no block state. See result R17. This rule
rests on one observation.

**BRK-ACK-003** (`source_suggested_black_box_confirmed`, semantic). A block of one part that holds a
water source changes to a water source. The breaker gets `block_update` with the state of a water
source, and then one acknowledgement. An observer gets `level_event` with type 2001 and the state id of
the old block, and then the same block state. No item drops. See results R19 and R26. The tested blocks
are in R26. A block of the same type with no water changes to air. Each block type rests on one
observation. Section 4 states the packets for tall seagrass.

**BRK-ACK-004** (`black_box_confirmed`, semantic). For a start action on water or lava, at a level from
0 to 8, an observer gets `level_event` with type 2001 and the state id of the fluid. An observer gets no
block state. The fluid stays. See result R25.

**BRK-ACK-005** (`source_suggested`, semantic). Water or lava at a level from 9 to 15 changes to the
same fluid at level 8. The breaker gets `block_update` with the new state, and then one acknowledgement.
An observer gets `level_event` with type 2001 and the old state id, and then the new block state. See
experiment E31.

### 3.4 Spawn protection

**BRK-SPN-001** (`source_suggested_black_box_confirmed`, semantic). A start action is refused when all of
these conditions are true. See results R20 and R21.

- The value of `spawn-protection` is more than 0.
- The block is in the dimension of the world spawn.
- At least one operator exists.
- The player is not an operator, at any operator level.
- The larger of the two distances on the `x` axis and on the `z` axis, from the world spawn block to the
  block position, is at most the value of `spawn-protection`.

The `y` coordinate has no effect. An operator breaks the block, and the break is normal. A server with
no operator does not refuse, as result R21 shows. Results R20 and R21 test only the `x` axis, with
`spawn-protection=16`. Experiment E33 tests the other conditions.

**BRK-SPN-002** (`black_box_confirmed`, wire). In this refusal, the breaker gets `system_chat` in the
overlay position, and then one acknowledgement. The breaker gets no block state. The message is the
translation key `build.spawn_protection` in red, with the argument `x, y, z`. See result R20. An
observer gets nothing.

### 3.5 The order of the checks

**BRK-ORD-001** (`source_suggested`, semantic). The server applies the checks in this order. The first
check that refuses the request sets the result.

1. Reach (BRK-RCH-001), for each action.
2. A position above the height limit (BRK-REF-001), for each action.
3. Spawn protection (BRK-SPN-001), for the start action.
4. The world border (BRK-REF-001), for the start action.
5. In creative mode, the held item (BRK-REF-002), and then the blocks of BRK-REF-003. In adventure
   mode or spectator mode, the game mode (BRK-REF-001).

Thus actions 1, 2 and 3 above the height limit also get `block_update`. An item that refuses a break
also refuses on air, on a fluid and below the height limit, and sends `block_update`. Only the refusal
of BRK-REF-003 sends a packet to an observer. See experiment E32.

**BRK-ORD-002** (`source_suggested`, semantic). Steps 2 to 4 do not refuse a position below the height
limit. When no check refuses it, the break changes nothing, so the breaker gets only the
acknowledgement. An observer gets
`level_event` with type 2001 and data 18655, the state id of void air. See experiment E32.

## 4. Blocks that have more than one part

**BRK-PRT-001** (`black_box_confirmed`, semantic). A break of one part of a door, a bed or a tall plant
also removes the other part. Both changes reach the breaker in one `section_blocks_update` when they
are in the same section. The acknowledgement comes last. See result R12.

**BRK-PRT-002** (`black_box_confirmed`, semantic). One part is the main part: the upper part of a door
or a tall plant, and the foot of a bed. The breaker gets one `level_event` with type 2001 at the main
part, whichever part was hit, before the block state. See results R12 and R27.

**BRK-PRT-004** (`black_box_confirmed`, semantic). An observer gets `level_event` with type 2001 for the
hit part and then for the other part. When the hit part is the main part, a third packet repeats it.
The data of each packet is the state of the part at its position. See results R12 and R27.

**BRK-PRT-003** (`black_box_confirmed`, semantic). A door, a bed and a tall plant drop no item in a
creative break. See result R12.

**BRK-PRT-005** (`black_box_confirmed`, semantic). A break of an extended piston also removes its
head: the breaker gets `level_event` with type 2001 at the head, and then the block states of both
parts. An observer gets the `level_event` for the piston and then for the head. No item drops. See
result R49. The result tests a piston and a sticky piston that face up.

**BRK-PRT-007** (`source_suggested`, semantic). A break of a piston head also removes its piston when
all of these conditions are true:

- The piston is behind the head, on the axis of the `facing` property of the head.
- The piston has `extended=true` and the same `facing` value as the head.
- The piston is a sticky piston for a head with `type=sticky`, and a piston for `type=normal`.

In other cases, the break removes only the head. In result R49, the piston stayed. That run does not
record the state of the head, so it does not decide this rule. See experiment E64.

**BRK-PRT-006** (`black_box_confirmed`, semantic). A break of one half of a double chest changes the
other half to a single chest. Both changes reach the breaker in one block update of the section. See
result R49.

## 5. Drops

**BRK-DRP-001** (`black_box_confirmed`, semantic). A creative break of a block drops no item for the
block itself. See results R13 and R22. Stone, an oak slab, and an empty chest, barrel, shulker box,
hopper and furnace drop nothing.

**BRK-DRP-002** (`black_box_confirmed`, semantic). The contents of a container drop. One item entity
appears for each stack, at a random position near the block. See result R22. The tested containers are a
chest, a barrel and a furnace.

**BRK-DRP-003** (`black_box_confirmed`, semantic). A shulker box with contents drops one item: the
shulker box, which holds the contents. See result R22. This rule rests on one observation.

**BRK-DRP-004** (`black_box_confirmed`, semantic). A block that the break removes because its support
is gone drops its item, in creative mode too. A torch, redstone wire and a sign on the block each drop
one item. See result R13. The item entity reaches the client before the block state.

**BRK-DRP-005** (`black_box_confirmed`, semantic). The order of packets to the breaker for such a break
is: `level_event` for the removed block, where the block has one, then `add_entity`, then
`section_blocks_update`, then the acknowledgement. See result R13. Redstone wire sends no `level_event`.

**BRK-DRP-006** (`black_box_confirmed`, semantic). A block that falls does so after the break. For sand
on the block, the server sends the acknowledgement first. About one tick later, it sends `add_entity`
and the new block state of the sand. See result R23. `block-simulation.md` states how neighbours update.

**BRK-DRP-007** (`black_box_confirmed`, semantic). In a creative break, these blocks drop the items
that they hold, and not the block item. The breaker and each observer get `add_entity` for each item
before the block state. See result R49.

| Block | Drop |
|-------|------|
| jukebox with a disc | the disc. The breaker and each observer also get `level_event` with type 1011 and data 0. |
| lectern with a book | the book |
| chiseled bookshelf | each book |
| campfire | each item on it |
| decorated pot | the item in it, and not the pot |

A flower pot with a plant drops nothing. Ice changes to air, and not to water. TNT does not ignite.

**BRK-DRP-008** (`black_box_confirmed`, semantic). In a creative break, a beehive or a bee nest with
bees or with a honey level more than 0 drops one item of the block. The item holds the bees and the
honey level. An empty beehive drops nothing. For the observer, the `add_entity` comes before the
`level_event` with type 2001. See result R49.

**BRK-DRP-009** (`black_box_confirmed`, semantic). A cactus or sugar cane on the broken block breaks in
the same tick, as in BRK-DRP-004, and drops its item. For kelp, the kelp plant above the broken block
breaks in the same tick, and the top kelp one tick later. Each part above the broken block drops one
kelp item. The positions of the kelp change to water. See result R49.

## 6. Several requests and the sequence

**BRK-SEQ-001** (`black_box_confirmed`, semantic). Requests that the server handles between two ticks
get one acknowledgement (BRK-BRK-007). It carries the highest sequence of those requests, whichever
came last. See result R6.

**BRK-SEQ-002** (`black_box_confirmed`, semantic). Requests about 200 ms apart each get their own
acknowledgement. See result R9.

**BRK-SEQ-003** (`black_box_confirmed`, semantic). The three actions 0, 3 and 2 in one tick break the
block and get one acknowledgement with the highest sequence. See result R2. The same request three
times in one tick sends one block state and one acknowledgement. See result R6.

**BRK-SEQ-004** (`black_box_confirmed`, wire). The sequences 0, 1, 2147483646 and 2147483647 are
valid. See result R24.

**BRK-SEQ-005** (`black_box_confirmed`, semantic). A negative sequence closes the connection with the
translation key `disconnect.packetError`. The block breaks before the connection closes. See result
R24. This rule rests on one observation.

**BRK-SEQ-006** (`black_box_confirmed`, semantic). When breaks in one tick change blocks in more than
one section, the breaker gets the packet of BRK-BRK-002 for each section, and then one
acknowledgement. The order of the sections is not specified. An observer gets the `level_event` of
each break in the order of the requests, and then the block states. See result R48.

## 7. Survival mode

The rules of this section apply in survival mode. The checks of section 3 apply to each action
first, in the order of BRK-ORD-001. BRK-SRV-016 states one exception. In adventure mode and in
spectator mode, BRK-REF-001 refuses the start action.

These rules state Java Edition. The decisions of issue #474 for Bedrock Edition apply at the Bedrock
edge, and not in these rules. For example, Bedrock Edition keeps the progress when the main-hand
item changes, and Java Edition restarts it (BRK-ITM-001).

### 7.1 Dependencies

Some rules need parts of the server that do not exist yet. An implementation can build the other
rules now. Without an inventory, the main-hand item is empty.

| Rules | Needs |
|-------|-------|
| BRK-SRV-001 to BRK-SRV-016, BRK-PRG-001, BRK-PRG-002 for an empty hand | nothing more |
| The item speed and the correct tool in BRK-PRG-002 and BRK-PRG-003, BRK-ITM-002, BRK-DMG-001 to BRK-DMG-003 | the player inventory, issue #521 |
| The effects and the attributes in BRK-PRG-002 | the effects and the attributes of a player |
| BRK-LOOT-001, BRK-LOOT-003 to BRK-LOOT-006 | the item entities, issue #522, the loot tables of `block-loot.md`, issue #72, and the block drops, issue #77 |
| BRK-LOOT-002 | the food of a player |

### 7.2 The dig

**BRK-SRV-001** (`black_box_confirmed`, semantic). A start action on a block with a rate less than 1
starts a dig at the position, with the face of the request. The breaker gets one acknowledgement and
no block state. Each observer of BRK-SRV-004 gets `block_destruction` with the entity id of the
breaker, the position and the stage `floor(rate × 10)`. See results R28 and R30.

**BRK-SRV-002** (`black_box_confirmed`, semantic). A start action on a block with a rate of 1 or
more breaks the block at once. The packets are the packets of a finish (BRK-SRV-008), but the
observer gets no `block_destruction`. A block with a hardness of 0 gives this result with each
main-hand item. See result R30.

**BRK-SRV-003** (`black_box_confirmed`, semantic). A start action at a new position while a dig
exists replaces the dig. The breaker gets `block_update` with the current state at the position of
the old dig, and then the acknowledgement. The observer gets `block_destruction` at the new
position, and the vanilla client moves the crack there (CBK-DST-002). See result R33.

**BRK-SRV-004** (`black_box_confirmed`, semantic). While a dig exists, and the block at its position
is not air, the server does these steps in each tick:

1. Calculate the progress (BRK-PRG-001).
2. If the stage is not the stage that the server sent last for the dig, send `block_destruction`
   with the new stage. The receivers are the observers at a distance less than 32 from the block
   corner. The breaker does not get it.
3. If the progress is 1 or less, send `level_event` to the breaker and to each observer at a
   distance less than 64 from the block corner, as in BRK-BRK-006. The type is 2020 when the tick
   count is a multiple of 4, and otherwise 2019. The data is the face of the dig.

In the first tick, the `level_event` comes before the acknowledgement of the start. See results R28,
R38 and R42.

**BRK-SRV-005** (`black_box_confirmed`, semantic). The stage of BRK-SRV-004 has no limit of 9. When the
progress is more than 1, the observer gets the stages 10, 11 and more until the dig ends. The vanilla
client removes the crack for such a stage (CBK-DST-002). The packet carries the stage modulo 256. Thus
after 249 comes 0, and a stage of 255 removes the crack as an abort does. See results R28 and R39.

**BRK-SRV-006** (`black_box_confirmed`, semantic). An abort action ends the dig. The breaker gets one
acknowledgement with the sequence of the request. The vanilla client sends the sequence 0
(CBK-SRV-006). Each observer gets `block_destruction` with the progress 255 at the position of the
request. When the dig has another position, the observer first gets the same packet for the position
of the dig, and the dig ends too. With no dig, the breaker gets the acknowledgement, and the observer
gets the packet for the position of the request. See results R33 and R41. Section 8, entry D2, records
a packet that the reference also sends with no dig.

**BRK-SRV-007** (`black_box_confirmed`, semantic). A face change action changes the face of the dig.
The position of the request has no effect. The breaker gets one acknowledgement. The progress does not
change. From the next tick, the data of the `level_event` of BRK-SRV-004 is the new face. With no dig,
the breaker gets only the acknowledgement. See results R33 and R41.

**BRK-SRV-008** (`black_box_confirmed`, semantic). A finish action at the position of the dig breaks
the block when the progress of the last tick is 0.7 or more. The dig ends. The breaker gets these
packets in one tick, in this order:

1. `add_entity` for each item entity and each experience orb that drops (BRK-LOOT-001).
2. `block_update` with the new state.
3. `container_set_slot` for the main-hand slot, when the item takes damage (BRK-DMG-001).
4. The acknowledgement.

An observer gets `block_destruction` with the progress 255, `level_event` with type 2001 and the
state id of the old block, `add_entity` for each drop, and then `block_update`. See results R32 and
R37.

**BRK-SRV-009** (`source_suggested`, semantic). A finish action at the position of the dig, with a
progress less than 0.7, does not break the block. The breaker gets one acknowledgement and no block
state. The dig changes to a delayed break with the same position and the same start tick. In each
tick, the server does steps 1 and 2 of BRK-SRV-004 for the delayed break. At a progress of 1 or more,
the block breaks with the packets of BRK-SRV-008, without the acknowledgement. The observer gets no
`block_destruction` with the progress 255 for this break. The stage of step 2 is then 10 or more, and
the vanilla client removes the crack for it (CBK-DST-002).

When the state of the block at the position changes, the delayed break ends. A player has at most one
delayed break. While it exists, a finish action with a progress less than 0.7 gets only the
acknowledgement, and the dig continues. A start action does not end the delayed break. The new dig
and the delayed break then both do their steps in each tick. Section 8, entries D3 and D5, record
the reference behaviour. See experiment E45 and result R40.

**BRK-SRV-010** (`black_box_confirmed`, semantic). In each of these cases, a finish action gets only
one acknowledgement, and the block stays:

- The player has no dig. See result R33. Section 8, entry D2, records a case where the reference
  breaks the block.
- The position of the request is not the position of the dig. The dig continues. See result R33.
- The request is out of reach (BRK-SRV-012). The dig continues. See result R36.

**BRK-SRV-011** (`black_box_confirmed`, semantic). A block with a hardness of -1 has a rate of 0. A
start action on such a block starts a dig with the stage 0, and the stage does not change. A finish
action gets only one acknowledgement, and makes no delayed break. See result R34. Section 8, entry D3,
records the reference behaviour.

**BRK-SRV-012** (`black_box_confirmed`, semantic). The reach limit of BRK-RCH-001 is the attribute
`minecraft:block_interaction_range` plus 1. In survival mode, the default attribute is 4.5, so the
default limit is 5.5. Creative mode adds 0.5 to the attribute, which gives the 6.0 of BRK-RCH-001.
Only the default attribute was tested. A start action out of reach gets one
acknowledgement, and the observer gets nothing. The limit applies to the finish action too. See
result R36.

**BRK-SRV-013** (`black_box_confirmed`, semantic). When the block at the position of a dig becomes
air, the dig ends in the next tick. Each observer of BRK-SRV-004 gets `block_destruction` with the
progress 255. The breaker gets nothing more. When the block becomes another state that is not air,
the dig continues, and the rate of each later tick uses the new state. See result R40. Section 8,
entry D2, records what a later finish action does in the reference.

**BRK-SRV-014** (`source_suggested`, semantic). A start action on air, in reach, starts a dig at the
position. The breaker gets one acknowledgement. Each observer of BRK-SRV-004 gets
`block_destruction` with the stage 10. The observer gets no `level_event` with type 2001. The dig
ends in the next tick, as BRK-SRV-013 states. BRK-ACK-002 applies only in creative mode. See
experiment E59.

**BRK-SRV-015** (`source_suggested`, semantic). A start action at the position of the dig, with a
rate less than 1, also replaces the dig, as in BRK-SRV-003. The breaker gets `block_update` with the
current state at that position, and then the acknowledgement. The new dig has the tick count 0. A
start action that breaks a block at once (BRK-SRV-002) does not end a dig at another position, and
sends no `block_update` for it. See experiment E60.

**BRK-SRV-016** (`source_suggested`, semantic). An abort action out of reach ends the dig when a dig
exists. The observer gets the packets of BRK-SRV-006. With no dig, an abort action out of reach gets
only the acknowledgement, and the observer gets nothing. The vanilla client sends such an abort
action when the player moves away from the block. See experiment E61.

### 7.3 Progress

**BRK-PRG-001** (`source_suggested`, client agreement). The progress at the tick count `c` is the
sum of the rates of the ticks from 0 to `c`, added one tick at a time in 32-bit floating point. Tick 0
is the start action. Each rate uses the inputs of its own tick. The vanilla client adds the rate of
each tick in the same way (CBK-SRV-002, CBK-SRV-003). The reference server calculates a product
instead, and results R28 and R29 confirm that product. Section 8, entry D4, records the difference.

For a finish action, the progress is the progress of the last tick before the request. Thus the
first tick after the start has the progress `2 × rate`.

**BRK-PRG-002** (`black_box_confirmed`, semantic). The rate is `S / H / F`:

- `H` is the hardness of the block state.
- `F` is 30 when the player can harvest the block (BRK-PRG-003), and 100 when it cannot.
- `S` is the break speed of the player.

`S` starts with the speed of the main-hand item for the block. The `minecraft:tool` component of the
item gives this speed: the speed of the first rule that matches the block and has a speed, or else
the default speed of the component. An item with no such component, and an empty hand, have the
speed 1. When this speed is more than 1, add the attribute `minecraft:mining_efficiency`. The
Efficiency enchantment of level `n` sets this attribute to `n × n + 1`. Then apply these factors in
this order:

1. Haste or Conduit Power with the amplifier `a`: multiply by `1 + (a + 1) × 0.2`. When the player
   has both effects, `a` is the larger amplifier. The amplifiers do not add.
2. Mining Fatigue with the amplifier `a`: multiply by `0.3^(a + 1)`. This applies to each amplifier,
   also 4 and more.
3. Multiply by the attribute `minecraft:block_break_speed`.
4. When the eyes of the player are in water: multiply by the attribute
   `minecraft:submerged_mining_speed`. Its default is 0.2. Aqua Affinity sets it to 1. Feet in water
   with the eyes out of water have no effect.
5. When the player is not on the ground: divide by 5. The server uses the on-ground flag of the last
   movement packet of the client.

Results R29, R31 and R43 test each factor. CBK-SRV-003 states the same rate for the client.

**BRK-PRG-003** (`black_box_confirmed`, semantic). The block definition states if a block needs the
correct tool for drops. A main-hand item is a correct tool for a block when the first rule of its
`minecraft:tool` component that matches the block and has a value for `correct_for_drops` has the
value true. Thus a wooden pickaxe has the speed 2 on iron ore, but is not a correct tool for it. See
results R29 and R37.

### 7.4 The held item

**BRK-ITM-001** (`source_suggested`, semantic). For Java Edition, a change of the main-hand item
restarts the progress (issue #474). The vanilla client sends an abort action and then a start action
for this (CBK-SRV-001). The start action makes a new dig with the tick count 0 (BRK-SRV-001). See
experiment E47.

**BRK-ITM-002** (`black_box_confirmed`, semantic). When the main-hand item changes and no start action
follows, the dig continues with the same start tick. From the next tick, the rate uses the new item.
See result R35. Section 8, entry D4, records that the reference also applies the new rate to the
earlier ticks.

### 7.5 Tool damage

**BRK-DMG-001** (`black_box_confirmed`, semantic). After a survival break of a block with a hardness
that is not 0, the main-hand item takes damage. The amount is the value of `damage_per_block` in the
`minecraft:tool` component of the item. An item with no such component takes no damage. The damage
does not need a harvest. A block with a hardness of 0 causes no damage. See result R37.

**BRK-DMG-002** (`black_box_confirmed`, semantic). When the damage of the item becomes equal to its
maximum damage or more, the item breaks, and the main-hand slot becomes empty. The breaker and each
observer get `entity_event` with the entity id of the breaker and the status 47, before the `add_entity` of
BRK-SRV-008. The breaker gets `container_set_slot` with an empty item for the slot. The server sends
no sound packet: the vanilla client plays the sound for the status 47. See results R37 and R44.

**BRK-DMG-003** (`source_suggested_black_box_confirmed`, semantic). When the item has the Unbreaking
enchantment of level `n`, each point of damage of BRK-DMG-001 applies with the chance `1 / (n + 1)`.
See result R44.

### 7.6 Drops and exhaustion

**BRK-LOOT-001** (`black_box_confirmed`, semantic). A survival break with a harvest drops the items of
the loot table of the block. The loot table uses the main-hand item, for example for Silk Touch. Each
item stack becomes one item entity at a random position. The `x` and `z` coordinates are from 0.25
to 0.75 inside the block cube. The `y` coordinate is from 0.125 to 0.625 above the bottom of the
cube: the centre minus half the item height of 0.25, then plus or minus 0.25. The observed `y`
offsets were from 0.13 to 0.57. A block that gives experience also drops experience orbs at the centre
of the block, after the items. A survival break without a harvest drops nothing from the loot table.
See result R37.

**BRK-LOOT-002** (`black_box_confirmed`, semantic). A survival break with a harvest adds 0.005 to the
food exhaustion of the breaker. A survival break without a harvest adds nothing. See result R37.

**BRK-LOOT-003** (`black_box_confirmed`, semantic). In a survival break, a container that keeps its
contents in the block drops each content stack as an item entity, and the block drops its own item
from its loot table. A shulker box drops only its own item, with the contents in the item. A block
that loses its support drops its items in the same tick, as in BRK-DRP-004 and BRK-DRP-005: the
breaker gets `level_event` with type 2001 for that block, then `add_entity` for its items, then for
the items of the broken block, then `section_blocks_update` with both changes. A block that falls
does so one tick later, as in BRK-DRP-006. See result R45.

**BRK-LOOT-004** (`black_box_confirmed`, semantic). A survival break with a harvest and without Silk
Touch drops the experience of this table. A range is a random integer from the first value to the
second value, both included. Other blocks drop no experience, for example iron ore, copper ore, gold
ore, their deepslate forms and ancient debris. BRK-LOOT-006 states one exception. A break with Silk
Touch or without a harvest drops no experience. The `minecraft:block_experience` effect of the Silk
Touch enchantment sets the experience to 0. An implementation must read this effect from the
enchantment of the main-hand item. See result R46.

| Block | Experience |
|-------|------------|
| coal ore, deepslate coal ore | 0 to 2 |
| nether gold ore | 0 to 1 |
| diamond ore, deepslate diamond ore, emerald ore, deepslate emerald ore | 3 to 7 |
| lapis ore, deepslate lapis ore, nether quartz ore | 2 to 5 |
| redstone ore, deepslate redstone ore, lit or not lit | 1 to 5 |
| spawner | 15 to 43 |
| sculk | 1 |
| sculk sensor, calibrated sculk sensor, sculk shrieker, sculk catalyst | 5 |

The server divides the
experience into orbs. Each orb takes the largest value of 37, 17, 7, 3 and 1 that is not more than
the remainder. For example, 5 gives the orbs 3, 1 and 1. The orb values above 37 are 73, 149, 307,
617, 1237 and 2477, and no block of this document gives them. The vanilla client gets the value of an
orb in `set_entity_data`, and the reference sends that packet two times for each orb.

**BRK-LOOT-005** (`source_suggested`, semantic). When the game rule `block_drops` is false, a
survival break drops no item from the loot table and no experience. The content stacks of a
container (BRK-LOOT-003) drop with each value of the game rule. They also drop in a survival break
without a harvest, for example a furnace with an empty hand. See experiment E62.

**BRK-LOOT-006** (`source_suggested`, semantic). A survival break of a creaking heart with the
property `natural=true` drops a random integer of experience from 20 to 24, both included. A harvest
and Silk Touch have no effect on this experience. The game rule `block_drops` applies, as in
BRK-LOOT-005. A creaking heart with `natural=false` drops no experience. See experiment E63.

## 8. Intentional deviations

**D2.**

- **Rule**: BRK-SRV-010.
- **Reference**: the reference keeps the position and the start tick of a dig after the dig ends. A
  finish action at that position with no new start uses them. The progress since the old start is
  large, so a new block at that position breaks at once, with drops and tool damage. This also
  occurs after the dig ends by an abort action at another position or because the block became air
  (result R40). An abort action with no dig also sends `block_destruction` with the progress 255 for
  the kept position (result R41).
- **Visible to the client**: yes. A finish action with no start breaks the block.
- **Test**: a differential test must not send a finish action without a start action.

**D3.**

- **Rule**: BRK-SRV-009 and BRK-SRV-011.
- **Reference**: a delayed break ends only when it breaks the block or when the block becomes air. A
  finish action on a block with a hardness of -1 thus makes a delayed break that never ends. When
  another block that is not air is set at the position later, the delayed break continues with that
  block. The observer gets a stage for it, and the block can break with no action of the player.
  Result R40 shows this for obsidian.
- **Visible to the client**: yes.
- **Test**: a differential test must not send a finish action on a block with a hardness of -1, and
  must not change the block of a delayed break.

**D5.**

- **Rule**: BRK-SRV-009.
- **Reference**: while a delayed break exists, a dig does not do the steps of BRK-SRV-004. The
  observer gets only the stage of the start action, and nobody gets `level_event`. A finish action on
  the dig still breaks the block, because the reference calculates the progress from the start tick
  (entry D4). A start action and an abort action do not end the delayed break. See result R40.
- **Visible to the client**: yes. Other players see no crack for the new dig.
- **Test**: a differential test must not start a dig while a delayed break exists.

**D4.**

- **Rule**: BRK-PRG-001 and BRK-ITM-002.
- **Reference**: the reference multiplies the rate of the current tick by `c + 1`, as a product in
  32-bit floating point. With a rate that does not change, the product and the sum of BRK-PRG-001
  can give a stage one tick apart. For stone with an empty hand, the product reaches the stage 2 at
  the tick count 29, and the sum at 30. A change of an input changes the progress of the earlier
  ticks too. In result R35, the stage went from 4 to 0
  when the item changed from a wooden pickaxe to the hand, and to 7 when it changed back. The vanilla
  client adds the rate of each tick, so the reference server and the vanilla client disagree when an
  input changes.
- **Visible to the client**: yes. The stage of an observer jumps, and a finish action that the client
  sends can get a delayed break.
- **Test**: a differential test compares the progress only with inputs that do not change, and
  accepts a stage one tick later than the reference.

## 9. Unresolved

- **U7**: a position in reach in a chunk that is not loaded. The only tested position was also out of
  reach.
- **U9**: the eye position for the poses of BRK-RCH-003 that no experiment tested: sleeping and
  flying with elytra.
- **U13**: blocks that act differently in a creative break and that result R49 does not test.
- **U22**: the packet order of a delayed break (BRK-SRV-009). No result exists for the game version
  of this document. In result R45, on `26.4-snapshot-4`, the breaker got the acknowledgement one tick
  before the `block_update`. Experiment E45 can show the order.

## 10. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`, `level-type=minecraft:flat` and `network-compression-threshold=-1`. Use
`spawn-protection=0`, except where a step states another value. Set the game rule `spawn_mobs` to
false. Connect one scripted client as the breaker and one as the observer, both in creative mode.
Before each step, set the block with `setblock`, and read the packets that arrive after the step.

| Experiment | Steps | Result |
|------------|-------|--------|
| E1 | Send the start action on stone 2 blocks away, with sequence 5. | R1 |
| E2 | Send each of the actions 1, 2 and 3 on stone. Then send 0, 3 and 2 in one burst. | R2 |
| E3 | Send action 9. | R3 |
| E4 | Send the start action with the faces 0, 2, 5 and 9. | R4 |
| E5 | Send the start action on each block of BRK-BRK-008, and on fire. | R5 |
| E6 | Send two start actions in one burst, with the sequences in order and then in the other order. Send one request three times. | R6 |
| E7 | Move the player along `x` from a block in the row of the eye position, and from a block in a higher row, then on a diagonal. Send the start action at each distance. | R7 |
| E8 | Put the player above a block, a bottom slab, at each distance. Send the start action. | R8 |
| E9 | Send two requests 200 ms apart. Measure the time to the first packet, 12 times. | R9 |
| E10 | Put the observer 20, 30, 45, 60, 63, 65 and 80 blocks away on `x`. Break a block. | R10 |
| E11 | Hold each item of R11 in the selected slot. Then hold a sword in another slot, and in the off hand. | R11 |
| E12 | Break the upper part of a door and of a tall plant, and the foot of a bed. | R12 |
| E27 | Hit each part of a door, a bed and the plants of R27, with both parts set without neighbour updates, in a stone wall. Repeat. | R27 |
| E13 | Put a torch, redstone wire and a sign on stone. Break the stone, with the player 4 blocks away. Read the item. | R13 |
| E14 | Send the start action in adventure mode and in spectator mode. | R14 |
| E15 | Set a world border that excludes a block. Stand near the height limits. Send the start action. | R15 |
| E16 | Break a command block, a structure block and a jigsaw block, without and with `op`. | R16 |
| E17 | Send the start action on air. | R17 |
| E18 | Wall a source of water and of lava in stone. Send the start action. | R18 |
| E19 | Break a waterlogged slab. | R19 |
| E20 | Use `spawn-protection=16` and `setworldspawn 0 -60 0`, with another player as an operator. Break blocks at `x` from 14 to 19 and from -14 to -18. | R20 |
| E21 | Do E20 with no operator. | R21 |
| E22 | Break each block of BRK-DRP-001. Break containers with contents. Read the item entities. | R22 |
| E23 | Put sand on stone. Break the stone. | R23 |
| E24 | Send the sequences 0, 1, 2147483646, 2147483647 and -1. | R24 |
| E25 | Wall a block position in stone. Set water and lava there, at the levels of R25. Wait until the breaker gets the `block_update`, and send the start action at once, before the fluid changes. Do each level that is not a source three times, and each source once. | R25 |
| E26 | Wall a block position in stone. Set each block of R26 there. Send the start action. Read the block and the item entities. For tall seagrass, hit each part. | R26 |
| E56 | Set stone at (6,-57,0). Put the breaker at the distances of R47 on `x` from the face of the block, and send the start action. Send `player_input` with the sneak flag and repeat. Clear the flag, set stone at (-1,-59,0) and (0,-59,0) above the breaker, and repeat. | R47 |
| E57 | Send two start actions in one burst on (-1,-60,2) and (0,-60,2), in two sections. Then do it on (2,-60,2) and (2,-60,3), in one section. | R48 |
| E58 | Set each block of R49, and send the start action on the block that R49 states. Read the blocks and the item entities. | R49 |

For experiments E34 to E44 and E48 to E55, put the breaker in survival mode, and keep the observer in creative mode.
Set the difficulty to peaceful, except in E43. Let the breaker send `move_player_status_only` each
50 ms, with the on-ground flag set, except where a step clears it. Give items with `item replace`
and effects with `effect give`. Before each step, clear the inventory, the effects, the item entities
and the experience orbs. Count the tick count from the acknowledgement of the start, or from the
`level_event` packets of BRK-SRV-004.

| Experiment | Steps | Result |
|------------|-------|--------|
| E34 | With an empty hand, send the start action on stone 2 blocks away. Wait 10 s. Send the finish action. | R28 |
| E35 | For each case of R29, send the start action and do not finish. Read the tick count of each stage of the observer. | R29 |
| E36 | Send the start action on short grass and on a torch with an empty hand. Send it on stone with the main-hand items of R30. | R30 |
| E37 | Do E35 on stone with a wooden pickaxe: with the on-ground flag clear, in water 4 blocks deep, and in that water with Aqua Affinity on the helmet. | R31 |
| E38 | With a wooden pickaxe, start a dig on stone. Send the finish action after the tick counts of R32. | R32 |
| E39 | Start a dig on stone. Send a face change action with face 3, an abort action, a start action on a second block, and a finish action on the second block. Then send a finish action on a block with no dig. | R33 |
| E40 | Start a dig on bedrock. After 30 ticks, send the finish action. | R34 |
| E41 | Start a dig on stone with a wooden pickaxe in slot 0. Change the selected slot with `set_carried_item`, with no new start action, as R35 states. | R35 |
| E42 | Put the eye position at the distances of R36 from the face of a stone block in the row of the eyes. Send the start action. Start a dig at 5.4, move the player to 5.6, and send the finish action. | R36 |
| E43 | For each case of R37, break the block with a start action and a finish action. Read the main-hand item, the item entities, the experience orbs and `foodExhaustionLevel`. | R37 |
| E44 | Put the observer 20, 31, 33 and 40 blocks from the block on `x`. Start a dig, and abort it after 0.6 s. | R38 |
| E48 | With a diamond pickaxe with Efficiency V, start a dig on stone. Do not finish. Read the stages for 4 s. | R39 |
| E49 | With a wooden pickaxe, start a dig on stone. After 6 ticks, set the block to air, to dirt or to obsidian. Read the packets. After air, set stone and send the finish action. After dirt, send the finish action. Then, 3 s after a new login, make a delayed break on obsidian with a finish action at the tick count 5. Start a dig on a second block, or start and abort a dig at the same position. Then set stone at the position of the delayed break. | R40 |
| E50 | Start a dig on stone. Send a face change action with face 3 at a second position. Send an abort action at the second position, then a finish action at the first position. Then send an abort action and a face change action with no dig. | R41 |
| E51 | Put the observer 48, 60, 64, 65 and 80 blocks from the block on `x`. Start a dig, and abort it after 0.8 s. | R42 |
| E52 | Do E35 for each case of R43. | R43 |
| E53 | Break stone 40 times at once with a netherite pickaxe with Efficiency V and Unbreaking III, with Haste amplifier 1. Do it 10 times without Unbreaking. Read the damage. Break stone with a wooden pickaxe with the damage 58, and read all the packets. | R44 |
| E54 | Break a chest with contents, a shulker box with contents, and stone with a torch, sand or the lower part of an oak door on it. | R45 |
| E55 | With Haste amplifier 50, break each block of R46 60 times, the deepslate forms included, with a netherite pickaxe with Efficiency V, or a netherite hoe with Efficiency V for the sculk blocks. Break diamond ore and sculk 10 times with Silk Touch, and diamond ore 10 times with a wooden pickaxe. Sum the values of the experience orbs, one value for each entity id. | R46 |

Experiments E28 to E33, E45 to E47 and E59 to E64 have no captured result. Nobody has run them yet.

| Experiment | Steps | Rule |
|------------|-------|------|
| E28 | Send the start action before the client sends `player_loaded`. Then send `player_loaded` and send the start action again. | BRK-REQ-005 |
| E29 | Put the observer 31 blocks and then 33 blocks from the block corner. Send action 2. | BRK-REQ-006 |
| E30 | Break each block of BRK-REF-004 without `op`, with operator level 1 and with operator level 2. Read the packets of the observer. | BRK-REF-004 |
| E31 | Set water and lava at the levels 9 and 15. Send the start action. | BRK-ACK-005 |
| E32 | Hold a diamond sword. Send the start action on air, on water and at `y=-65`. Send actions 1, 2 and 3 at `y=320`. With an empty hand, send the start action at `y=-65` and read the observer. | BRK-ORD-001, BRK-ORD-002 |
| E33 | Do E20 on the `z` axis, on a diagonal, with an operator of level 1, and in the nether. | BRK-SPN-001 |
| E45 | Start a dig on stone with an empty hand. Send the finish action at the tick count 5. Read the block, the stages and the packets of the break until the tick count 160. Repeat, and set the block to dirt at the tick count 20. Repeat with a second dig, and send a finish action on it at the tick count 5. | BRK-SRV-009 |
| E46 | On mcrs, change the main-hand item during a dig with no start action, as in E41. Read the stages. | BRK-PRG-001, BRK-ITM-002 |
| E47 | With a vanilla client and a proxy, start a dig on stone with a wooden pickaxe. Select another hotbar slot during the dig. Read the actions that the client sends. | BRK-ITM-001 |
| E59 | In survival mode, send the start action on air 2 blocks away. Read the packets of the observer for 3 ticks. | BRK-SRV-014 |
| E60 | Start a dig on stone. After 10 ticks, send the start action at the same position. Read the packets and the stages. Then start a dig on stone, and send the start action on short grass. Read the stages of the stone. | BRK-SRV-015 |
| E61 | Start a dig on stone at the distance 5.4. Move the player to 5.6. Send the abort action. Read the packets of the observer. Repeat with no dig. | BRK-SRV-016 |
| E62 | Set the game rule `block_drops` to false. Break stone and coal ore with a stone pickaxe, and a chest with contents. Then set the game rule to true, and break a furnace with contents with an empty hand. | BRK-LOOT-005 |
| E63 | Break a creaking heart with `natural=true` 20 times with an iron axe, with Silk Touch and without it. Break one with `natural=false`. Sum the values of the experience orbs. | BRK-LOOT-006 |
| E64 | Set a piston with `extended=true` and `facing=up`, and a piston head with `facing=up` and `type=normal` on it. Send the start action on the head. Repeat with a head with `facing=north`. Read the blocks. | BRK-PRT-007 |

Experiment E46 runs on mcrs, because the reference has the deviation D4.

## 11. Captured results

Each packet is shortened to its name and the fields that matter. A state id of 0 is air, and 1 is
stone.

**R1.**

```text
breaker:  block_update (2,-60,0) state 0
          block_changed_ack seq 5
observer: level_event type 2001 (2,-60,0) data 1
          block_update (2,-60,0) state 0
```

**R2.**

```text
action 1: block_changed_ack seq 7; block stays
action 2: block_changed_ack seq 7; block stays
          observer: block_destruction entity 1 (2,-60,0) progress 255
action 3: block_changed_ack seq 7; block stays
actions 0, 3, 2 with seq 8, 9, 10: block_update state 0; block_changed_ack seq 10
```

**R3.** The connection closes with `Failed to decode packet 'serverbound/minecraft:player_action'`.

**R4.** The block is air after each of the faces 0, 2, 5 and 9.

**R5.**

```text
bedrock, barrier, end_portal_frame, structure_void, light, moving_piston,
nether_portal, end_portal:
  block_update state 0; block_changed_ack
fire:
  breaker:  level_event type 1009 data 0; block_update state 0; block_changed_ack
  observer: level_event type 1009 data 0; block_update state 0
```

**R6.**

```text
(2,-60,0) seq 20, then (2,-60,1) seq 21:
  section_blocks_update with both at state 0; block_changed_ack seq 21
(2,-60,0) seq 31, then (2,-60,1) seq 30:
  section_blocks_update with both at state 0; block_changed_ack seq 31
the same request three times, seq 96:
  block_update state 0; block_changed_ack seq 96
```

**R7.** The distance is the distance on `x` from the eye position to the block cube.

| Distance | Result |
|----------|--------|
| 5.5, 5.75, 5.9, 5.95, 5.99 | `block_update`, `block_changed_ack`; the block is air |
| 6.0, 6.01, 6.05, 6.1, 6.25 | `block_changed_ack`; the block stays |

With a block in a higher row, a horizontal distance from the centre of 6.25 breaks and 6.5 does not.
On a diagonal with the centre on the line of the eye position, a distance from the centre of 6.75 breaks
and 7.0 does not. Both agree with BRK-RCH-001.

**R8.** The breaker is above the block. The distance is from the eye position to the top of the block
cube.

| Block | 5.75 | 5.95 | 6.05 | 6.25 |
|-------|------|------|------|------|
| Stone | breaks | breaks | stays | stays |
| Bottom slab | breaks | breaks | stays | stays |

**R9.**

```text
(202,-60,200) seq 6, then 200 ms later (202,-60,201) seq 7:
  t=0   block_update (202,-60,200) state 0; block_changed_ack seq 6
  t=201 block_update (202,-60,201) state 0; block_changed_ack seq 7
delay in ms: 1.7 49.9 49.4 49.0 47.4 46.4 45.9 44.4 43.1 41.7 41.8 39.6
```

**R10.**

| Observer distance on `x` | Observer gets |
|--------------------------|---------------|
| 20, 30, 45, 60, 63 | `level_event` type 2001, `block_update` |
| 65 | `block_update` |
| 80 | nothing |

The view distance of the server is 4.

**R11.**

| Selected slot | Result |
|---------------|--------|
| Wooden, golden, diamond and netherite sword; trident; mace; debug stick | `block_update` state 1, `block_changed_ack`; the block stays |
| Iron axe, diamond hoe, shears, copper spear, diamond pickaxe, stick, empty | `block_update` state 0, `block_changed_ack`; the block is air |

A diamond sword in hotbar slot 1, with slot 0 selected, does not refuse. A diamond sword in the off hand
does not refuse. With a diamond sword in the selected slot, the observer gets nothing.

**R12.**

```text
oak door, upper part, seq 54:
  breaker: level_event type 2001 (2,-59,0) data 7246
           section_blocks_update (2,-59,0) and (2,-60,0) at state 0
           block_changed_ack seq 54
  both parts are air
tall grass, upper part, seq 115:
  breaker:  level_event type 2001 (2,-59,0) data 14772
            section_blocks_update (2,-59,0) and (2,-60,0) at state 0
            block_changed_ack seq 115
  observer: level_event type 2001 (2,-59,0) data 14772
            level_event type 2001 (2,-60,0) data 14773
            level_event type 2001 (2,-59,0) data 14772
            section_blocks_update
red bed, foot, seq 114:
  breaker: level_event type 2001 (2,-60,0) data 2261
           section_blocks_update (2,-60,1) and (2,-60,0) at state 0
           block_changed_ack seq 114
```

**R13.**

```text
torch on stone:         level_event, add_entity, section_blocks_update, block_changed_ack
                        item: minecraft:torch, count 1
redstone wire on stone: add_entity, section_blocks_update, block_changed_ack
                        item: minecraft:redstone, count 1
oak sign on stone:      level_event, add_entity, section_blocks_update, block_changed_ack
                        item: minecraft:oak_sign, count 1
observer for the torch: level_event type 2001 (stone), level_event type 2001 (torch),
                        add_entity, section_blocks_update
```

**R14.**

```text
adventure: block_update state 1; block_changed_ack; observer gets nothing; block stays
spectator: block_update state 1; block_changed_ack; observer gets nothing; block stays
```

**R15.**

```text
outside the world border:  block_update state 1; block_changed_ack; block stays
y=320 with the player at y=318: block_update state 18655; block_changed_ack (two runs)
y=-65 with the player at y=-63: block_changed_ack (and with the player at y=-66)
y=319, air, in reach:      block_changed_ack
y=-64, bedrock of the flat world: block_update state 0; block_changed_ack; block is air
```

**R16.**

```text
not an operator:
  command_block:   block_update state 11718 (two times); block_changed_ack
  structure_block: block_update state 25086 (two times); block_entity_data; block_changed_ack
  jigsaw:          block_update state 25099 (two times); block_entity_data; block_changed_ack
an operator: each block: block_update state 0; block_changed_ack; block is air
```

**R17.**

```text
breaker:  block_changed_ack seq 11
observer: level_event type 2001 (2,-60,0) data 0
```

**R18.** Water and lava: `block_changed_ack` only. The source stays.

**R19.** Waterlogged oak slab: `block_update` state 89; `block_changed_ack`. The position then holds
water.

**R20.**

```text
player not an operator, x=2:
  system_chat {translate: "build.spawn_protection", with: ["2, -60, 0"], color: "red"} overlay
  block_changed_ack; block stays
another player, an operator, x=2: block_update state 0; block_changed_ack
  the first player gets: level_event type 2001 data 1; block_update state 0
x = 14, 15, 16, -14, -15, -16: system_chat, block_changed_ack; block stays
x = 17, 18, 19, -17, -18:      block_update state 0, block_changed_ack; block is air
```

**R21.** With `spawn-protection=16` and no operator, a player that is not an operator breaks a block at
`x=2`. The result is `block_update` state 0 and `block_changed_ack`.

**R22.**

```text
stone, oak_slab, empty chest, barrel, shulker_box, hopper, furnace: no item
chest with 3 diamonds and 2 dirt: 2 add_entity, then block_update, then block_changed_ack
  one entity reads {count: 3, id: "minecraft:diamond"}
barrel with 1 diamond: 1 item; furnace with 1 coal: 1 item
shulker_box with 1 diamond: 1 item:
  {components: {"minecraft:container": [{item: {id: "minecraft:diamond"}, slot: 0}]},
   count: 1, id: "minecraft:shulker_box"}
```

**R23.**

```text
t=0   block_update (2,-60,0) state 0; block_changed_ack seq 52
t=50  add_entity, falling block, at (2.5,-59,0.5); block_update (2,-59,0) state 0
t=350 block_update (2,-60,0) state 121 (the sand lands); add_transient_block; remove_entities
```

**R24.**

```text
seq 0:          block_update state 0; block_changed_ack seq 0
seq 1:          block_update state 0; block_changed_ack seq 1
seq 2147483646: block_update state 0; block_changed_ack seq 2147483646
seq 2147483647: block_update state 0; block_changed_ack seq 2147483647
seq -1:         disconnect {translate: "disconnect.packetError"}; block is air
```

**R25.** The state ids are 89 for a water source and 105 for a lava source. The other ids are the fluid at
that level. The result is the same in each of three runs, and in the one run of each source.

| Fluid | State id | Breaker | Observer | After |
|-------|----------|---------|----------|-------|
| water, level 0 | 89 | `block_changed_ack` | `level_event` type 2001 data 89 | stays |
| water, level 1 | 90 | `block_changed_ack` | `level_event` type 2001 data 90 | stays |
| water, level 7 | 96 | `block_changed_ack` | `level_event` type 2001 data 96 | stays |
| water, level 8 | 97 | `block_changed_ack` | `level_event` type 2001 data 97 | stays |
| lava, level 0 | 105 | `block_changed_ack` | `level_event` type 2001 data 105 | stays |
| lava, level 2 | 107 | `block_changed_ack` | `level_event` type 2001 data 107 | stays |
| lava, level 6 | 111 | `block_changed_ack` | `level_event` type 2001 data 111 | stays |
| lava, level 8 | 113 | `block_changed_ack` | `level_event` type 2001 data 113 | stays |

**R26.** `<old>` is the state id of the block before the break.

```text
waterlogged oak_slab, oak_stairs, oak_fence, sea_pickle, ladder;
kelp, kelp_plant, seagrass, bubble_column:
  breaker:  block_update state 89; block_changed_ack
  observer: level_event type 2001 data <old>; block_update state 89
  the position holds a water source; no item
oak_slab with no water:
  breaker:  block_update state 0; block_changed_ack
  observer: level_event type 2001 data <old>; block_update state 0
tall_seagrass, lower part hit:
  breaker:  level_event type 2001 at the upper part, data 2371
            section_blocks_update with both parts at state 89; block_changed_ack
  observer: level_event type 2001 at the lower part, data 2372
            level_event type 2001 at the upper part, data 2371
            section_blocks_update
tall_seagrass, upper part hit:
  breaker:  level_event type 2001 at the upper part, data 2371
            section_blocks_update with both parts at state 89; block_changed_ack
  observer: level_event type 2001 at the upper part, data 2371
            level_event type 2001 at the lower part, data 2372
            level_event type 2001 at the upper part, data 2371
            section_blocks_update
```

**R27.** Two runs, with the same result. `main` is the main part, `other` the other part. For tall
seagrass both parts become state 89. For the others they become state 0.

```text
oak_door:       main (upper) data 7246, other (lower) data 7254
red_bed south:  main (foot) data 2261, other (head) data 2260
tall_grass:     main (upper) data 14772, other (lower) data 14773
sunflower:      main (upper) data 14764, other (lower) data 14765
tall_seagrass:  main (upper) data 2371, other (lower) data 2372
small_dripleaf: main (upper) data 33812, other (lower) data 33814
pitcher_plant:  main (upper) data 16792, other (lower) data 16793

main part hit:
  breaker:  level_event type 2001 at main
            section_blocks_update with both parts (n=2); block_changed_ack
  observer: level_event at main, level_event at other, level_event at main
            section_blocks_update
other part hit:
  breaker:  level_event type 2001 at main
            section_blocks_update with both parts (n=2); block_changed_ack
  observer: level_event at other, level_event at main
            section_blocks_update
```

**R28.** Stone, empty hand, sequence 10. A stage is first seen at the tick count in brackets.

```text
start:
  breaker:  level_event type 2019 data 1; block_changed_ack seq 10
  observer: block_destruction entity 1 (2,-60,0) progress 0; level_event type 2019 data 1
each tick: level_event to both, type 2020 at the tick counts 4, 8, 12, ..., otherwise 2019
observer: progress 1 (14), 2 (29), 3 (44), 4 (59), 5 (74), 6 (89), 7 (104), 8 (119), 9 (134),
          10 (149), 11 (164), 12 (179), 13 (194)
level_event: the last one comes in the tick of progress 10
finish after 10 s, seq 11:
  breaker:  block_update state 0; block_changed_ack seq 11
  observer: block_destruction progress 255; level_event type 2001 data 1; block_update state 0
```

**R29.** The last column gives a stage and the tick count where the observer first gets it. Each tick
count is equal to the first tick count `c` where `floor(10 × rate × (c + 1))`, in 32-bit floating
point, reaches the stage. For cobweb with an empty hand, exact arithmetic gives one tick less.

| Block | Main-hand item and effect | `H` | `S` | `F` | Stage at tick count |
|-------|---------------------------|-----|-----|-----|---------------------|
| stone | empty | 1.5 | 1 | 100 | 10 at 149 |
| stone | wooden pickaxe | 1.5 | 2 | 30 | 10 at 22 |
| stone | diamond pickaxe | 1.5 | 8 | 30 | 10 at 5 |
| dirt | empty (three runs) | 0.5 | 1 | 30 | 10 at 14 |
| dirt | wooden pickaxe (three runs) | 0.5 | 1 | 30 | 10 at 14 |
| dirt | wooden shovel | 0.5 | 2 | 30 | 10 at 7 |
| dirt | wooden sword | 0.5 | 1 | 30 | 10 at 14 |
| obsidian | diamond pickaxe | 50 | 8 | 30 | 10 at 187 |
| iron ore | wooden pickaxe | 3 | 2 | 100 | 10 at 149 |
| iron ore | stone pickaxe | 3 | 4 | 30 | 10 at 22 |
| stone | wooden pickaxe, Efficiency II | 1.5 | 2 + 5 | 30 | 10 at 6 |
| dirt | wooden pickaxe, Efficiency II | 0.5 | 1 | 30 | 10 at 14 |
| stone | wooden pickaxe, Haste amplifier 1 | 1.5 | 2 × 1.4 | 30 | 10 at 16 |
| stone | empty, Haste amplifier 1 | 1.5 | 1.4 | 100 | 10 at 107 |
| stone | wooden pickaxe, Conduit Power amplifier 0 | 1.5 | 2 × 1.2 | 30 | 10 at 18 |
| stone | wooden pickaxe, Mining Fatigue amplifier 0 | 1.5 | 2 × 0.3 | 30 | 10 at 74 |
| stone | diamond pickaxe, Mining Fatigue amplifier 2 | 1.5 | 8 × 0.3³ | 30 | 10 at 208 |
| stone | diamond pickaxe, Mining Fatigue amplifier 3 | 1.5 | 8 × 0.3⁴ | 30 | 1 at 69 |
| glass | empty | 0.3 | 1 | 30 | 10 at 8 |
| cobweb | iron sword | 4 | 15 | 30 | 10 at 7 |
| cobweb | empty | 4 | 1 | 100 | 1 at 40, 7 at 279 |
| white wool | shears | 0.8 | 5 | 30 | 10 at 4 |
| oak leaves | wooden hoe | 0.2 | 2 | 30 | 10 at 2 |
| bedrock | empty | -1 | 1 | 100 | 0 only |

A first run of dirt with a wooden pickaxe gave 10 at 17, with a lag of the server. Three later runs
gave 14.

**R30.**

```text
short_grass, empty hand:
  breaker:  block_update state 0; block_changed_ack
  observer: level_event type 2001 data 2363; block_update state 0; no item
torch, empty hand:
  breaker:  add_entity (item minecraft:torch); block_update state 0; block_changed_ack
stone, netherite pickaxe with Efficiency V, Haste amplifier 1 (rate 1.09):
  breaker:  add_entity (item); block_update state 0; container_set_slot; block_changed_ack
  observer: level_event type 2001 data 1; add_entity; block_update state 0
stone, diamond pickaxe with Efficiency V (rate 0.756):
  breaker:  block_changed_ack; observer: block_destruction progress 7; the block stays
```

**R31.** Stone, wooden pickaxe. The rate with no factor is 0.0444.

| Case | Rate | Stage at tick count |
|------|------|---------------------|
| on-ground flag clear (three runs) | 0.00889 | 7 at 78, 10 at 112 |
| in water 4 blocks deep, eyes in water | 0.00889 | 10 at 112 |
| in water, helmet with Aqua Affinity | 0.0444 | 10 at 22 |

**R32.** Stone, wooden pickaxe, rate 0.0444. The finish action follows the tick count `k`. The rows
with a progress less than 0.7 show the delayed break of `26.4-snapshot-4`, which BRK-SRV-009 does not
state.

| `k` | Progress | Result |
|-----|----------|--------|
| 5 | 0.267 | `26.4-snapshot-4`: `block_changed_ack`; one tick later the block breaks; the observer gets progress 42 |
| 13 | 0.622 | `26.4-snapshot-4`: as `k = 5`, with progress 73 |
| 14 | 0.667 | `26.4-snapshot-4`: as `k = 5`, with progress 108 |
| 15 | 0.711 | the block breaks at once |
| 16 | 0.756 | the block breaks at once |

```text
k = 15:
  breaker:  add_entity (item minecraft:cobblestone); block_update state 0; container_set_slot;
            block_changed_ack
  observer: block_destruction progress 255; level_event type 2001 data 1; add_entity;
            block_update state 0
```

**R33.** Stone, wooden pickaxe, a dig at (2,-60,0).

```text
face change, face 3, seq 0: block_changed_ack seq 0; the next level_event has data 3;
                            the stages continue
abort at (2,-60,0), seq 0:  breaker:  block_changed_ack seq 0
                            observer: block_destruction (2,-60,0) progress 255
abort at (2,-60,1):         observer: block_destruction (2,-60,0) progress 255,
                                      then block_destruction (2,-60,1) progress 255
start at (2,-60,1):         breaker:  block_update (2,-60,0) state 1; block_changed_ack
                            observer: block_destruction (2,-60,1) progress 0, then 1, 2, ...
finish at (2,-60,1):        block_changed_ack; both blocks stay; the stages at (2,-60,0) continue
finish at (2,-60,-1), no dig in a new session: block_changed_ack; the block stays
```

**R34.** Bedrock, empty hand.

```text
start:             breaker: block_changed_ack; observer: block_destruction progress 0
the next 30 ticks: no block_destruction
finish:            block_changed_ack; the block stays
```

**R35.** Stone. A wooden pickaxe is in slot 0, and slot 2 is empty. The observer gets these stages,
with the tick count in brackets: 1 (2), 2 (4), 3 (6), 4 (8). The player selects slot 2 at the tick
count 10, and the observer gets 0 (11). The player selects slot 0 at the tick count 14, and the
observer gets 1 (14) and 7 (15). The finish action at the tick count 16 breaks the block.

**R36.** Stone at `y=-59`, in the row of the eye position. The distance is on `x`, from the eye
position to the face of the block.

| Distance | Result |
|----------|--------|
| 5.4, 5.49 | `block_changed_ack`; the observer gets the stages |
| 5.5, 5.51, 5.6 | `block_changed_ack`; the observer gets nothing |
| a dig at 5.4, a finish at 5.6 | `block_changed_ack`; the block stays; the stages continue |

**R37.** The exhaustion is the increase of `foodExhaustionLevel`. The maximum damage of a wooden
pickaxe is 59.

| Block | Main-hand item | Damage after | Items | Exhaustion |
|-------|----------------|--------------|-------|------------|
| stone | empty | none | none | 0 |
| stone | wooden pickaxe | 1 | cobblestone | 0.005 |
| stone | wooden pickaxe, Silk Touch | 1 | stone | 0.005 |
| stone | wooden pickaxe, damage 58 | the item breaks | cobblestone | 0.005 |
| stone | wooden hoe | 1 | none | 0 |
| dirt | wooden pickaxe | 1 | dirt | 0.005 |
| dirt | wooden shovel | 1 | dirt | 0.005 |
| dirt | wooden sword | 2 | dirt | 0.005 |
| dirt | mace | 2 | dirt | 0.005 |
| short grass | wooden pickaxe | 0 | none | 0.005 |
| torch | wooden pickaxe | 0 | torch | 0.005 |
| iron ore | wooden pickaxe | 1 | none | 0 |
| iron ore | stone pickaxe | 1 | raw_iron | 0.005 |
| coal ore | stone pickaxe | 1 | coal, one experience orb | 0.005 |
| glass | empty | none | none | 0.005 |
| glass | trident | 2 | none | 0.005 |
| white wool | shears | 1 | white_wool | 0.005 |
| oak leaves | shears | 1 | oak_leaves | 0.005 |
| cobweb | iron sword | 2 | string | 0.005 |
| cobweb | empty | none | none | 0 |
| grass block | empty | none | dirt | 0.005 |
| oak log | empty | none | oak_log | 0.005 |

```text
coal ore at (3,-60,0), breaker:
  add_entity item at (3.43,-59.64,0.45); add_entity experience_orb at (3.5,-59.5,0.5);
  block_update state 0; container_set_slot; block_changed_ack
```

For the block at (3,-60,0), the item entities of all the runs were at `x` from 3.27 to 3.70, `y`
from -59.87 to -59.43 and `z` from 0.25 to 0.72.

**R38.** The observer is on the `x` axis from the block (3,-60,0), at `z=0.5`. The dig lasts about 12
ticks.

| Distance | `block_destruction` packets | `level_event` 2019 and 2020 packets |
|----------|-----------------------------|-------------------------------------|
| 20 | 7 | 12 |
| 31 | 7 | 12 |
| 33 | 0 | 12 |
| 40 | 0 | 12 |

**R39.** Stone, diamond pickaxe with Efficiency V, rate 0.756. The observer gets these stages, with
the tick count in brackets: 7 (0), 15 (1), 22 (2), and on to 241 (31), 249 (32), 0 (33), 8 (34),
16 (35), and on to 250 (66), 1 (67), 9 (68). The dig continues until the abort. Each value is
`floor(7.56 × (c + 1))` modulo 256.

**R40.** Stone, wooden pickaxe, a dig at (2,-60,0).

```text
set air at tick count 6:
  breaker:  block_update state 0; the level_event packets stop
  observer: block_update state 0; block_destruction progress 255
then set stone:
  both:     block_update state 1; no level_event; no block_destruction
then finish, 1.2 s later:
  the block breaks with the packets of BRK-SRV-008 (deviation D2)
set dirt at tick count 6:
  observer: block_update state 10; block_destruction progress 6, then 7, 8, ..., 20
  the level_event packets continue until the progress reaches 1; the finish breaks the dirt
set obsidian at tick count 6:
  observer: block_update; block_destruction progress 0; the level_event packets continue
```

Delayed break on obsidian, 3 s after a new login, rate 0.0004. The last line shows the delayed break
of `26.4-snapshot-4`:

```text
finish at tick count 5:     breaker: block_changed_ack; observer: nothing more
start at a second block:    breaker: block_changed_ack; observer: block_destruction progress 0
                            no level_event and no stage for 15 s
finish at the second block: the block breaks with the packets of BRK-SRV-008
start and abort at the position of the delayed break:
                            observer: progress 0, nothing for 15 s, then progress 255
then set stone at the position, in both cases:
                            the stone breaks in the next tick; the observer gets progress 42
```

**R41.** Stone, wooden pickaxe, a dig at (2,-60,0). The second position is (2,-60,1).

```text
face change, face 3, at the second position: block_changed_ack seq 0;
                                             the next level_event has data 3; the stages continue
abort at the second position:                breaker:  block_changed_ack seq 0; the level_event stop
                                             observer: progress 255 at (2,-60,0), then at (2,-60,1)
finish at (2,-60,0):                         the block breaks (deviation D2)
abort with no dig, at (2,-60,1):             breaker:  block_changed_ack seq 0
                                             observer: progress 255 at (2,-60,0), then at (2,-60,1)
face change with no dig:                     breaker:  block_changed_ack seq 0
```

**R42.** The observer is on the `x` axis from the block corner (3,-60,0), at `z=0.5`. The dig lasts
0.8 s.

| Distance | `level_event` 2019 and 2020 packets |
|----------|-------------------------------------|
| 48, 60 | 10 |
| 64, 65, 80 | 0 |

**R43.** Each case is on stone. A stage is first seen at the tick count in brackets. The predicted
tick count uses BRK-PRG-001 in 32-bit floating point. The observed tick counts are one less than the
predicted ones in most cases, because of the time of the first packet.

| Main-hand item and effect | Rate | Observed | Predicted |
|---------------------------|------|----------|-----------|
| wooden pickaxe, Haste amplifier 1 and Conduit Power amplifier 0 | 0.0622 | 5 (7), 10 (15) | 5 (8), 10 (16) |
| wooden pickaxe, Haste amplifier 0 and Conduit Power amplifier 1 | 0.0622 | 5 (7), 10 (15) | 5 (8), 10 (16) |
| netherite pickaxe with Efficiency V, Mining Fatigue amplifier 3 | 0.0063 | 1 (14), 2 (30) | 1 (15), 2 (31) |
| netherite pickaxe with Efficiency V, Mining Fatigue amplifier 4 | 0.00189 | 1 (51) | 1 (52) |
| netherite pickaxe with Efficiency V, Mining Fatigue amplifier 9 | 0.000005 | none in 180 ticks | none |
| wooden pickaxe, `minecraft:block_break_speed` 2 | 0.0889 | 10 (10) | 10 (11) |
| wooden pickaxe, `minecraft:block_break_speed` 0.5 | 0.0222 | 10 (43) | 10 (44) |
| wooden pickaxe, feet in water, eyes out of water | 0.0444 | 10 (21) | 10 (22) |
| wooden pickaxe, eyes in water, on-ground flag clear | 0.00178 | 1 (55), 2 (111), 3 (167) | 1 (56), 2 (112), 3 (168) |

With the sum of the two amplifiers, the first two cases would give 10 (13).

**R44.**

| Item | Breaks | Damage after |
|------|--------|--------------|
| netherite pickaxe, Efficiency V, Unbreaking III | 40 | 12 |
| netherite pickaxe, Efficiency V | 10 | 10 |

```text
wooden pickaxe with the damage 58, finish on stone:
  breaker:  entity_event entity 9 status 47; add_entity (item); block_update state 0;
            container_set_slot slot 36, empty; block_changed_ack
  observer: block_destruction progress 255; level_event type 2001 data 1;
            entity_event entity 9 status 47; add_entity; block_update state 0
  no sound packet
```

**R45.**

| Block | Main-hand item | Items |
|-------|----------------|-------|
| chest with 3 diamonds and 2 sticks | iron axe | diamond ×3, stick ×2, chest |
| red shulker box with 3 diamonds | iron pickaxe | red shulker box with the 3 diamonds in its container component |
| stone with a torch on it | wooden pickaxe | torch, cobblestone |
| stone with the lower part of an oak door on it | wooden pickaxe | oak door, cobblestone |

```text
stone with a torch on it, breaker:
  level_event type 2001 at (2,-59,0) data 4926; add_entity (torch); add_entity (cobblestone);
  section_blocks_update with both blocks; container_set_slot; block_changed_ack
stone with sand on it, breaker:
  add_entity (item); block_update state 0; container_set_slot; block_changed_ack;
  one tick later: add_entity (falling block); block_update (2,-59,0) state 0
red shulker box: the finish came at a progress less than 0.7, and the box broke by the delayed
break of 26.4-snapshot-4:
  breaker:  add_entity; container_set_slot; block_changed_ack; one tick later: block_update state 0
  observer: block_destruction progress 79; level_event type 2001; add_entity; block_update
```

**R46.** Each count is the number of breaks out of 60 that gave that experience.

| Block | Experience: number of breaks |
|-------|------------------------------|
| coal ore | 0: 20, 1: 24, 2: 16 |
| deepslate coal ore | 0: 23, 1: 20, 2: 17 |
| nether gold ore | 0: 30, 1: 30 |
| diamond ore | 3: 7, 4: 13, 5: 13, 6: 15, 7: 12 |
| deepslate diamond ore | 3: 13, 4: 11, 5: 10, 6: 14, 7: 12 |
| emerald ore | 3: 11, 4: 12, 5: 13, 6: 13, 7: 11 |
| lapis ore | 2: 15, 3: 13, 4: 17, 5: 15 |
| nether quartz ore | 2: 21, 3: 14, 4: 11, 5: 14 |
| redstone ore | 1: 11, 2: 10, 3: 12, 4: 14, 5: 13 |
| lit redstone ore | 1: 14, 2: 17, 3: 6, 4: 13, 5: 10 |
| spawner | from 15 to 43, mean 29.5; 15 and 43 each came one time |
| sculk | 1: 60 |
| sculk sensor, calibrated sculk sensor | 5: 59, 4: 1 |
| sculk shrieker, sculk catalyst | 5: 60 |
| deepslate emerald ore | 3: 11, 4: 13, 5: 13, 6: 12, 7: 11 |
| deepslate lapis ore | 2: 14, 3: 14, 4: 17, 5: 15 |
| deepslate redstone ore | 1: 11, 2: 15, 3: 13, 4: 10, 5: 11 |
| lit deepslate redstone ore | 1: 9, 2: 7, 3: 17, 4: 13, 5: 14 |
| iron ore, copper ore, gold ore, their deepslate forms, ancient debris | 0: 60 |
| diamond ore with Silk Touch, sculk with Silk Touch, diamond ore with a wooden pickaxe (10 breaks each) | 0 |

The orbs of one break had the values 1, 1 and 3 for 5, 3 and 3 for 6, 7 for 7, and 1, 1, 37 for 39.
The one result of 4 for a sculk sensor had the orbs 1 and 3. A probable cause is that an orb of 1
merged with an older orb before the client got its value.

**R47.** Creative mode, so the limit is 6.0. The block is at (6,-57,0), above the eye position. The
distance is on `x`, from the player position to the face of the block. The last column gives the
distance from the eye position to the block cube for the eye height of BRK-RCH-003.

| Pose | `x` distance | Result | Distance with that eye height |
|------|--------------|--------|-------------------------------|
| stands | 5.79 | breaks | 5.952 |
| stands | 5.84 | stays | 6.000 |
| crouches | 5.79 | stays | 6.043 |
| crouches | 5.6, 5.5 | breaks | 5.861, 5.765 |
| stands again | 5.79 | breaks | 5.952 |
| crawls | 5.6 | stays | 6.174 |
| crawls | 5.3, 5.2 | breaks | 5.903, 5.817 |

**R48.**

```text
(-1,-60,2) and (0,-60,2), two sections:
  breaker:  block_update (0,-60,2) state 0; block_update (-1,-60,2) state 0; block_changed_ack
  observer: level_event 2001 at (-1,-60,2); level_event 2001 at (0,-60,2);
            block_update (0,-60,2); block_update (-1,-60,2)
(2,-60,2) and (2,-60,3), one section:
  breaker:  section_blocks_update with 2 entries; block_changed_ack
  observer: level_event 2001 at (2,-60,2); level_event 2001 at (2,-60,3); section_blocks_update
```

**R49.** Creative mode. The block is at (2,-60,0), except where the case states another position.

```text
piston head at (2,-59,0) on an extended piston, hit the head:
  breaker:  block_update (2,-59,0) state 0; block_changed_ack; the piston stays; no item
  observer: level_event 2001 at the head; block_update
extended piston, hit the piston:
  breaker:  level_event 2001 at (2,-59,0) data 2403; section_blocks_update with both; block_changed_ack
  observer: level_event 2001 at the piston, data 2383; level_event 2001 at the head; section_blocks_update
sticky piston head:  as the piston head
beehive, honey level 5, one bee:
  breaker:  add_entity (item); block_update state 0; block_changed_ack
  observer: add_entity; level_event 2001; block_update
  item:     beehive with the components minecraft:bees (one bee) and minecraft:block_state honey_level 5
bee nest, honey level 5, no bee: as the beehive; the item has honey_level 5
empty beehive:       block_update; block_changed_ack; no item
left half of a double chest:
  breaker:  section_blocks_update: (2,-60,0) air, (3,-60,0) a single chest; block_changed_ack
ice:                 the block changes to air
jukebox with a disc:
  breaker:  add_entity (music_disc_13); level_event 1011 data 0; block_update; block_changed_ack
  observer: level_event 2001; add_entity; level_event 1011; block_update
lectern with a book:          one item, the writable book
chiseled bookshelf, one book: one item, the book
campfire with beef:           one item, the beef
decorated pot with a diamond: one item, the diamond
potted poppy:                 no item
tnt:                          the block changes to air; no entity
cactus on sand, two high, hit the bottom:
  breaker:  level_event 2001 at (2,-59,0); add_entity (cactus); section_blocks_update with both;
            block_changed_ack
sugar cane, two high:         as the cactus; one item, sugar_cane
kelp plant, kelp plant, kelp in a glass column of water, hit the bottom:
  breaker:  level_event 2001 at (2,-59,0); add_entity (kelp); section_blocks_update: (2,-60,0)
            and (2,-59,0) water; block_changed_ack
            one tick later: level_event 2001 at (2,-58,0); add_entity (kelp); block_update water
```
