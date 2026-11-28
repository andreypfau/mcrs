# Block breaking: behavioural specification

| Field | Value |
|-------|-------|
| Version | see `assets/minecraft/version.json` |

This document states how a player breaks a block in creative mode and in survival mode, and what
the server sends for it. It states only what a client, the server console and the server files
show. Most rules come from a vanilla server in offline mode.
A rule with a `source_suggested` label comes from the reference implementation. Section 10 gives the
experiments.

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

- `black_box_confirmed`: shown by an experiment on a vanilla server.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with an experiment on a vanilla server.
- `source_suggested`: taken from the reference implementation, with no experiment result. Section 10
  attaches an experiment.
- `unresolved`: not decided. Section 9 collects these items.

All the results come from one vanilla server and scripted clients on one machine. A rule that rests
on one observation states this.

### 0.3 ID prefix

Each rule has an ID with the prefix `BRK`.

## 1. The request

**BRK-REQ-001** (`black_box_confirmed`, wire). The `player_action` packet has these fields, in this
order: action (VarInt), position (long), face (byte), sequence (VarInt).

**BRK-REQ-002** (`black_box_confirmed`, semantic). In creative mode, only the start action can break
a block.

| Action | Result in creative mode |
|--------|-------------------------|
| 0 | The block breaks, unless section 3 refuses it. |
| 1 | An acknowledgement only. |
| 2 | An acknowledgement to the breaker. An observer near the block gets `block_destruction` with progress 255 (BRK-REQ-006). |
| 3 | An acknowledgement only. |

Actions 4 to 8 do not act on a block. This document does not state them.

**BRK-REQ-003** (`black_box_confirmed`, wire). An action of 9 closes the connection, because the
packet does not decode. The block does not change. This rule rests on one observation.

**BRK-REQ-004** (`black_box_confirmed`, semantic). The face has no effect on a break. The values 0, 1,
2, 5 and 9 all break the block.

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
acknowledgement.

**BRK-BRK-002** (`black_box_confirmed`, wire). The server sends `block_update` when one block changes
in a section. It sends `section_blocks_update` when more than one block in a section changes together.

**BRK-BRK-003** (`black_box_confirmed`, semantic). An observer gets `level_event` with type 2001 and
the state id of the old block as its data, and then the new block state. An observer does not get an
acknowledgement.

**BRK-BRK-004** (`black_box_confirmed`, semantic). The breaker does not get `level_event` for a break
of one block. The breaker does get it for the parts of section 4.

**BRK-BRK-005** (`black_box_confirmed`, semantic). A creative break does not send
`block_destruction`.

**BRK-BRK-006** (`source_suggested_black_box_confirmed`, semantic). An observer in the same dimension
gets the `level_event` when the distance from its position to the block corner is less than 64. An
observer gets the new block state while its client holds the chunk.

**BRK-BRK-007** (`source_suggested_black_box_confirmed`, semantic). The server handles a request
between two ticks. In the next tick, it sends the changed block states, and then the acknowledgement.
The server sends the `block_update` of a refusal when it handles the request.

**BRK-BRK-008** (`black_box_confirmed`, semantic). Bedrock, barrier, end portal frame, structure void,
light, moving piston, nether portal and end portal break like any other block.

**BRK-BRK-009** (`source_suggested_black_box_confirmed`, semantic). A start action on fire or soul fire
extinguishes it. The breaker and each observer get `level_event` with type 1009, and then the new block
state. They do not get type 2001.

## 3. Refusals

### 3.1 Reach

**BRK-RCH-001** (`source_suggested_black_box_confirmed`, semantic). A start action is refused when the
distance from the eye position to the nearest point of the block cube is the limit or more. A distance
less than the limit is in reach. The breaker gets one acknowledgement and no block state. An
observer gets nothing.

The limit is the value of the `minecraft:block_interaction_range` attribute of the breaker, plus 1.0
(ATR-VAL-001 of `attributes.md`). The base value is 4.5, and creative mode adds 0.5 with a modifier
(ATR-SRC-003 of `attributes.md`). Thus, with that base value and no other modifier, the limit is 6.0 in creative mode and 5.5 in
survival mode.

