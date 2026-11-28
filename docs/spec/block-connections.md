# Block connections: behavioural specification

| Field | Value |
|-------|-------|
| Version | see `assets/minecraft/version.json` |

A fence, a glass pane, bars, a wall and a fence gate change their shape with their neighbours. This
document states which neighbour gives a connection, what the connection does to the block state, and
when the state changes. It states only what a player, a client, a data file and a save file can see.

The machinery that delivers a shape update to a block is in `block-simulation.md`, section 4
(SIM-SHP-001 to SIM-SHP-004). The face tests of a support shape are in `block-support.md`, section 1
(SUP-FACE-001, SUP-FACE-002). The states that a placement gives are in `block-placing.md`
(PLC-STA-006).

## 0. Conventions

### 0.1 Terms

- **Subject**: the block at the position P whose connections this document states: a fence, a pane,
  bars, a wall or a fence gate.
- **Side**: one of the four horizontal directions `W`, `E`, `N` and `S`, as `block-simulation.md`,
  section 0.3, defines them. The property of the state with that name holds the connection of the
  side. `D` and `U` are down and up.
- **Neighbour at a side**: the state at P + side.
- **Connected**: a side of a fence, a pane or bars is connected when its property is `true`. A side of a
  wall is connected when its property is `low` or `tall`.
- **Fence**: a block of the tag `#minecraft:fences`. **Wooden fence**: a block of the tag
  `#minecraft:wooden_fences`. **Other fence**: a fence that is not a wooden fence.
- **Gate**: a block of the tag `#minecraft:fence_gates`. The **axis** of a gate is the axis of its
  `facing` property: `north` and `south` give the axis `z`, `east` and `west` give the axis `x`.
- **Pane**: `glass_pane` and each block `<colour>_stained_glass_pane`, for the sixteen colours.
- **Bars**: a block of the tag `#minecraft:bars`.
- **Wall**: a block of the tag `#minecraft:walls`.
- **Full face**: a face of the support shape of a state that passes the test `full` of SUP-FACE-001.
  The support shape is SUP-FACE-002.
- **Excepted block**: a block of the set of CON-DATA-001. An excepted block gives no connection by its
  faces.
- **Bottom area**: the face area of the collision shape of a state on its bottom face, as
  `block-support.md`, section 0.1, defines a face area. It is a set of points on the square of 16 × 16
  sixteenths. The axes of the square are `x` from west to east and `z` from north to south.
- **Post area**: the square from 7 to 9 on both axes of the bottom area.
- **Arm area of a side**: the strip from 7 to 9 across the side, from the centre to the edge of the
  side. For `E`: `x` from 7 to 16 and `z` from 7 to 9. For `W`: `x` from 0 to 9 and `z` from 7 to 9.
  For `S`: `z` from 7 to 16 and `x` from 7 to 9. For `N`: `z` from 0 to 9 and `x` from 7 to 9.
- **Covers**: a bottom area covers an area when it holds each point of that area.
- **Line**: the set of connected sides of a wall is a line when it is exactly `W` and `E`, exactly
  `N` and `S`, or all four sides.

### 0.2 Provenance

- `black_box_confirmed`: shown by an experiment on a vanilla server. The experiments of section 8
  wrote the fixture `crates/mcrs_minecraft_block_support/tests/fixtures/vanilla/connections.txt`. A
  line of the fixture that starts with `#INVALID` or `#UNSTABLE` is void; the text in brackets gives
  the reason and the valid repeat.
- `derived`: a consequence of other rules of this document. No record shows it on its own.
- `design`: a choice of mcrs. The rule states the reason.
- `unresolved`: not decided. Section 7 collects these items.

### 0.3 ID prefix

Rules of this document have the prefix `CON-`. The topics are these:

- `FENCE`: the sides of a fence.
- `PANE`: the sides of a pane and of bars.
- `WALL`: the sides and the post of a wall.
- `GATE`: the `in_wall` property of a gate.
- `RUN`: when a connection is calculated.
- `DATA`: the data that the rules read.

## 1. Fences

**CON-FENCE-001** (`black_box_confirmed`, semantic, confidence high). A side of a fence is connected
when the neighbour at that side satisfies one of these conditions:

1. The neighbour is a wooden fence, and the subject is a wooden fence.
2. The neighbour is an other fence, and the subject is an other fence.
3. The neighbour is a gate, and the axis of the gate is not the axis of the side. For the sides `E` and
   `W`, the gate has `facing=north` or `facing=south`. For the sides `N` and `S`, the gate has
   `facing=east` or `facing=west`. The `open` and `in_wall` properties of the gate do not matter.
4. The neighbour is not an excepted block, not a fence and not a gate, and the face of the neighbour
   that looks at P is a full face.

