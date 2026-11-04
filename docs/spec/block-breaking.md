# Block breaking: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

This document states how a player breaks a block in creative mode, and what the server sends for
it. It states only what a client, the server console and the server files show. It does not
prescribe an architecture. Every rule comes from a vanilla server in offline mode. Section 8 gives
the experiments, and section 9 gives the captured results.

Scope: creative mode only. This document does not state break progress, tools, survival mode or
block placement. It will grow to cover them.

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
- **Eye position**: the position of the player, with 1.62 added to `y`.
- **Break**: the server changes the block at the position to air.
- **Refusal**: the block at the position does not change.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 9.
- `unresolved`: not decided. Section 7 collects these items.

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
| 2 | An acknowledgement to the breaker. Each observer gets `block_destruction` with progress 255. |
| 3 | An acknowledgement only. |

**BRK-REQ-003** (`black_box_confirmed`, wire). An action of 9 closes the connection, because the
packet does not decode. The block does not change. See result R3. This rule rests on one observation.

**BRK-REQ-004** (`black_box_confirmed`, semantic). The face has no effect on a break. The values 0, 1,
2, 5 and 9 all break the block. See results R1 and R4.

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

**BRK-BRK-006** (`black_box_confirmed`, semantic). An observer gets the `level_event` only within a
distance. An observer gets the new block state while its client holds the chunk. See result R10. The
limit of the `level_event` is between 63 and 65 blocks along one axis (U1).

**BRK-BRK-007** (`black_box_confirmed`, semantic). The server answers no later than about one tick
after the request. The largest delay in result R9 is 49.9 ms. The exact tick is unresolved (U2).

**BRK-BRK-008** (`black_box_confirmed`, semantic). Bedrock, barrier, end portal frame, structure void,
light, moving piston, nether portal and end portal break like any other block. See result R5.

**BRK-BRK-009** (`black_box_confirmed`, semantic). A start action on fire extinguishes it. The breaker
and each observer get `level_event` with type 1009, and then the new block state. They do not get type
2001. See result R5. This rule rests on one block type.

## 3. Refusals

### 3.1 Reach

**BRK-RCH-001** (`black_box_confirmed`, semantic). A start action is refused when the distance from
the eye position to the nearest point of the block cube is 6.0 or more. A distance less than 6.0 is in
reach. See results R7 and R8. The breaker gets one acknowledgement and no block state. An observer
gets nothing.

**BRK-RCH-002** (`black_box_confirmed`, semantic). The distance uses the block cube, and not the
shape of the block. A bottom slab has the same limit as a full block when the breaker is above it. See
result R8.

### 3.2 Refusals that send the block state

**BRK-REF-001** (`black_box_confirmed`, semantic). In each case of this table, the breaker gets
`block_update` with the current state of the block, and then one acknowledgement. An observer gets
nothing where the table states it.

| Case | Result | Observer |
|------|--------|----------|
| The selected hotbar slot holds an item that cannot break blocks (BRK-REF-002) | R11 | Gets nothing, R11 |
| The player is in adventure mode or spectator mode | R14 | Gets nothing, R14 |
| The position is outside the world border | R15 | Not observed |
| The position is above the height limit, and in reach | R15 | Not observed |

For the position above the height limit, the state id in the packet is 18655, and not 0.

**BRK-REF-002** (`black_box_confirmed`, semantic). All swords, the trident, the mace and the debug stick
refuse a break. An axe, a hoe, shears, a spear, a pickaxe, a stick and an empty hand do not refuse a
break. Only the selected hotbar slot counts. The other hotbar slots and the off hand have no effect.
See result R11. The list of items outside these results is unresolved (U3).

**BRK-REF-003** (`black_box_confirmed`, semantic). A command block, a structure block and a jigsaw
block refuse a break when the player is not an operator. An operator breaks them. See result R16. In a
refusal, the breaker gets `block_update` with the current state two times. For a structure block and a
jigsaw block, the breaker then gets `block_entity_data`. The acknowledgement comes last. The operator
level that this needs is unresolved (U4).

### 3.3 Refusals that send only an acknowledgement

**BRK-ACK-001** (`black_box_confirmed`, semantic). In each case of this table, the breaker gets one
acknowledgement and no block state.

| Case | Result |
|------|--------|
| The block is out of reach (BRK-RCH-001) | R7 |
| The position is air | R17 |
| The position holds water or lava, at any level | R18, R25 |
| The position is below the height limit | R15 |

**BRK-ACK-002** (`black_box_confirmed`, semantic). For a start action on air, an observer gets
`level_event` with type 2001 and data 0. An observer gets no block state. See result R17. This rule
rests on one observation.

**BRK-ACK-003** (`black_box_confirmed`, semantic). A block that holds a water source changes to a water
source. The breaker gets `block_update` with the state of a water source, and then one acknowledgement.
An observer gets `level_event` with type 2001 and the state id of the old block, and then the same block
state. No item drops. See results R19 and R26. The tested blocks are in R26. A block of the same type
with no water changes to air. Each block type rests on one observation.

**BRK-ACK-004** (`black_box_confirmed`, semantic). For a start action on water or lava, at any level, an
observer gets `level_event` with type 2001 and the state id of the fluid. An observer gets no block
state. The fluid stays. See result R25.

### 3.4 Spawn protection

**BRK-SPN-001** (`black_box_confirmed`, semantic). With `spawn-protection=16`, a start action is
refused when all of these conditions are true. See results R20 and R21.

- At least one operator exists.
- The player is not an operator.
- The block position is at most 16 blocks from the world spawn block on the `x` axis, in each direction.

The limit on the `z` axis is unresolved (U5). An operator breaks the block, and the break is normal. A
server with no operator does not refuse, as result R21 shows.

**BRK-SPN-002** (`black_box_confirmed`, wire). In this refusal, the breaker gets `system_chat` in the
overlay position, and then one acknowledgement. The breaker gets no block state. The message is the
translation key `build.spawn_protection` in red, with the argument `x, y, z`. See result R20. An
observer gets nothing.

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

## 6. Several requests and the sequence

**BRK-SEQ-001** (`black_box_confirmed`, semantic). Requests that the server handles in one tick get one
acknowledgement. It carries the highest sequence of those requests, whichever came last. See result R6.

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

## 7. Unresolved

- **U1**: the exact distance limit of the `level_event` to an observer, and how it counts distance.
- **U2**: the tick in which the server handles a request and sends the answer. The results show only
  arrival times at the client.
- **U3**: the full list of items that refuse a break. The experiments tested the items of R11.
- **U4**: the operator level that a command block, a structure block and a jigsaw block need. The
  experiments used the default level of the `op` command.
- **U5**: spawn protection on the `z` axis, on a diagonal, with a spawn that is not at the origin, and
  the operator level that it needs.
- **U7**: a position in reach in a chunk that is not loaded. The only tested position was also out of
  reach.
- **U8**: what an observer gets for a start action outside the world border, or above the height limit.
- **U9**: the eye position when the player sneaks, swims or has another pose, and the effect of an
  attribute that changes reach. All experiments used a player that stands. Result R8 confirms the
  height 1.62 only to within 0.05.
- **U10**: why a position above the height limit sends a block state and a position below it does not.
  Each case rests on two observations.
- **U11**: two breaks in one tick in different sections.
- **U12**: blocks that hold water outside R26.

## 8. Experiments

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

## 9. Captured results

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