**BRK-RCH-002** (`black_box_confirmed`, semantic). The distance uses the block cube, and not the
shape of the block. A bottom slab has the same limit as a full block when the breaker is above it.

**BRK-RCH-003** (`source_suggested_black_box_confirmed`, semantic). The eye position is the position of
the player plus the eye height of its current pose. The eye height is 1.62 when the player stands,
1.27 when it crouches, and 0.4 when it crawls or swims. The player crouches while its last
`player_input` has the sneak flag and it does not fly. It crawls when a block above it leaves no room
to stand or crouch. The result bounds the crouch height to 0.85 to 1.43, and the crawl
height to 0.19 to 0.85.

### 3.2 Refusals that send the block state

**BRK-REF-001** (`black_box_confirmed`, semantic). In each case of this table, the breaker gets
`block_update` with the current state of the block, and then one acknowledgement. An observer gets
nothing where the table states it.

| Case | Observer |
|------|----------|
| The selected hotbar slot holds an item that cannot break blocks (BRK-REF-002) | Gets nothing |
| The player is in adventure mode or spectator mode | Gets nothing |
| The position is outside the world border | Gets nothing, BRK-ORD-001 |
| The position is above the height limit, and in reach | Gets nothing, BRK-ORD-001 |

For the position above the height limit, the state id in the packet is 18655, and not 0.

**BRK-REF-002** (`source_suggested_black_box_confirmed`, semantic). The item in the selected hotbar slot
refuses a break in two cases:

- The `minecraft:tool` component of the item has `can_destroy_blocks_in_creative` set to false.
- The item is the debug stick.

All swords, the trident and the mace have this component value. An axe, a hoe, shears, a
spear, a pickaxe, a stick and an empty hand do not refuse a break. Only the selected hotbar slot counts.
The other hotbar slots and the off hand have no effect.

For an operator of level 2 or higher, the debug stick also selects a block property and sends a
message. This document does not state that behaviour.

**BRK-REF-003** (`black_box_confirmed`, semantic). A command block, a structure block and a jigsaw
block refuse a break when the player is not an operator. An operator breaks them. In a
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

| Case |
|------|
| The block is out of reach (BRK-RCH-001) |
| The position is air |
| The position holds water or lava, at a level from 0 to 8 |
| The position is below the height limit |

**BRK-ACK-002** (`black_box_confirmed`, semantic). For a start action on air, an observer gets
`level_event` with type 2001 and data 0. An observer gets no block state. This rule
rests on one observation.

**BRK-ACK-003** (`source_suggested_black_box_confirmed`, semantic). A block of one part that holds a
water source changes to a water source. The breaker gets `block_update` with the state of a water
source, and then one acknowledgement. An observer gets `level_event` with type 2001 and the state id of
the old block, and then the same block state. No item drops. A block of the same type with no water changes to air. Each block type rests on one
observation. Section 4 states the packets for tall seagrass.

**BRK-ACK-004** (`black_box_confirmed`, semantic). For a start action on water or lava, at a level from
0 to 8, an observer gets `level_event` with type 2001 and the state id of the fluid. An observer gets no
block state. The fluid stays.

**BRK-ACK-005** (`source_suggested`, semantic). Water or lava at a level from 9 to 15 changes to the
same fluid at level 8. The breaker gets `block_update` with the new state, and then one acknowledgement.
An observer gets `level_event` with type 2001 and the old state id, and then the new block state. See
experiment E31.

### 3.4 Spawn protection

**BRK-SPN-001** (`source_suggested_black_box_confirmed`, semantic). A start action is refused when all of
these conditions are true.

- The value of `spawn-protection` is more than 0.
- The block is in the dimension of the world spawn.
- At least one operator exists.
- The player is not an operator, at any operator level.
- The larger of the two distances on the `x` axis and on the `z` axis, from the world spawn block to the
  block position, is at most the value of `spawn-protection`.

The `y` coordinate has no effect. An operator breaks the block, and the break is normal. A server with
no operator does not refuse. Experiment E33 tests the other conditions.