If not, the side is not connected.

Observed (fixture, sections 1, F, F2 and G):

- `oak_fence` next to `nether_brick_fence`, and the reverse: no connection.
- A fence next to `glass_pane`, `iron_bars`, `copper_bars`, `cobblestone_wall` and `andesite_wall`:
  no connection. Their faces are not full.
- Condition 4 gives a connection to: stone, glass, ice, `honey_block`, `soul_sand`, `snow[layers=8]`,
  the back of stairs, a double slab, a closed door whose panel looks at P, an open trapdoor whose
  panel looks at P, a piston that is not extended, the back of an extended piston, the plate of a
  piston head that looks at P, and a composter.
- Condition 4 gives no connection to: `light`, `structure_void`, a chest, a cauldron, a hopper,
  farmland, `dirt_path`, `snow[layers=7]`, a cactus, a chain, a lantern, a bed, a sign, a lectern, a
  carpet, a pressure plate, water and lava.

**CON-FENCE-002** (`black_box_confirmed`, semantic, confidence high). The states at P + U and at P + D
do not change the sides of a fence (fixture, section 1.6). The `waterlogged` property of a fence does
not change its sides, and a side does not change `waterlogged` (fixture, section F6).

## 2. Panes and bars

**CON-PANE-001** (`black_box_confirmed`, semantic, confidence high). A side of a pane or of bars is
connected when the neighbour at that side satisfies one of these conditions:

1. The neighbour is a pane, bars or a wall.
2. The neighbour is not an excepted block, not a fence and not a gate, and the face of the neighbour
   that looks at P is a full face.

If not, the side is not connected.

Observed (fixture, sections 3, F and G):

- A pane and bars next to each fence, and next to each gate `facing` and `open` value: no connection.
- Condition 1 gives a connection to `glass_pane`, `white_stained_glass_pane`, `iron_bars`,
  `copper_bars`, `cobblestone_wall` and `andesite_wall`.
- Condition 2 gives the connections of CON-FENCE-001, condition 4, for each candidate of the
  fixture.

## 3. Walls

**CON-WALL-001** (`black_box_confirmed`, semantic, confidence high). A side of a wall is connected
when the neighbour at that side satisfies one of these conditions:

1. The neighbour is a wall, a pane or bars.
2. The neighbour is a gate, and the axis of the gate is not the axis of the side, as CON-FENCE-001,
   condition 3, states it.
3. The neighbour is not an excepted block, not a fence and not a gate, and the face of the neighbour
   that looks at P is a full face.

If not, the side is `none`. A wall next to each fence has no connection (fixture, section 4.1).

**CON-WALL-002** (`black_box_confirmed`, semantic, confidence high). A connected side of a wall is
`tall` when the bottom area of the state at P + U covers the arm area of that side. If not, the
connected side is `low`.

The bottom area reads the collision shape, not the support shape. The excepted blocks are not excepted
here.

Observed (fixture, sections 4.3, F3, G and H):

- An empty bottom area covers nothing: a top slab, a top trapdoor, stairs with `half=top`, a lantern
  with `hanging=true`, a chain with `axis=x` or `axis=z`, and a block with no collision shape.
- Each arm area is covered by: stone, glass, leaves, a bottom slab, a bottom trapdoor, stairs with
  `half=bottom`, `soul_sand`, a carpet and a lectern.
- A closed gate covers the arm areas of the two sides across its axis, and not the two along it.
- A door covers no arm area.
- `honey_block`, a chest and a cake cover the post area and no arm area. `honey_block` has a full
  support shape and a collision shape that does not reach the edges, and it makes no side `tall`.
- Leaves above a wall make its sides `tall`.

**CON-WALL-003** (`black_box_confirmed`, semantic, confidence high). The `up` property of a wall is
calculated after its sides, in this order:

1. If the block at P + U is in the tag `#minecraft:wall_post_override`, `up` is `true`.
2. If the connected sides are not a line, `up` is `true`.
3. If the bottom area of the state at P + U covers the post area, and no two opposite connected sides
   are both `tall`, `up` is `true`.
4. If not, `up` is `false`.

Observed (fixture, sections 4.2, 4.3, 4.4, F3, G and H):

- No connected side, one side, two sides that are not opposite, or three sides: the wall has a post.
- The sides `E` and `W` and nothing above: no post.
- A torch, a sign, a banner, a pressure plate, tripwire or `brown_mushroom` above: a post. These
  blocks are in `#minecraft:wall_post_override` (CON-DATA-002). `short_grass` above, which is not in
  the tag and has no collision shape: no post.