**BRK-SPN-002** (`black_box_confirmed`, wire). In this refusal, the breaker gets `system_chat` in the
overlay position, and then one acknowledgement. The breaker gets no block state. The message is the
translation key `build.spawn_protection` in red, with the argument `x, y, z`. An
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
are in the same section. The acknowledgement comes last.

**BRK-PRT-002** (`black_box_confirmed`, semantic). One part is the main part: the upper part of a door
or a tall plant, and the foot of a bed. The breaker gets one `level_event` with type 2001 at the main
part, whichever part was hit, before the block state.

**BRK-PRT-004** (`black_box_confirmed`, semantic). An observer gets `level_event` with type 2001 for the
hit part and then for the other part. When the hit part is the main part, a third packet repeats it.
The data of each packet is the state of the part at its position.

**BRK-PRT-003** (`black_box_confirmed`, semantic). A door, a bed and a tall plant drop no item in a
creative break.

**BRK-PRT-005** (`black_box_confirmed`, semantic). A break of an extended piston also removes its
head: the breaker gets `level_event` with type 2001 at the head, and then the block states of both
parts. An observer gets the `level_event` for the piston and then for the head. No item drops. The result tests a piston and a sticky piston that face up.

**BRK-PRT-007** (`source_suggested`, semantic). A break of a piston head also removes its piston when
all of these conditions are true:

- The piston is behind the head, on the axis of the `facing` property of the head.
- The piston has `extended=true` and the same `facing` value as the head.
- The piston is a sticky piston for a head with `type=sticky`, and a piston for `type=normal`.

In other cases, the break removes only the head. See experiment E64.

**BRK-PRT-006** (`black_box_confirmed`, semantic). A break of one half of a double chest changes the
other half to a single chest. Both changes reach the breaker in one block update of the section.

## 5. Drops

**BRK-DRP-001** (`black_box_confirmed`, semantic). A creative break of a block drops no item for the
block itself. Stone, an oak slab, and an empty chest, barrel, shulker box,
hopper and furnace drop nothing.

**BRK-DRP-002** (`black_box_confirmed`, semantic). The contents of a container drop. One item entity
appears for each stack, at a random position near the block. The tested containers are a
chest, a barrel and a furnace.

**BRK-DRP-003** (`black_box_confirmed`, semantic). A shulker box with contents drops one item: the
shulker box, which holds the contents. This rule rests on one observation.

**BRK-DRP-004** (`black_box_confirmed`, semantic). A block that the break removes because its support
is gone drops its item, in creative mode too. A torch, redstone wire and a sign on the block each drop
one item. The item entity reaches the client before the block state.

**BRK-DRP-005** (`black_box_confirmed`, semantic). The order of packets to the breaker for such a break
is: `level_event` for the removed block, where the block has one, then `add_entity`, then
`section_blocks_update`, then the acknowledgement. Redstone wire sends no `level_event`.

**BRK-DRP-006** (`black_box_confirmed`, semantic). A block that falls does so after the break. For sand
on the block, the server sends the acknowledgement first. About one tick later, it sends `add_entity`
and the new block state of the sand. `block-simulation.md` states how neighbours update.

**BRK-DRP-007** (`black_box_confirmed`, semantic). In a creative break, these blocks drop the items
that they hold, and not the block item. The breaker and each observer get `add_entity` for each item
before the block state.

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
`level_event` with type 2001.

**BRK-DRP-009** (`black_box_confirmed`, semantic). A cactus or sugar cane on the broken block breaks in
the same tick, as in BRK-DRP-004, and drops its item. For kelp, the kelp plant above the broken block
breaks in the same tick, and the top kelp one tick later. Each part above the broken block drops one
kelp item. The positions of the kelp change to water.

## 6. Several requests and the sequence

**BRK-SEQ-001** (`black_box_confirmed`, semantic). Requests that the server handles between two ticks
get one acknowledgement (BRK-BRK-007). It carries the highest sequence of those requests, whichever
came last.

**BRK-SEQ-002** (`black_box_confirmed`, semantic). Requests about 200 ms apart each get their own
acknowledgement.

**BRK-SEQ-003** (`black_box_confirmed`, semantic). The three actions 0, 3 and 2 in one tick break the
block and get one acknowledgement with the highest sequence. The same request three
times in one tick sends one block state and one acknowledgement.

**BRK-SEQ-004** (`black_box_confirmed`, wire). The sequences 0, 1, 2147483646 and 2147483647 are
valid.

**BRK-SEQ-005** (`black_box_confirmed`, semantic). A negative sequence closes the connection with the
translation key `disconnect.packetError`. The block breaks before the connection closes. This rule rests on one observation.

**BRK-SEQ-006** (`black_box_confirmed`, semantic). When breaks in one tick change blocks in more than
one section, the breaker gets the packet of BRK-BRK-002 for each section, and then one
acknowledgement. The order of the sections is not specified. An observer gets the `level_event` of
each break in the order of the requests, and then the block states.

## 7. Survival mode

The rules of this section apply in survival mode. The checks of section 3 apply to each action
first, in the order of BRK-ORD-001. BRK-SRV-016 states one exception. In adventure mode and in
spectator mode, BRK-REF-001 refuses the start action.

These rules state Java Edition. For example, Bedrock Edition keeps the progress when the main-hand
item changes, and Java Edition restarts it (BRK-ITM-001).

### 7.2 The dig

**BRK-SRV-001** (`black_box_confirmed`, semantic). A start action on a block with a rate less than 1
starts a dig at the position, with the face of the request. The breaker gets one acknowledgement and
no block state. Each observer of BRK-SRV-004 gets `block_destruction` with the entity id of the
breaker, the position and the stage `floor(rate × 10)`.

**BRK-SRV-002** (`black_box_confirmed`, semantic). A start action on a block with a rate of 1 or
more breaks the block at once. The packets are the packets of a finish (BRK-SRV-008), but the
observer gets no `block_destruction`. A block with a hardness of 0 gives this result with each
main-hand item.

**BRK-SRV-003** (`black_box_confirmed`, semantic). A start action at a new position while a dig
exists replaces the dig. The breaker gets `block_update` with the current state at the position of
the old dig, and then the acknowledgement. The observer gets `block_destruction` at the new
position, and the vanilla client moves the crack there (CBK-DST-002).

**BRK-SRV-004** (`black_box_confirmed`, semantic). While a dig exists, and the block at its position
is not air, the server does these steps in each tick:

1. Calculate the progress (BRK-PRG-001).
2. If the stage is not the stage that the server sent last for the dig, send `block_destruction`
   with the new stage. The receivers are the observers at a distance less than 32 from the block
   corner. The breaker does not get it.
3. If the progress is 1 or less, send `level_event` to the breaker and to each observer at a
   distance less than 64 from the block corner, as in BRK-BRK-006. The type is 2020 when the tick
   count is a multiple of 4, and otherwise 2019. The data is the face of the dig.

In the first tick, the `level_event` comes before the acknowledgement of the start.

**BRK-SRV-005** (`black_box_confirmed`, semantic). The stage of BRK-SRV-004 has no limit of 9. When the
progress is more than 1, the observer gets the stages 10, 11 and more until the dig ends. The vanilla
client removes the crack for such a stage (CBK-DST-002). The packet carries the stage modulo 256. Thus
after 249 comes 0, and a stage of 255 removes the crack as an abort does.

**BRK-SRV-006** (`black_box_confirmed`, semantic). An abort action ends the dig. The breaker gets one
acknowledgement with the sequence of the request. The vanilla client sends the sequence 0
(CBK-SRV-006). Each observer gets `block_destruction` with the progress 255 at the position of the
request. When the dig has another position, the observer first gets the same packet for the position
of the dig, and the dig ends too. With no dig, the breaker gets the acknowledgement, and the observer
gets the packet for the position of the request. Section 8, entry D2, records
a packet that the reference also sends with no dig.

**BRK-SRV-007** (`black_box_confirmed`, semantic). A face change action changes the face of the dig.
The position of the request has no effect. The breaker gets one acknowledgement. The progress does not
change. From the next tick, the data of the `level_event` of BRK-SRV-004 is the new face. With no dig,
the breaker gets only the acknowledgement.

**BRK-SRV-008** (`black_box_confirmed`, semantic). A finish action at the position of the dig breaks
the block when the progress of the last tick is 0.7 or more. The dig ends. The breaker gets these
packets in one tick, in this order:

1. `add_entity` for each item entity and each experience orb that drops (BRK-LOOT-001).
2. `block_update` with the new state.
3. `container_set_slot` for the main-hand slot, when the item takes damage (BRK-DMG-001).
4. The acknowledgement.

An observer gets `block_destruction` with the progress 255, `level_event` with type 2001 and the
state id of the old block, `add_entity` for each drop, and then `block_update`.

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
the reference behaviour. See experiment E45.

**BRK-SRV-010** (`black_box_confirmed`, semantic). In each of these cases, a finish action gets only
one acknowledgement, and the block stays:

- The player has no dig. Section 8, entry D2, records a case where the reference
  breaks the block.
- The position of the request is not the position of the dig. The dig continues.
- The request is out of reach (BRK-SRV-012). The dig continues.

**BRK-SRV-011** (`black_box_confirmed`, semantic). A block with a hardness of -1 has a rate of 0. A
start action on such a block starts a dig with the stage 0, and the stage does not change. A finish
action gets only one acknowledgement, and makes no delayed break. Section 8, entry D3,
records the reference behaviour.

**BRK-SRV-012** (`black_box_confirmed`, semantic). The reach limit of BRK-RCH-001 is the attribute
`minecraft:block_interaction_range` plus 1. In survival mode, the default attribute is 4.5, so the
default limit is 5.5. Creative mode adds 0.5 to the attribute, which gives the 6.0 of BRK-RCH-001.
Only the default attribute was tested. A start action out of reach gets one
acknowledgement, and the observer gets nothing. The limit applies to the finish action too.

**BRK-SRV-013** (`black_box_confirmed`, semantic). When the block at the position of a dig becomes
air, the dig ends in the next tick. Each observer of BRK-SRV-004 gets `block_destruction` with the
progress 255. The breaker gets nothing more. When the block becomes another state that is not air,
the dig continues, and the rate of each later tick uses the new state. Section 8,
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
instead. Section 8, entry D4, records the difference.

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

CBK-SRV-003 states the same rate for the client.

**BRK-PRG-003** (`black_box_confirmed`, semantic). The block definition states if a block needs the
correct tool for drops. A main-hand item is a correct tool for a block when the first rule of its
`minecraft:tool` component that matches the block and has a value for `correct_for_drops` has the
value true. Thus a wooden pickaxe has the speed 2 on iron ore, but is not a correct tool for it.

### 7.4 The held item

**BRK-ITM-001** (`source_suggested`, semantic). For Java Edition, a change of the main-hand item
restarts the progress. The vanilla client sends an abort action and then a start action
for this (CBK-SRV-001). The start action makes a new dig with the tick count 0 (BRK-SRV-001). See
experiment E47.

**BRK-ITM-002** (`black_box_confirmed`, semantic). When the main-hand item changes and no start action
follows, the dig continues with the same start tick. From the next tick, the rate uses the new item. Section 8, entry D4, records that the reference also applies the new rate to the
earlier ticks.

### 7.5 Tool damage

**BRK-DMG-001** (`black_box_confirmed`, semantic). After a survival break of a block with a hardness
that is not 0, the main-hand item takes damage. The amount is the value of `damage_per_block` in the
`minecraft:tool` component of the item. An item with no such component takes no damage. The damage
does not need a harvest. A block with a hardness of 0 causes no damage.

**BRK-DMG-002** (`black_box_confirmed`, semantic). When the damage of the item becomes equal to its
maximum damage or more, the item breaks, and the main-hand slot becomes empty. The breaker and each
observer get `entity_event` with the entity id of the breaker and the status 47, before the `add_entity` of
BRK-SRV-008. The breaker gets `container_set_slot` with an empty item for the slot. The server sends
no sound packet: the vanilla client plays the sound for the status 47.

**BRK-DMG-003** (`source_suggested_black_box_confirmed`, semantic). When the item has the Unbreaking
enchantment of level `n`, each point of damage of BRK-DMG-001 applies with the chance `1 / (n + 1)`.

### 7.6 Drops and exhaustion