- A pane, bars, a fence, an end rod, a lantern with `hanging=false`, a chain with `axis=y`, a
  lightning rod, a flower pot, a hopper, a chest, a cake, `honey_block` or a wall with `up=true`
  above: the post comes back.
- Stone, glass, leaves, a bottom slab, a carpet, a lectern, `soul_sand` or a closed gate across the
  line above: the sides are `tall` and the post stays away.
- A wall above that has no post and arms along the line: the sides are `tall` and the post stays
  away. A wall above that has no post and arms across the line: the post comes back.
- Three connected sides of which two opposite sides are `tall`: a post (section F3).

**CON-WALL-004** (`black_box_confirmed`, semantic, confidence high). The state at P + D does not change
a wall (fixture, section F4). A wall below a wall reads only the arm areas and the post area of the
wall above it, as CON-WALL-002 and CON-WALL-003 state (fixture, section 4.5).

## 4. Fence gates

**CON-GATE-001** (`black_box_confirmed`, semantic, confidence high). The `in_wall` property of a gate
is `true` when a wall is at one of the two sides across the axis of the gate, or at both. For a gate
with `facing=north` or `facing=south`, these sides are `W` and `E`. For a gate with `facing=east` or
`facing=west`, these sides are `N` and `S`.

Observed (fixture, sections 5 and F5): a fence at these sides, a wall at the two sides along the
axis, and a wall at P + U or P + D keep `in_wall` false. With walls at both sides, `in_wall` stays
`true` until the second wall goes.

## 5. When a connection is calculated

**CON-RUN-001** (`black_box_confirmed`, semantic, confidence high). A set of a subject with
`setblock` calculates each side that the command does not give, from the neighbours at that time. A
side that the command gives stays as given. A set with `strict` calculates no side.

Observed (fixture, sections 2.1 and F7): `oak_fence` with no properties next to stone gets
`east=true`; `oak_fence[east=false]` next to stone keeps `east=false`; `oak_fence[north=true]` with
nothing at the north keeps `north=true`; `oak_fence` with `strict` next to stone keeps `east=false`.
The sides that a placement by a player gives are PLC-STA-006 and U5.

**CON-RUN-002** (`black_box_confirmed`, semantic, confidence high). A shape update from a side
calculates that side again, from the neighbour at that side. The other three sides stay as they are.
For a wall, the update also calculates the height of that side (CON-WALL-002) and the `up` property
(CON-WALL-003). For a gate, an update from a side across its axis calculates `in_wall` again.

Observed (fixture, sections 2.1 and F7): `oak_fence[north=true]` with nothing at the north keeps
`north=true` when stone is set at the south, and `oak_fence[east=false]` next to stone gets
`north=true` and keeps `east=false` when stone is set at the north.

**CON-RUN-003** (`black_box_confirmed`, semantic, confidence high). A shape update from `U`
calculates the height of each connected side of a wall and its `up` property again (fixture, section
F3: stone above the wall removed, each `tall` side becomes `low`). A shape update from `D` or `U`
changes no side of a fence (CON-FENCE-002) and no property of a gate (CON-GATE-001). For a pane and
bars this is `derived`: CON-PANE-001 reads only the neighbour at the side.

**CON-RUN-004** (`black_box_confirmed`, semantic, confidence high). A change of the neighbour in its
cell is a shape update as a set or a removal is. Stone replaced by leaves disconnects the side, leaves
replaced by stone connects it (fixture, section 2.5). A gate that opens or closes keeps its axis, so
the sides next to it do not change (fixture, section 2.2). A trapdoor that opens or closes changes its
panel, so the side next to it changes with CON-FENCE-001, condition 4 (fixture, section 2.3).

**CON-RUN-005** (`black_box_confirmed`, semantic, confidence high). A side that changes is a change of
the block state at P. An observer that looks at P gives a pulse when a side changes, and no pulse when a
neighbour changes without a change of a side (fixture, section G2).

## 6. Data

**CON-DATA-001** (`design`, data, confidence high). The excepted blocks are these: `barrier`,
`carved_pumpkin`, `jack_o_lantern`, `melon`, `pumpkin`, each block of `#minecraft:shulker_boxes` and
each block of `#minecraft:leaves`. The data pack has no tag for this set, so mcrs keeps the set as data
of the behaviour, with the tags read from the data pack. The leaves have an empty support shape
(SUP-FACE-001), so their faces give no connection also without this set; the set names them so that a
data pack that gives leaves a support shape keeps the vanilla result.

Observed (fixture, sections 1, 3 and 4.1): a fence, a pane, bars and a wall next to `barrier`,
`pumpkin`, `carved_pumpkin`, `jack_o_lantern`, `melon`, `shulker_box`, `white_shulker_box`,
`oak_leaves` and `azalea_leaves` have no connection at that side. A `conduit` gives no connection
either, because its faces are not full.