**BRK-LOOT-001** (`black_box_confirmed`, semantic). A survival break with a harvest drops the items of
the loot table of the block. The loot table uses the main-hand item, for example for Silk Touch. Each
item stack becomes one item entity at a random position. The `x` and `z` coordinates are from 0.25
to 0.75 inside the block cube. The `y` coordinate is from 0.125 to 0.625 above the bottom of the
cube: the centre minus half the item height of 0.25, then plus or minus 0.25. The observed `y`
offsets were from 0.13 to 0.57. A block that gives experience also drops experience orbs at the centre
of the block, after the items. The orbs of a creaking heart are an exception: they come before the
items (BRK-LOOT-006). A survival break without a harvest drops nothing from the loot table.

**BRK-LOOT-002** (`black_box_confirmed`, semantic). A survival break with a harvest adds 0.005 to the
food exhaustion of the breaker. A survival break without a harvest adds nothing.

**BRK-LOOT-003** (`black_box_confirmed`, semantic). In a survival break, a container that keeps its
contents in the block drops each content stack as an item entity, and the block drops its own item
from its loot table. A shulker box drops only its own item, with the contents in the item. A block
that loses its support drops its items in the same tick, as in BRK-DRP-004 and BRK-DRP-005: the
breaker gets `level_event` with type 2001 for that block, then `add_entity` for its items, then for
the items of the broken block, then `section_blocks_update` with both changes. A block that falls
does so one tick later, as in BRK-DRP-006.

**BRK-LOOT-004** (`black_box_confirmed`, semantic). A survival break with a harvest and without Silk
Touch drops the experience of this table. A range is a uniform random integer from the first value
to the second value, both included, except for the spawner. Other blocks drop no experience, for
example iron ore, copper ore, gold ore, their deepslate forms and ancient debris. BRK-LOOT-006 states
one exception. A break without a harvest drops no experience. A break with Silk Touch drops no
experience, except from a spawner. The
`minecraft:block_experience` effect of the Silk Touch enchantment sets the experience to 0. An
implementation must read this effect from the enchantment of the main-hand item. These statements
are for the enchantments of the vanilla data pack. With another enchantment, BDR-XP-002 of
`block-drops.md` gives the experience, also for iron ore and after Silk Touch.

The spawner is `source_suggested`, from the reference of the game version above. Its experience is
the sum of 15 and two uniform integers from 0 to 14, so it is not uniform. Silk Touch has no effect
on it. BDR-XP-002 of `block-drops.md` states the draws and the effect of each enchantment.

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
BRK-LOOT-005. A creaking heart with `natural=false` drops no experience. The server pays this
experience before it removes the block. Thus its orbs reach the client before the items of the
break, as BDR-ORD-001 of `block-drops.md` states. See experiment E63.

## 8. Intentional deviations

**D2.**

- **Rule**: BRK-SRV-010.
- **Reference**: the reference keeps the position and the start tick of a dig after the dig ends. A
  finish action at that position with no new start uses them. The progress since the old start is
  large, so a new block at that position breaks at once, with drops and tool damage. This also
  occurs after the dig ends by an abort action at another position or because the block became air. An abort action with no dig also sends `block_destruction` with the progress 255 for
  the kept position.
- **Visible to the client**: yes. A finish action with no start breaks the block.
- **Test**: a differential test must not send a finish action without a start action.

**D3.**

- **Rule**: BRK-SRV-009 and BRK-SRV-011.
- **Reference**: a delayed break ends only when it breaks the block or when the block becomes air. A
  finish action on a block with a hardness of -1 thus makes a delayed break that never ends. When
  another block that is not air is set at the position later, the delayed break continues with that
  block. The observer gets a stage for it, and the block can break with no action of the player.
- **Visible to the client**: yes.
- **Test**: a differential test must not send a finish action on a block with a hardness of -1, and
  must not change the block of a delayed break.

**D5.**

- **Rule**: BRK-SRV-009.
- **Reference**: while a delayed break exists, a dig does not do the steps of BRK-SRV-004. The
  observer gets only the stage of the start action, and nobody gets `level_event`. A finish action on
  the dig still breaks the block, because the reference calculates the progress from the start tick
  (entry D4). A start action and an abort action do not end the delayed break.