**CON-DATA-002** (`design`, data, confidence high). The tags `#minecraft:fences`,
`#minecraft:wooden_fences`, `#minecraft:fence_gates`, `#minecraft:walls`, `#minecraft:bars`,
`#minecraft:shulker_boxes`, `#minecraft:leaves` and `#minecraft:wall_post_override` come from the
loaded data pack. A data pack that changes a tag changes the connections. The panes have no tag in the
data pack, so mcrs names them by their block ids.

In the shipped data pack, `#minecraft:wall_post_override` holds `torch`, `soul_torch`,
`redstone_torch`, `copper_torch`, `tripwire`, `cactus_flower`, `brown_mushroom`, `red_mushroom`, the
signs, the banners and the pressure plates. `#minecraft:bars` holds `iron_bars` and the copper bars.

## 7. Unresolved

Each item names the experiment that decides it.

- **U1**: a wall with a post and with arms along the line of the wall below it. CON-WALL-002 and
  CON-WALL-003 read its collision shape, so the sides below become `tall` and the post below stays away.
  No experiment set this case. Experiment E8.
- **U2**: an open shulker box. CON-DATA-001 excepts it in both states, and the author's knowledge of
  the game says the same. The experiments ran without a player and could not open a box. Experiment E9.
- **U3**: the oxidised and waxed copper bars, and the walls other than `cobblestone_wall` and
  `andesite_wall`. The rules read the tags `#minecraft:bars` and `#minecraft:walls`. Experiment E10.
- **U4**: a gate with `powered=true` next to a fence or a wall. CON-FENCE-001, condition 3, reads
  only the axis of the gate. No record has a powered gate. Experiment E3.
- **U5**: the sides of a subject that a player places by hand. CON-RUN-001 shows a set with
  `setblock` and no properties. Experiment E11.

## 8. Experiments

Use a vanilla server of the game version of `assets/minecraft/version.json`, with
`online-mode=false` and `level-type=minecraft:flat`, in creative mode. The experiments send commands to
the console only. No client is necessary. Set each block with `setblock` without `strict`: a `strict`
set calculates no side of the subject and sends no shape update to the neighbours. Read each property
with `execute if block <P> <block>[<property>=<value>] run say <value>`. Write each result as one
line of the fixture: the subject, the step and the full state.

| Experiment | Steps |
|------------|-------|
| E1 | Set the subject at P. For each candidate of the fixture, section 1: set the candidate at P + E, record the state of the subject, remove the candidate, record the state again. Do it with `oak_fence`, `nether_brick_fence`, `glass_pane`, `iron_bars` and `cobblestone_wall` as the subject. |
| E2 | Set `oak_fence[north=true]` at P with no block at P + N. Set stone at P + S, record, remove it, record. Set `oak_fence` with all sides `true` and no neighbours; set stone at P + S and at P + U; record. |
| E3 | Set `oak_fence` at P and a gate, a trapdoor, stone at P + E. Open and close the gate and the trapdoor with `setblock`. Set the gate with `powered=true`. Replace the stone with `oak_leaves` and back. Record after each step. |
| E4 | Set an observer that looks at P, a lamp and a dropper that count its pulses. Set and remove stone, leaves and a gate at P + E; open the gate. Record the pulses. |
| E5 | Set `cobblestone_wall` at P with walls at no side, one side, two opposite sides, two sides that are not opposite, three sides and four sides. Record `up`. |
| E6 | With walls at `E` and `W`, at all four sides, and at `E` only: set each candidate of the fixture, section 4.3, at P + U. Record the sides and `up` before and after. |
| E7 | Set `oak_fence_gate` with `facing=north` and with `facing=east` at P. Set `cobblestone_wall`, then `oak_fence`, at each of the six neighbours in turn. Record `in_wall` after each set and after each removal. Set walls at both sides across the axis, remove them one by one. |
| E8 | Set `cobblestone_wall` at P with walls at `E` and `W`. At P + U, set a wall with walls at `E` and `W` of it and a torch above it, so that the wall above has `up=true` and arms along the line. Record the wall at P. |
| E9 | With a client, open a shulker box next to a fence, a pane and a wall. Record the subject while the box is open. |
| E10 | Do E1 with `oxidized_copper_bars` and `waxed_copper_bars` as the candidate and as the subject, and with `tuff_wall` as the subject. |
| E11 | With a client, place a fence, a pane, bars, a wall and a gate by hand next to stone, next to a wall and with nothing around. Record the state. |

E1, E2, E4 to E7 ran on 2026-11-22, and E3 ran without the powered gate; their records are the
fixture. E8 to E11 have not run.

## 9. Intentional deviations

None.