- **Visible to the client**: yes. Other players see no crack for the new dig.
- **Test**: a differential test must not start a dig while a delayed break exists.

**D4.**

- **Rule**: BRK-PRG-001 and BRK-ITM-002.
- **Reference**: the reference multiplies the rate of the current tick by `c + 1`, as a product in
  32-bit floating point. With a rate that does not change, the product and the sum of BRK-PRG-001
  can give a stage one tick apart. For stone with an empty hand, the product reaches the stage 2 at
  the tick count 29, and the sum at 30. A change of an input changes the progress of the earlier
  ticks too. The vanilla
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
- **U13**: blocks that act differently in a creative break and that no experiment tests.
- **U22**: the packet order of a delayed break (BRK-SRV-009). Experiment E45 can show the order.

## 10. Experiments

Use a vanilla server of the game version of `assets/minecraft/version.json`, with
`online-mode=false`, `level-type=minecraft:flat` and `network-compression-threshold=-1`. Use
`spawn-protection=0`, except where a step states another value. Set the game rule `spawn_mobs` to
false. Connect one scripted client as the breaker and one as the observer, both in creative mode.
Before each step, set the block with `setblock`, and read the packets that arrive after the step.

| Experiment | Steps | Rule |
|------------|-------|------|
| E28 | Send the start action before the client sends `player_loaded`. Then send `player_loaded` and send the start action again. | BRK-REQ-005 |
| E29 | Put the observer 31 blocks and then 33 blocks from the block corner. Send action 2. | BRK-REQ-006 |
| E30 | Break each block of BRK-REF-004 without `op`, with operator level 1 and with operator level 2. Read the packets of the observer. | BRK-REF-004 |
| E31 | Set water and lava at the levels 9 and 15. Send the start action. | BRK-ACK-005 |
| E32 | Hold a diamond sword. Send the start action on air, on water and at `y=-65`. Send actions 1, 2 and 3 at `y=320`. With an empty hand, send the start action at `y=-65` and read the observer. | BRK-ORD-001, BRK-ORD-002 |
| E33 | Set `spawn-protection=16` and `setworldspawn 0 -60 0`. Break blocks on the `z` axis, on a diagonal, with an operator of level 1, and in the nether. | BRK-SPN-001 |
| E45 | Start a dig on stone with an empty hand. Send the finish action at the tick count 5. Read the block, the stages and the packets of the break until the tick count 160. Repeat, and set the block to dirt at the tick count 20. Repeat with a second dig, and send a finish action on it at the tick count 5. | BRK-SRV-009 |
| E46 | Start a dig on stone with a wooden pickaxe in slot 0. Change the selected slot with `set_carried_item`, with no new start action. Read the stages. | BRK-PRG-001, BRK-ITM-002 |
| E47 | With a vanilla client and a proxy, start a dig on stone with a wooden pickaxe. Select another hotbar slot during the dig. Read the actions that the client sends. | BRK-ITM-001 |
| E59 | In survival mode, send the start action on air 2 blocks away. Read the packets of the observer for 3 ticks. | BRK-SRV-014 |
| E60 | Start a dig on stone. After 10 ticks, send the start action at the same position. Read the packets and the stages. Then start a dig on stone, and send the start action on short grass. Read the stages of the stone. | BRK-SRV-015 |
| E61 | Start a dig on stone at the distance 5.4. Move the player to 5.6. Send the abort action. Read the packets of the observer. Repeat with no dig. | BRK-SRV-016 |
| E62 | Set the game rule `block_drops` to false. Break stone and coal ore with a stone pickaxe, and a chest with contents. Then set the game rule to true, and break a furnace with contents with an empty hand. | BRK-LOOT-005 |
| E63 | Break a creaking heart with `natural=true` 20 times with an iron axe, with Silk Touch and without it. Break one with `natural=false`. Sum the values of the experience orbs. | BRK-LOOT-006 |
| E64 | Set a piston with `extended=true` and `facing=up`, and a piston head with `facing=up` and `type=normal` on it. Send the start action on the head. Repeat with a head with `facing=north`. Read the blocks. | BRK-PRT-007 |

