# Pathfinding: the ground path search and the path follower

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states how a mob that walks finds a path and follows it. It states the path types of
the blocks, the malus of each path type, the rules of the ground node evaluator, the path search,
the navigation requests and the path follower. It also states the move control, which changes a
path into the input of the walker tick of `entity-physics.md`.

The vanilla client does not compute paths. It sees only the positions and the rotations of the mobs.
Thus no rule of this document is of the kind "client agreement". The rules are of the kind
"semantic": the result is the motion of the mob that a player sees. Section 13 records where this
server selects a clean rule in place of an accident of the reference.

`entity-ai-timing.md` states when the parts of this document run, with the prefix `AIT`. This
document states what they compute. `gold-farm.md`, step G4, is the reason for this document.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **position**, **box**,
  **velocity**, **step height**, **on-ground flag**, **horizontal collision flag**, **input
  vector**, **jump input**, **travel step**, **f32**, **f64**, **widen**, **`f32(v)`**: as
  `entity-physics.md`, section 0.1, defines them.
- **Mob**, **goal**, **path**, **path search**, **next node**, **guidance**, **path follower**,
  **move control**, **jump control**, **follow range**, **decision**, **reflex**, **issue tick**,
  **snapshot**, **order key**: as `entity-ai-timing.md`, section 0.1, defines them.
- **Block position**: the integer position `(floor(x), floor(y), floor(z))` of a point.
- **Mob block position**: the block position of the position of a mob.
- **Air**: a state of the block `air`, `cave_air` or `void_air`.
- **Path type**: one of the 27 values of the table of PATH-TYP-001. A path type tells a mob what a
  block position means for a walk.
- **Malus**: an f32 cost that a mob adds when a path enters a node of one path type. A negative
  malus means that the mob must not enter the node.
- **Malus table**: the malus of each path type for one mob (PATH-MAL-001).
- **State path type**: the path type of one block state, without the blocks around it
  (PATH-TYP-002).
- **Block path type**: the path type of one block position. It reads the block below and the
  blocks around it (PATH-TYP-003).
- **Mob path type**: the path type of one block position for one mob. It reads each block position
  that the box of the mob covers (PATH-TYP-004).
- **Partial collision type**: one of the path types `FENCE`, `DOOR_WOOD_CLOSED` and
  `DOOR_IRON_CLOSED`.
- **Node**: one block position in a path search, with the fields of PATH-SRC-002.
- **Start node**: the node where a path search starts (PATH-NOD-001).
- **Target**: one block position that a path search tries to reach.
- **Reach range**: the largest Manhattan distance from a node to a target at which the search ends.
- **Path length limit**: the value `L` of PATH-SRC-001.
- **Node budget**: the number of node pops after which a search stops (PATH-SRC-001).
- **Search region**: the block columns that a search can read (PATH-REQ-003).
- **Floor level**: the `y` of the top of the floor under a block position (PATH-NOD-002).
- **Jump size**: the number of blocks that a node evaluator can step up from one node
  (PATH-NOD-003).
- **Jump height**: the largest difference of floor levels for a step to a neighbour
  (PATH-NOD-003).
- **Fall limit**: the largest number of blocks that a mob can fall from one node to the next
  (PATH-NOD-008).
- **Probe**: one call of the procedure of PATH-NOD-005 for one block column next to a node.
- **Candidate**: the result of a probe: no node, or a node with a path type and a malus.
- **Open set**: the nodes that the search can pop next (PATH-SRC-003).
- **Closed node**: a node that the search popped, or a node that a probe marked as closed
  (PATH-NOD-005).
- **Best node of a target**: the node with the smallest heuristic distance to that target, of the
  nodes that got a cost (PATH-SRC-004).
- **Reached path**: a path whose end node is in the reach range of a target.
- **Partial path**: a path that ends at a best node because no node reached a target.
- **Navigation**: the state of one mob that holds its path, the target of the path, the speed
  modifier, the timers of PATH-FOL and the recompute state of PATH-RCP.
- **Speed modifier**: the factor that a request gives. The move control multiplies the movement
  speed by it.
- **Wanted position**: the point that the path follower gives to the move control in one tick.
- **Move operation**: the state of the move control: `wait`, `move_to`, `strafe` or `jump`.
- **Mob speed**: the f32 value that the move control sets last. The walker tick uses it as the
  speed of PHY-GRV-010.
- **Temporary position**: the point `(x, floor(y + 0.5), z)` of a mob that a navigation on the
  ground uses for its checks (PATH-FOL-001).
- **Navigating mob**: a mob whose navigation has a path that is not done.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 16.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a captured result in section 16.
- `source_suggested`: taken from the reference implementation, with no captured result. The
  confidence is at most medium. Section 15 attaches an experiment.
- `design`: a rule of this server's own design. A test of section 15.2 confirms it.
- `unresolved`: not decided. Section 14 collects these items.

Each rule states its kind and its confidence, for example
`(source_suggested, semantic, confidence: medium)`. The kinds are those of `docs/spec/README.md`.

### 0.3 ID prefix

Each rule has an ID with the prefix `PATH`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `PATH-TYP` | path types of states, of blocks and of mobs |
| `PATH-MAL` | the malus of each path type |
| `PATH-NOD` | the ground node evaluator: start node, neighbours, steps, falls |
| `PATH-SRC` | the path search |
| `PATH-REQ` | navigation requests and the search region |
| `PATH-FOL` | the path follower |
| `PATH-MOV` | the move control and the jump control |
| `PATH-RCP` | the recompute of a path |
| `PATH-DET` | determinism and the place in the architecture |
| `PATH-PRF` | cost and caches |
| `PATH-DEV` | intentional deviations |

### 0.4 Scope

This document states:

- the path types and the malus table;
- the ground node evaluator, which each mob with ground navigation uses;
- the path search, its limits and its result;
- the navigation requests of a mob on the ground;
- the path follower, the move control and the jump control;
- the recompute of a path after a block change;
- the determinism, the cost and the caches.

These subjects are out of scope:

- The swim, fly and amphibious node evaluators. They use other neighbour rules. PATH-NOD-010 states
  the hooks that keep the model open for them.
- The goals and behaviours that make requests, for example the turtle egg goal or the melee attack.
  `mob-goals.md` (step G5) states them. This document states only the request.
- The look control and the body rotation. They do not change the motion.
- The door goals (AIT-VAN-008). This document states only how the evaluator sees a door.
- The navigation of a mob that a player rides, and the malus of a vehicle that a passenger reads.
- The movement physics. `entity-physics.md` states the walker tick.

### 0.5 Artefacts cited

- `assets/mcrs/block_definition/<block>.json`: the block definition corpus. It gives the
  collision shape of each state (PHY-SHP-002), the fluid of each state (`mcrs:fluid_state`) and the
  properties `open` and `lit`.
- The data pack: the block tags `pathfinding/open`, `pathfinding/drop_down`,
  `pathfinding/powder_snow`, `pathfinding/rails`, `pathfinding/damaging`, `pathfinding/leaves`,
  `pathfinding/sticky`, `pathfinding/avoid_in_air`, `pathfinding/damage_cautious`, `fire`,
  `campfires`, `fences`, `walls`, `doors`, `cauldrons` and `air`. In `26.4-snapshot-7`,
  `pathfinding/drop_down` holds `#trapdoors`, `lily_pad` and `big_dripleaf`.
- The corpus does not give the flag "pathfindable on land", the door kind or the legacy solid flag
  of a state. Section 12.3 states the component that the corpus must add.

## 1. Path types

**PATH-TYP-001** (`source_suggested`, semantic, confidence: medium). These are the path types,
with their order number and their default malus. The order number is used by PATH-TYP-004.

| No. | Path type | Default malus | No. | Path type | Default malus |
|-----|-----------|---------------|-----|-----------|---------------|
| 0 | `BLOCKED` | -1 | 14 | `FIRE` | 16 |
| 1 | `OPEN` | 0 | 15 | `DAMAGING_IN_NEIGHBOR` | 8 |
| 2 | `WALKABLE` | 0 | 16 | `DAMAGING` | -1 |
| 3 | `WALKABLE_DOOR` | 0 | 17 | `DOOR_OPEN` | 0 |
| 4 | `DROP_DOWN` | 0 | 18 | `DOOR_WOOD_CLOSED` | -1 |
| 5 | `POWDER_SNOW` | -1 | 19 | `DOOR_IRON_CLOSED` | -1 |
| 6 | `ON_TOP_OF_POWDER_SNOW` | 0 | 20 | `BREACH` | 4 |
| 7 | `FENCE` | -1 | 21 | `LEAVES` | -1 |
| 8 | `LAVA` | -1 | 22 | `STICKY` | 8 |
| 9 | `WATER` | 8 | 23 | `AVOID_IN_AIR` | 0 |
| 10 | `WATER_BORDER` | 8 | 24 | `DAMAGE_CAUTIOUS` | 0 |
| 11 | `RAIL` | 0 | 25 | `ON_TOP_OF_DROP_DOWN` | 0 |
| 12 | `UNPASSABLE_RAIL` | -1 | 26 | `BIG_MOBS_CLOSE_TO_DANGER` | 4 |
| 13 | `FIRE_IN_NEIGHBOR` | 8 | | | |

**PATH-TYP-002** (`source_suggested`, semantic, confidence: medium). The state path type of a block
state is the first match of this list:

1. The tags, in this order: `pathfinding/open` gives `OPEN`; `pathfinding/drop_down` gives
   `DROP_DOWN`; `pathfinding/powder_snow` gives `POWDER_SNOW`; `pathfinding/rails` gives `RAIL`;
   `pathfinding/damaging` gives `DAMAGING`; `pathfinding/leaves` gives `LEAVES`;
   `pathfinding/sticky` gives `STICKY`; `pathfinding/avoid_in_air` gives `AVOID_IN_AIR`;
   `pathfinding/damage_cautious` gives `DAMAGE_CAUTIOUS`.
2. A state whose fluid (`mcrs:fluid_state`) is `minecraft:lava` gives `LAVA`.
3. A fire state gives `FIRE`. A state is a fire state when one of these is true:
   - the block is in the tag `fire`;
   - the block is `lava`, `magma_block` or `lava_cauldron`;
   - the block is in the tag `campfires` and the state has `lit=true`.
4. A door state (the door kind of section 12.3 is not `none`) gives `DOOR_OPEN` when it has
   `open=true`. Else it gives `DOOR_IRON_CLOSED` when the door kind is `not_by_hand`, and
   `DOOR_WOOD_CLOSED` when the door kind is `by_hand`.
5. A state of a block in the tag `fences` or in the tag `walls` gives `FENCE`. A state of a block in
   the tag `fence_gates` with `open=false` also gives `FENCE` (PATH-DEV-005).
6. A state that is not pathfindable on land (section 12.3) gives `BLOCKED`.
7. A state whose fluid is `minecraft:water` gives `WATER`.
8. Each other state gives `OPEN`.

Thus each trapdoor state gives `DROP_DOWN`, open or closed. A `magma_block` gives `FIRE`.

**PATH-TYP-003** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The block path type at `(x, y,
z)` is:

1. Let `t` be the state path type at `(x, y, z)`. If `t` is not `OPEN`, or `y` is the lowest `y`
   of the dimension, the result is `t`.
2. Else let `b` be the state path type at `(x, y - 1, z)`. The result comes from `b`:

   | `b` | Result |
   |-----|--------|
   | `OPEN`, `WATER`, `LAVA`, `WALKABLE` | `OPEN` |
   | `FIRE` | `FIRE` |
   | `DAMAGING` | `DAMAGING` |
   | `STICKY` | `STICKY` |
   | `POWDER_SNOW` | `ON_TOP_OF_POWDER_SNOW` |
   | `DAMAGE_CAUTIOUS` | `DAMAGE_CAUTIOUS` |
   | `DROP_DOWN` | `ON_TOP_OF_DROP_DOWN` |
   | each other path type | the neighbour check of step 3 |

3. The neighbour check visits the positions `(x + dx, y + dy, z + dz)` with `dx`, `dy` and `dz`
   from -1 to 1. It skips the three positions with `dx = 0` and `dz = 0`: the block itself, the
   block above and the block below. Thus it visits 24 positions. The order is `dx` first,
   then `dy`, then `dz`, each from -1 up. The first visited position with one of these state path
   types gives the result:
   - `DAMAGING` gives `DAMAGING_IN_NEIGHBOR`;
   - `FIRE` or `LAVA` gives `FIRE_IN_NEIGHBOR`;
   - `WATER` gives `WATER_BORDER`;
   - `DAMAGE_CAUTIOUS` gives `DAMAGE_CAUTIOUS`.

   When no visited position matches, the result is `WALKABLE`.

Thus the air over a magma block is `FIRE`, and the air over an open trapdoor is
`ON_TOP_OF_DROP_DOWN`. An open trapdoor itself is `DROP_DOWN`.

**PATH-TYP-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The mob path type at `(x, y,
z)` for a mob uses the cell counts `w = floor(width + 1.0F)` and `h = floor(height + 1.0F)`, in
f32, from the dimensions of the mob at the start of the search. The covered positions are `(x + i,
y + j, z + k)` with `0 ≤ i < w`, `0 ≤ j < h` and `0 ≤ k < w`. Thus the cells start at the node and
extend to the positive axes. The procedure is:

1. For each covered position, take its block path type and change it in this way:
   - `DOOR_WOOD_CLOSED` becomes `WALKABLE_DOOR` when the mob can open doors and can pass doors;
   - `DOOR_OPEN` becomes `BLOCKED` when the mob cannot pass doors;
   - `RAIL` becomes `UNPASSABLE_RAIL` when the block path type at the mob block position is not
     `RAIL`, and the block path type at one block below the mob block position is not `RAIL`. The
     mob block position is the position at the start of the search.
2. Let `S` be the set of the changed path types. If `S` has one element, return that element.
3. If `S` holds `FENCE`, return `FENCE`. If `S` holds `UNPASSABLE_RAIL`, return
   `UNPASSABLE_RAIL`.
4. Find `H` with this loop, where `malus` is the malus of the mob:

   ```text
   H = BLOCKED; mH = malus(BLOCKED)
   for e in S, in the order of the numbers:
       if malus(e) < 0: return e
       if malus(e) >= mH: H = e; mH = malus(e)
   ```

5. Let `c` be the block path type at `(x, y, z)`, without the changes of step 1.
6. When `w > 1`: return `BIG_MOBS_CLOSE_TO_DANGER` when `malus(c) < mH` and
   `malus(BIG_MOBS_CLOSE_TO_DANGER) < mH`. Else return `H`.
7. When `w = 1`: return `OPEN` when `c` is `OPEN`, `H` is not `OPEN` and `mH` is 0. Else return
   `H`.

A zombified piglin has `w = 1` and `h = 2`. On a magma platform, its node over a magma block covers
the types `FIRE` and `OPEN`, and the result is `FIRE`.

## 2. The malus table

**PATH-MAL-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The malus of
a path type for a mob is the value of the entity type for that path type, when the type sets one.
Else it is the default malus of PATH-TYP-001. The malus does not change while a search runs.

The values of the entity types are data. This table holds the types that the gold farm needs:

| Entity type | Path type | Malus |
|-------------|-----------|-------|
| `zombified_piglin` | `LAVA` | 8 |

A zombified piglin keeps the default malus 16 of `FIRE`, although it does not take fire damage.
E-V3 confirms this: a zombified piglin takes a detour of 16 blocks on stone in place of 10 blocks
on magma.

**PATH-MAL-002** (`source_suggested`, semantic, confidence: medium). A mob type that a later
specification adds must give its rows of the table of PATH-MAL-001. A goal can change a malus while
it runs. Such a change applies to the next search, not to a search that runs.

## 3. The ground node evaluator

### 3.1 Inputs

**PATH-NOD-010** (`source_suggested`, semantic, confidence: medium). The node evaluator reads these
inputs of the mob at the issue tick of the search:

- the position, the box, the on-ground flag and the dimensions;
- the step height (PHY-STP-001);
- the malus table (PATH-MAL-001);
- the fall limit (PATH-NOD-008);
- the mob block position, for the rail rule of PATH-TYP-004;
- the four flags `can_open_doors`, `can_pass_doors`, `can_float` and `can_walk_over_fences`.

The defaults of the flags are false, true, false and false. A goal or a mob type can change them. A
zombie with the door break ability has `can_open_doors` true. A zombified piglin has the defaults.

The swim, fly and amphibious evaluators replace PATH-NOD-001 to PATH-NOD-009 and keep the search of
section 4. An amphibious evaluator also changes the floor level and the `WATER` rules of
PATH-NOD-005. A later specification states them.

### 3.2 The start node

**PATH-NOD-001** (`source_suggested`, semantic, confidence: medium). The start node of a mob on the
ground has the position `(bx, sy, bz)`. Here `bx` and `bz` come from the mob block position. `sy`
comes from these steps:

1. If `can_float` is true and the mob is in a fluid of the tag `entity_floatable`, start at the
   block of the mob. Go up while that block has a fluid of that tag. `sy` is the last block with
   the fluid.
2. Else, if the on-ground flag is true, `sy = floor(y + 0.5)`.
3. Else go down from `floor(y + 1.0)`. In each step, let `sy` be the current `y`, then read the
   block below. Stop when that block is not air and is not pathfindable on land. Also stop when
   the current `y` is the lowest `y` of the dimension.

A mob that can stand on a fluid has another start rule. No mob of the gold farm can, so this document does not state it.

Then the start position is checked. A position can be a start when its mob path type is not `OPEN`
and has a malus of 0 or more. When `(bx, sy, bz)` cannot be a start, try the four corners of the box
at `sy`, in this order: `(xmin, zmin)`, `(xmin, zmax)`, `(xmax, zmin)`, `(xmax, zmax)`. The first
corner block that can be a start is the start position. Else the start position stays `(bx, sy,
bz)`.

The start node has the mob path type of its position and the malus of that type.

### 3.3 Floor level, jump size and jump height

**PATH-NOD-002** (`source_suggested`, semantic, confidence: medium). The floor level of a block
position `(x, y, z)` is `(y - 1) + m`. Here `m` is the largest `y` of the collision shape of the block
at `(x, y - 1, z)`, relative to its position, or 0 for an empty shape. The shape is the shape for
no entity (PHY-SHP-002). When `can_float` is true and the block at `(x, y, z)` has a fluid of the
tag `entity_floatable`, the floor level is `y + 0.5`.

**PATH-NOD-003** (`source_suggested`, semantic, confidence: medium). Let `s` be the step height of
the mob, an f32. The jump height is `max(1.125, s)`, in f64. The jump size of a node is
`floor(max(1.0F, s))` when both these conditions are true. Else the jump size is 0.

- the mob path type one block above the node has a malus of 0 or more;
- the mob path type of the node is not `STICKY`.

A zombified piglin has the step height 0.6. Its jump size is 1, and its jump height is 1.125.

### 3.4 Neighbours

**PATH-NOD-004** (`source_suggested`, semantic, confidence: medium). The neighbours of a node `n`
come from these steps:

1. Let `c` be the mob path type of `n`, and `hn` the floor level of `n`.
2. Probe the four sides in this order: north `(0, -1)`, east `(1, 0)`, south `(0, 1)`, west
   `(-1, 0)`. Each probe uses the column `(n.x + dx, n.y, n.z + dz)`, the jump size of `n`, `hn`,
   the side and `c` (PATH-NOD-005).
3. A side candidate is a neighbour when all these conditions are true:
   - it is a node;
   - it is not a closed node;
   - its malus is 0 or more, or the malus of `n` is negative.
4. Then visit the four corners: north and east, east and south, south and west, west and north.
   For a corner of the sides `a` and `b`, the diagonal check of PATH-NOD-007 uses the candidates of
   `a` and `b`. When the check passes, probe the corner column with the side `a`. The corner
   candidate is a neighbour when it is a node, not a closed node, not `WALKABLE_DOOR`, and its
   malus is 0 or more.

The order of the neighbours is the order of steps 2 and 4.

**PATH-NOD-005** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). A probe
of the column `(x, y, z)` with the jump size `j`, the floor level `hn`, the side `d` and the type
`c` does these steps:

1. If the floor level of `(x, y, z)` minus `hn` is more than the jump height, the candidate is no
   node. Stop.
2. Let `t` be the mob path type at `(x, y, z)` and `m` its malus. When `m ≥ 0`, the candidate is the
   node at `(x, y, z)`. Its path type becomes `t`, and its malus becomes the larger of its malus
   and `m`. Else the candidate is no node.
3. If `c` is a partial collision type, the candidate is a node with a malus of 0 or more, and the
   sweep of PATH-NOD-009 to that node finds a collision, the candidate becomes no node.
4. If `t` is `WALKABLE`, stop.
5. Else apply the first branch that matches:
   1. The candidate is no node or has a negative malus, `j > 0`, `t` is not `FENCE` or
      `can_walk_over_fences` is true, and `t` is none of `UNPASSABLE_RAIL`, `DROP_DOWN` and
      `POWDER_SNOW`: the candidate is the result of the step up of PATH-NOD-006.
   2. `t` is `WATER` and `can_float` is false: go down from `y - 1` while the mob path type is
      `WATER`, while the `y` is more than the lowest `y` of the dimension. Each such position
      becomes the candidate, as in step 2. Stop at the first other type.
   3. `t` is `OPEN`: the candidate is the result of the fall of PATH-NOD-008.
   4. `t` is a partial collision type and the candidate is no node: the candidate is the node at
      `(x, y, z)`, with the path type `t`, the default malus of `t`, and the closed mark.

Thus a node over an open trapdoor (`ON_TOP_OF_DROP_DOWN`) and a node in an open trapdoor
(`DROP_DOWN`) are floors with the malus 0. The evaluator does not try a fall from them. E-V2
confirms that the path of a zombified piglin goes into the cell of an open trapdoor.

**PATH-NOD-006** (`source_suggested`, semantic, confidence: medium). The step up from a probe of
`(x, y, z)` on the side `(dx, dz)` does these steps:

1. Probe `(x, y + 1, z)` with the jump size `j - 1`, the same `hn`, side and `c`. If the result is
   no node, the step up gives no node.
2. If the width of the mob is 1.0 or more, the step up gives that result.
3. If the path type of the result is not `OPEN` and not `WALKABLE`, the step up gives that result.
4. Else test the box from `(cx - w2, f1 + 0.001, cz - w2)` to `(cx + w2, height + f2 - 0.002, cz +
   w2)`, in f64. Here `cx = x - dx + 0.5` and `cz = z - dz + 0.5`, with `(dx, dz)` the side of the
   probe. For a side probe this is the centre of the node that the probe starts from. For a corner
   probe it is not, because the side is only the first side of the corner; the formula applies.
   `w2` is half of the width. `f1` is the floor level of the block position of
   `(cx, y + 1, cz)`, and `f2` is the floor level of the result. When the box has a collision
   (PATH-NOD-009), the step up gives no node. Else it gives the result.

**PATH-NOD-007** (`source_suggested`, semantic, confidence: medium). The diagonal check of a node
`n` with the side candidates `a` and `b` passes when all these conditions are true:

- `a` and `b` are nodes, and neither has a `y` above `n.y`;
- neither has the path type `WALKABLE_DOOR`;
- the mob width is not more than 1.0, or neither `a` nor `b` has a malus more than 0;
- for each of `a` and `b`: its `y` is below `n.y`, or its malus is 0 or more, or both `a` and `b`
  are `FENCE` and the mob width is less than 0.5.

A closed side candidate, for example a fence node of step 5.4 of PATH-NOD-005, takes part in this
check.

**PATH-NOD-008** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The fall
from a probe of `(x, y, z)` visits `cy` from `y - 1` down to the lowest `y` of the dimension:

1. If `y - cy` is more than the fall limit, the candidate is the node at `(x, cy, z)` with the path
   type `BLOCKED` and the malus -1. Stop.
2. Let `t` be the mob path type at `(x, cy, z)`. If `t` is `OPEN`, continue with the next `cy`.
3. Else, if the malus of `t` is 0 or more, the candidate is the node at `(x, cy, z)` as in step 2 of
   PATH-NOD-005. Stop.
4. Else the candidate is the node at `(x, cy, z)` with the path type `BLOCKED` and the malus -1.
   Stop.

When the loop ends at the lowest `y`, the candidate is the node at `(x, y, z)` with `BLOCKED` and
-1. A `BLOCKED` result changes the stored node at that position (PATH-SRC-002).

The fall limit of a mob without an attack target is 3. Step G4 implements only this case, because
health (step G2) and attack targets (step G5) do not exist yet. With an attack target, the fall
limit is `floor(k + 3.0F)`, in f32. Here `k = int(health - max_health × 0.33F) - (3 - d) × 4`, and `k = 0` when this is
negative. `d` is 0 for peaceful, 1 for easy, 2 for normal and 3 for hard. `int` truncates to zero.

E-V4 confirms the limit: a zombified piglin with no target walks off a ledge of 3 blocks to its
target. With 4 blocks, it stops at the ledge. P3 confirms the limit with an attack target and the
health 20: 16 on hard, 12 on normal and 8 on easy.

**PATH-NOD-009** (`source_suggested`, semantic, confidence: medium). A collision test of a box in
the evaluator uses the block colliders of PHY-SHP and the world border. It does not use entity
colliders. A shape that depends on the entity (PHY-SHP-003) uses the state of the mob at the issue
tick: its fall distance, its shift key state and the bottom of its own box, not of the tested box.
The implementation can use `query::intersects` with `NoBodies` of `mcrs_minecraft_physics`.

The sweep to a node `p` starts from the box of the mob at the issue tick. Let `v = (p.x - x + sx /
2, p.y - y + sy / 2, p.z - z + sz / 2)`, where `(x, y, z)` is the position and `(sx, sy, sz)` the
size of the box. Let `n = ceil(|v| / a)`, where `a` is the mean of the three sizes. When `n = 0`, the
sweep finds no collision. Else multiply `v` by `f32(1.0F / n)`, widened, and move the box by `v` for
`n` steps. The sweep finds a collision when the box has a collision after any of the steps.

## 4. The path search

**PATH-SRC-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). A search
has these limits:

- The path length limit is `L = max(follow_range, m)` in f32. `follow_range` is the current value
  of the attribute. `m` is 16, or the value that the mob type sets (AIT-VAN-014).
- The node budget is `B = int(floor(L × 16) × f)`, in f32. `f` is the node multiplier of the
  request, 1.0 by default. `int` truncates.
- The search pops at most `B - 1` nodes. It stops before the pop that makes the count `B`.

The follow range of a zombified piglin is not constant. Its base value is 35. A spawn adds a random
bonus to the base, and on a high local difficulty a second bonus can multiply the value by 2 to
2.5 (`natural-spawning.md`). E-V1 gives the values in the farm: from 31.1 to 94.3, so `B` is from
498 to 1508. Only 1 % of the piglins have `B = 560`.

**PATH-SRC-002** (`source_suggested`, semantic, confidence: medium). A search keeps one node for each
block position that it reads. A node has these fields:

- the position;
- `g`, `h` and `f`, f32 values; `walked`, an f32 distance;
- the path type and the malus, as the last probe set them;
- the previous node, or none;
- the closed mark, and the open mark with the place in the open set.

A new node has the path type `BLOCKED`, the malus 0, no previous node and no marks.

**PATH-SRC-003** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The distance between two
nodes is `sqrt(dx² + dy² + dz²)`, in f32, with `dx`, `dy` and `dz` the f32 differences of the
coordinates. The heuristic of a node is the smallest distance from the node to a target. When the
search computes the heuristic of a node, it also updates the best node of each target
(PATH-SRC-004). The search does these steps:

1. Set `g = 0` and `h` = the heuristic of the start node, and `f = h`. Put the start node into the
   open set.
2. While the open set is not empty:
   1. Add 1 to the count. If the count is `B` or more, stop.
   2. Pop the node `n` with the smallest `f` (PATH-SRC-005). Give `n` the closed mark.
   3. For each target with a Manhattan distance from `n` of at most the reach range, mark the
      target as reached. If a target is reached, stop.
   4. If the distance from `n` to the start node is `L` or more, do not expand `n`. Continue.
   5. For each neighbour `q` of `n` (PATH-NOD-004), in their order:
      1. Let `e` be the distance from `n` to `q`. Set `q.walked = n.walked + e`.
      2. Let `t = n.g + e + q.malus`, left to right in f32.
      3. If `q.walked < L`, and `q` is not in the open set or `t < q.g`: set the previous node of
         `q` to `n`, `q.g = t` and `q.h` = the heuristic of `q` × `1.5F`. Set `q.f = q.g + q.h`. If
         `q` is in the open set, change its key to `q.f`. Else put it into the open set.

Step 2.5.1 changes `q.walked` also when step 2.5.3 does not accept `q`. This is the reference
behaviour, and this server keeps it.

**PATH-SRC-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high). Each target keeps a best node
and a best value, which starts at the largest f32. When the search computes the distance `v` from a
node to the target, and `v` is less than the best value, the node becomes the best node of the
target. The heuristic weight `1.5F` does not apply to this value. After the loop:

- If one or more targets are reached, make a path from the best node of each reached target. The
  result is the path with the fewest nodes. On equal counts, the first target in target order
  (PATH-DET-003) wins. The path is a reached path.
- Else make a path from the best node of each target. The result is the path whose end node has
  the smallest Manhattan distance to its target, then the fewest nodes, then the first target. The
  path is a partial path.

A path is the list of nodes from the start node to the end node, along the previous nodes. A new
path has the next node index 0, so its first next node is the start node.

Thus a reached path ends at the node closest to the target, which can be a node in the open set and
not the node that reached the target.

**PATH-SRC-005** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The open set is a binary heap
in an array, with the smallest `f` at index 0. It compares with `<` only. The parent of index `i` is
`(i - 1) / 2`, and the children are `2i + 1` and `2i + 2`.

- Insert: put the node at the end, then move it up while its `f` is less than the `f` of its parent.
- Pop: take index 0. Move the last node to index 0. When the heap is not empty, move it down: let
  `l` and `r` be the `f` of the children, with `r` = positive infinity when there is no right
  child. When `l < r`, the left child is the candidate, else the right child. Swap with the
  candidate while the `f` of the candidate is less than the `f` of the node.
- Change of the key to `k`: keep the old `f`, set `f = k`. When `k` is less than the old `f`, move
  up, else move down.

This rule fixes the order of nodes with equal `f`. A path in an open area has many equal costs, so
the path shape depends on it. The rule makes the paths of this server equal to the vanilla paths.

## 5. Navigation requests

**PATH-REQ-001** (`source_suggested`, semantic, confidence: medium). A request gives the targets,
the reach range, the speed modifier, and optionally `L` and the node multiplier. Its kind is a
point, a set of block targets or an entity. PATH-REQ-004 reads the kind. A request of a mob on the
ground does these steps:

1. For a single block target: if the column of the target is not loaded, give no path. Else adjust
   the target with PATH-REQ-002. A request for an entity uses the block position of the entity as
   a block target.
2. Give no path when the set of targets is empty, the mob is below the lowest `y` of the
   dimension, or the mob cannot update a path. A mob on the ground can update a path when its
   on-ground flag is true, it is in a liquid, or it is a passenger.
3. If the navigation has a path that is not done, and its target is in the set of targets, give
   that path.
4. Else run the search in the search region (PATH-REQ-003). The result holds its target and the
   reach range of the request. The search does not change the navigation; PATH-REQ-004 applies the
   result. PATH-DEV-006 records the reference.

A request for a point `(x, y, z)` uses the block position of the point and the reach range 1, unless
the request names a reach range.

Thus a mob that is not on the ground gets no path. A goal must ask again later. In E-V2 the turtle
egg goal asks again after 40 ticks, so the summoned piglin starts to walk at tick 40.

**PATH-REQ-002** (`source_suggested`, semantic, confidence: medium). A ground navigation adjusts a
block target `p` in this way, unless the mob can path to targets below the surface:

1. If `p` is air, go down from `p.y - 1` while the block is air. If a block that is not air is found
   above the lowest `y`, the target is the block above it. Stop.
2. Else (all air down to the lowest `y`), go up from `p.y + 1` while the block is air, up to the
   highest `y`. Let `p` be the first position that is not air.
3. If `p` does not have the legacy solid flag (section 12.3), the target is `p`.
4. Else go up from `p.y + 1` while the block has the legacy solid flag. The target is the first
   position without it.

**PATH-REQ-003** (`source_suggested`, semantic, confidence: medium). The search region of a request
is the set of block columns of the chunks that intersect the square from `s - r` to `s + r` on the
`x` and `z` axes. Here `s` is the mob block position, and `r = int(L + 8)`. A ground navigation
changes an entity target into a block target (PATH-REQ-001), so it always uses this region. The
region of AIT-VAN-014 for an entity target, `L + 16` from the block above the mob, applies only to
the other navigations. The region has the full
height of the dimension. A read at a position outside the region, or in a column that is not
loaded, gives air. A read above or below the build height gives air.

**PATH-REQ-004** (`source_suggested`, semantic, confidence: medium). A navigation accepts a new path
from a request in this way:

1. If the result is no path, the kind of the request selects the result. Each case then stops:
   - A request for an entity keeps the current path, the speed modifier, the stored target and
     the reach range. The navigation does not change.
   - A request for a point or for a set of block targets clears the path. The navigation has no
     path.

   A result of no path includes each early stop of PATH-REQ-001, steps 1 and 2.
2. If the result comes from a search, apply the cauldron rule to each node in order: when the
   node is at a block of the tag `cauldrons`, compare its `y` with the `y` of the next node. Move
   the node up by 1. When the old `y` is not less than the `y` of the next node, the next node also
   takes the `y` of the moved node. A request that gives the current path (PATH-REQ-001, step 3)
   skips this step.
3. If the result is done (its next node index is the node count, also for 0 nodes), the navigation
   drops its path. Stop.
4. The path changes when the navigation has no unfinished path, or when the node positions of the
   result differ from those of the current path. When the path changes, the result becomes the
   path. Then reset the stuck check (PATH-FOL-003) and the timeout (PATH-FOL-004), and clear the
   stuck state (AIT-VAN-012).
5. Store the target and the reach range of the result, and the speed modifier.

A request that gives the current path (step 3 of PATH-REQ-001) keeps the next node index. A search
result with the node positions of the current unfinished path also keeps it. PATH-DEV-007 records
the reference.

A goal that follows an entity asks again on its own timer, so a failed request does not stop the
mob on its current path. `mob-goals.md`, E-G3, shows a failed request for an entity: a summoned
baby pig is not on the ground in its first tick. Experiment: give a mob a stroll path, then let a
request for an entity fail. The mob walks on to the end of the stroll path.

**PATH-REQ-005** (`design`, semantic, confidence: medium). A request is a decision (AIT-DEC-001). The
search runs on the snapshot of the issue tick, and PATH-REQ-004 applies the result at the
application tick. A navigation has at most one pending request. A new request of a later issue
tick replaces a pending request; the result of the replaced request is dropped. Until the
application, the follower uses the current path. The storage of the target, the reach range and
the resets of PATH-REQ-004 occur at the application.

## 6. The path follower

**PATH-FOL-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The path
follower runs in each tick in step 4 of AIT-VAN-001. It counts its ticks. It does these steps:

1. If a recompute waits (PATH-RCP-002), check its wait condition. When the condition no longer
   holds, issue the recompute as a request (PATH-REQ-005). The follower does no search.
2. If the path is done or the navigation has no path, stop.
3. If the mob can update a path (PATH-REQ-001, step 2), do the steps of PATH-FOL-002 and
   PATH-FOL-003.
4. Else, while the mob is in the air: advance the next node when all these conditions are true:
   - the `y` of the temporary position is more than the `y` of the next node;
   - the on-ground flag is false;
   - the block `x` and the block `z` of the mob equal those of the point of the next node.
5. If the path is not done, give the move control the wanted position (PATH-FOL-005).

The point of a node is `(x + k × 0.5, y, z + k × 0.5)`, where `k = int(width + 1.0F)`. For a width
less than 1, the point is the bottom centre of the node.

An advance of the next node adds 1 to the next node index, and resets the stuck check.

**PATH-FOL-002** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). On the
ground, the follower advances the next node when the mob is close to it, or when the corner rule
passes. Let `w` be the width, an f32. The limit `a` is `w / 2` when `w > 0.75F`, else `0.75F - w /
2`, in f32. The mob is close when all these conditions are true, in f64:

- `|x - (nx + 0.5)| < a` and `|z - (nz + 0.5)| < a`;
- `|y - ny| < 1.0`.

Here `(nx, ny, nz)` is the next node. The corner rule passes when the path type of the next node is
not `FIRE_IN_NEIGHBOR`, `DAMAGING_IN_NEIGHBOR` or `WALKABLE_DOOR`, and all these conditions are
true:

1. The next node is not the last node.
2. Let `c` be the bottom centre of the next node and `e` the bottom centre of the node after it. Let
   `p` be the temporary position. The squared distance from `p` to `c` is less than 4.
3. Let `u = c - p` and `v = e - p`. `|v|² < |u|²`, or `|u|² < 0.5`.
4. The dot product of `v` and `u`, both normalized, is less than 0. A vector shorter than
   `1.0E-5F`, widened, normalizes to zero.

A zombified piglin has `a = 0.45`.

**PATH-FOL-003** (`source_suggested`, semantic, confidence: medium). The stuck check runs after the
advance. When the tick count of the follower minus the tick of the last check is more than 100:

1. Let `s` be the mob speed. Let `q = s` when `s ≥ 1`, else `q = s × s`. Let `D = q × 100 × 0.25F`,
   in f32.
2. If the squared distance from the temporary position to the position of the last check is less
   than `D × D`, the navigation drops its path.
3. The tick of the last check becomes the current tick count, and its position becomes the
   temporary position.

A reset of the stuck check sets the tick of the last check to the current tick count and its
position to the temporary position. An advance resets the check, so the check fires only after 101
ticks with no advance. A zombified piglin with the mob speed 0.23 has `D = 1.3225`. When `s` is 0,
`D` is 0, and the check never drops the path.

**PATH-FOL-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The timeout check runs after
the stuck check, while the path is not done. It uses the game time `T`:

1. If the next node is the node of the last check, add `T` minus the time of the last check to the
   timer.
2. Else the next node becomes the node of the last check, and the timer becomes 0. Let `d` be the
   distance, in f64, from the temporary position to the bottom centre of that node. The limit
   becomes `d / s × 20` when the mob speed `s` is more than 0, else 0.
3. If the limit is more than 0 and the timer is more than 3 times the limit, the navigation drops
   its path and resets the timeout.
4. The time of the last check becomes `T`.

A reset of the timeout clears the node of the last check and sets the timer and the limit to 0.
The acceptance of a changed path (PATH-REQ-004) and a timeout reset it. Thus the timer counts the
ticks with one next node, and a path times out only at a node that the mob does not reach in 3
times its expected time. A slow mob on a long path keeps its path: in the scene of P4 each node
takes about 50 ticks against a limit of about 310 ticks. PATH-DEV-008 records the reference.

**PATH-FOL-005** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The wanted
position is `(px, gy, pz)`, where `(px, py, pz)` is the point of the next node. `gy` is `py` when the
block below the block position of the point is air. Else `gy` is the floor level of that block
position (PATH-NOD-002, without the fluid case). The follower gives the wanted position and the
speed modifier of the navigation to the move control.

**PATH-FOL-006** (`design`, semantic, confidence: medium). The stuck window is two values of the
navigation: the start tick and the start position. Only three events write them: the acceptance of
a changed path (PATH-REQ-004, step 4), an advance, and a stuck check. A request that gives the
current path (PATH-REQ-001, step 3) writes no value of the follower. A recompute is a request, so
it starts a window only when its path changes. The navigation compares node positions only for the
result of a search, never in each tick. The follower can skip the ticks of a resting row (PATH-DET-006). This
does not change a result: a path after a rest is a changed path, so its acceptance starts a window.

## 7. The move control and the jump control

**PATH-MOV-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). A wanted
position from the follower sets the move operation to `move_to`, unless the operation is `jump`.
In each tick, after the goals and the follower (step 6 of AIT-VAN-001), the move control does the
step of its operation:

- `move_to`: see PATH-MOV-002.
- `jump`: set the mob speed to `f32(k × v)`. Here `k` is the speed modifier and `v` is the value of
  the attribute `movement_speed`, in f64. When the on-ground flag is true, or the mob is in a liquid
  that affects it, the operation becomes `wait`.
- `wait`: set the forward input to 0.
- `strafe`: a goal sets it. `mob-goals.md` reserves it for the ranged goals (section 6.5).

A set of the mob speed also sets the forward input to the same value.

**PATH-MOV-002** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The step
`move_to` does these steps:

1. Set the operation to `wait`.
2. Let `(dx, dy, dz)` be the wanted position minus the position, in f64. If `dx² + dy² + dz²` is
   less than `2.5000003E-7F`, widened, set the forward input to 0. Stop.
3. Let `r = f32(atan2(dz, dx) × 180.0F / f32(π)) - 90.0F`, with the `atan2` of
   `mcrs_minecraft_core::mth`. Turn the yaw to `r` by at most 90 degrees: let `e` be
   `wrap(r - yaw)`, limited to `[-90, 90]`. The new yaw is `yaw + e`, plus 360 when it is less than
   0, minus 360 when it is more than 360, in f32. `wrap(a)` is `a mod 360.0F`, minus 360 when it is
   180 or more, plus 360 when it is less than -180.
4. Set the mob speed to `f32(k × v)`, as in PATH-MOV-001.
5. Start a jump when one of these is true:
   - `dy` is more than the step height and `dx² + dz²` is less than `max(1.0F, width)`;
   - the block at the mob block position has a collision shape that is not empty, the `y` of the
     mob is less than the top of that shape, and the block is not in the tag `doors` or the tag
     `fences`. The shape is the shape for the mob (PHY-SHP-003).

   A jump calls the jump control and sets the operation to `jump`.

E-V2 confirms step 5: an open trapdoor is not in the tag `doors`. Thus a piglin that walks into the
cell of an open trapdoor jumps in that cell before it falls.

**PATH-MOV-003** (`source_suggested`, semantic, confidence: medium). In the reference, the AI step
runs in step 4 of PHY-ORD-002: after the base tick, the jump delay decrease and the decay of
PHY-INP-003, and before the jump and the travel step. The steps `move_to`, `jump` and `wait` each
set the forward input. None of them sets the sideways input, so the sideways input keeps its value
after the decay. The vertical input stays 0. The jump control sets the jump input to true in the
tick of a jump, and to false in each other tick. PHY-INP-004 then applies the jump.

**PATH-MOV-004** (`design`, semantic, confidence: medium). The guidance writes the result of the
move control and the jump control into the walker row of `mcrs_minecraft_physics` in this way:

- It runs in the tile task, before `tick_entity` of the row, in each tick in which the mob has a
  navigation.
- It sets `Living::steering` in each such tick to `Steering { input, jump, speed }`. `input` has
  `sideways = f32(old sideways × 0.98F)`, `vertical = 0` and `forward` from the operation. `jump`
  is the value of the jump control. `speed` is the mob speed; a step that does not set the mob speed
  keeps the old `Living::speed`.
- It sets `Living::yaw` to the yaw of PATH-MOV-002 step 3, when that step runs.

`tick_living` multiplies the old input by `0.98F` and then replaces it with `Steering`. The product
of the guidance is the same f32 operation on the same value, so the result is bit-equal to
PHY-ORD-002. No kernel change is necessary.

The position, the on-ground flag and the box that the guidance reads are the values at the start of
the tick. The base tick and the jump delay do not change them, so they are the values of step 4 of
PHY-ORD-002.

A zombified piglin with the speed modifier 1 and `movement_speed` 0.23 sets the speed and the
forward input to 0.23. On a block with the friction `0.6F`, it walks at about 0.1165 blocks in each
tick. E-V2 measures 0.1165.

## 8. The recompute of a path

**PATH-RCP-001** (`source_suggested`, semantic, confidence: medium). A block change can trigger a
recompute for a navigating mob. The trigger needs all these conditions:

- the collision shape of the new state differs from the shape of the old state, for no entity;
- no recompute waits for this navigation;
- the path is not done and has one or more nodes;
- let `M` be the midpoint of the position of the mob and the end node, in f64. The squared distance
  from the centre of the changed block to `M` is less than `R²`, where `R` is the node count minus
  the next node index.

**PATH-RCP-002** (`source_suggested`, semantic, confidence: medium). A recompute does these steps:

1. If the tick of the recompute minus the tick of the last recompute is 20 or less, or the mob
   cannot update a path, the recompute waits. Stop. The follower tries it again in each tick
   (PATH-FOL-001).
2. Else run a search to the stored target with the stored reach range and the speed modifier of
   the navigation (PATH-REQ-001, steps 2 and 4). Accept the result as PATH-REQ-004 states for a
   request of a set of block targets: no path drops the path. The tick of the last recompute becomes
   the current tick. The recompute no longer waits.

The tick of the last recompute starts at 0. Each tick of this section is the game time.

The end of the path drops a waiting recompute: the path is done, the follower drops it, or a goal
stops it. PATH-DEV-009 records the reference.

**PATH-RCP-003** (`design`, semantic, confidence: medium). This server checks the trigger of
PATH-RCP-001 for the block changes of tick `T` at tick `T + 1`. The block changes are the
`BlockPlaced` messages of `T`, with their old and new states; the system `stir_changed_blocks`
already reads them. The check reads the guidance state at the start of `T + 1`. It tests each
navigating mob of the dimension against each change, with no index. The recompute is a request
with the issue tick `T + 1` (PATH-REQ-005). Section 13 records the difference from the reference as
PATH-DEV-004.

The goals and the behaviours give the other recompute triggers (AIT-VAN-015). The follower gives the
stuck and the timeout triggers (PATH-FOL-003, PATH-FOL-004). This document does not add a
trigger.

## 9. Determinism and the place in the architecture

**PATH-DET-001** (`design`, semantic, confidence: high). A path search is a function of these
inputs only:

- the blocks of the snapshot of its issue tick (AIT-LAG-001), read through the search region;
- the path type table of the tags of that tick (PATH-PRF-002);
- the inputs of the mob of PATH-NOD-010, from the snapshot;
- the request: the targets, the reach range, `L` and the node multiplier;
- the difficulty and the attack target, for the fall limit.

The search uses no random value, no wall time and no state of another search. Thus a search gives
the same path on 1 thread and on N threads, in each division of the world into cells, and in each
order of the searches.

**PATH-DET-002** (`design`, semantic, confidence: high). The search, the request and the recompute
are decisions of the AI stage (AIT-DEC-001), with the allowed delay of AIT-DEC-005. The follower,
the move control and the jump control are reflexes (AIT-RFX-001). They run in the hot tick and
read the state of AIT-SPL-002. The block reads of the follower and the move control use the copy of
PHY-CMP-009. A search can stop after a slice of its work and resume later on the same snapshot
(`physics-architecture.md`, section 5.2). The slices do not change the result.

**PATH-DET-003** (`design`, semantic, confidence: high). The target order of PATH-SRC-004 is the
order of the target positions by `x`, then `y`, then `z`, from the lowest. The node lookup of a
search uses the full block position as its key. Section 13 records these choices as PATH-DEV-001 and
PATH-DEV-002.

**PATH-DET-004** (`design`, semantic, confidence: high). Before the AI stage of track A exists, the
engine runs a search in the AI step of the mob, from the snapshot of the issue tick, and applies the
result at the issue tick plus 1. This is the mode `AiDelay::Fixed(1)` for searches only. Thus the
tests of section 12 do not change when the AI stage replaces this runner. In the exact-parity mode,
the search applies at once (AIT-LAG-011).

**PATH-DET-005** (`design`, semantic, confidence: high). The navigation of a mob lives in its tile,
next to the rows: `Tile` in `entity_rows.rs` gains `navigation: BTreeMap<EntityId, Navigation>`.
Each mob with an AI has an entry, also when it has no path; a mob with `NoAI:1b` has none.
`Navigation` holds the path, the stored target, the reach range, `L`, the speed modifier, the move
operation, the wanted position, the timers of PATH-FOL-003 and PATH-FOL-004, the recompute state of
PATH-RCP-002 and the pending request of PATH-REQ-005. `EntityRows::move_rows` moves the entry with
its row. A despawn of the row removes the entry. The `BTreeMap` keeps the order of the entries
independent of the thread count.

**PATH-DET-006** (`design`, semantic, confidence: high). A mob whose navigation has a path that is
not done, a pending request or a waiting recompute does not rest (`physics-architecture.md`,
section 6.6). The guidance clears the rest state of the row in each such tick. Else the pass skips
the follower of a resting row.

## 10. Cost and caches

**PATH-PRF-001** (`design`, semantic, confidence: medium). These are the cost targets of one search
on one core of the reference machine (Apple M4 Max), with warm tables:

| Search | Target (median) |
|--------|-----------------|
| a search that pops 560 nodes | ≤ 250 µs |
| a search that pops 1508 nodes | ≤ 650 µs |
| the farm search from the edge of a layer to its centre | ≤ 100 µs |

`physics-architecture.md`, section 12.1, budgets 100 µs for a mean search. In the gold farm, about
13 piglins stand on the platforms and each asks for a path at most a few times in 200 ticks
(`mob-goals.md`). Thus the searches of the farm cost less than 0.1 ms in each tick. A server with
many more mobs scales with the request rate, not with the mob count.

**PATH-PRF-002** (`design`, semantic, confidence: high). The implementation must keep these tables
and caches:

- A `PathTypeTable` in `mcrs_minecraft_pathfind`, with two values for each block state: the state
  path type (one byte) and the top of the collision shape for no entity, for PATH-NOD-002. The
  table stores the top because `CollisionTable` keeps only the context class of a state with a
  shape that depends on the entity. The build reads the definitions and the block tags, beside
  `build_tables` of `entity_physics_plugin.rs`, which already builds again when the registries
  change. The inline runner of PATH-DET-004 reads the table of the same tick, so step G4 needs no
  version for each snapshot (`physics-architecture.md`, section 15, question 65).
- For each search, a cache of section references with their masks, of 64 entries
  (`physics-architecture.md`, section 6.7). `BlockWorld::section` builds the masks from the
  palette in each call, so the search must not call it for each block read.
- For each search, a map from a block position to its block path type, and a map from a block
  position to its mob path type. A position is computed at most one time in a search.
- For each search, a map from a box to the result of its collision test (PATH-NOD-006,
  PATH-NOD-009).
- For each search, the nodes in an arena with a map from the position to the node, and the open set
  of PATH-SRC-005. The arena and the maps are cleared, not freed, between searches on one thread.

A cache that lives longer than one search must give the same values as the snapshot of the search.
The implementation must not keep a path type cache of the whole dimension (PATH-DEV-003).

**PATH-PRF-003** (`design`, semantic, confidence: medium). The follower and the move control do a
fixed amount of work in each tick: at most four block reads and no search (AIT-SPL-003). The
recompute trigger of PATH-RCP-001 costs one distance test for each pair of a block change and a
navigating mob. An index is not necessary until a measurement shows this cost.

## 11. Invariants

**PATH-INV-001** (`design`, semantic, confidence: high). Two runs of the same search inputs give
the same list of nodes, bit for bit in each f32 field.

**PATH-INV-002** (`design`, semantic, confidence: high). Each node of a path is in the search region.
Each step from one node to the next is a neighbour of PATH-NOD-004.

## 12. Implementation

### 12.1 Order of work

1. Corpus: add the component `mcrs:navigation` of section 12.3 to the block definition dumper of
   `tools/vanilla-oracle`, add it to the corpus schema, and recapture the corpus with
   `mcrs_minecraft_update`. `Components` in `schema.rs` denies unknown fields, so the schema change
   and the recaptured corpus land in one commit. Add the dump P2 in the same step.
2. Kernel: a new crate `mcrs_minecraft_pathfind`, without Bevy. It holds the `PathTypeTable`
   (PATH-PRF-002), the block and mob path types, the evaluator, the search and the heap (sections 1
   to 4). It also holds the follower and the move control as pure functions of the navigation and
   the row (sections 6 and 7), with their unit tests. It reads blocks through `CollisionWorld` and
   tests boxes with `query::intersects` and `NoBodies`. It reuses `mcrs_minecraft_core::mth::atan2`
   and `wrap_degrees`, and `block_at` of `mcrs_minecraft_physics`. Add the crate to the list of
   packages without Bevy in `xtask/src/checks.rs`.
3. Engine: in `mcrs_minecraft_engine_physics`, the navigation store of PATH-DET-005, the call of the
   guidance before `tick_entity` (PATH-MOV-004), the rest rule of PATH-DET-006, the runner of
   PATH-DET-004 and the recompute check of PATH-RCP-003. This step is only the connection of the
   kernel functions.
4. The scripted target and the exit test of section 12.2.

### 12.2 Exit test

The exit criterion of step G4 in `gold-farm.md` is: a piglin at the edge of a platform walks to the
centre and falls into a shaft. Until step G5 gives the goals, a scripted target makes the requests.

The scripted target asks for a path to the block `(-317, 198, -609)` with the reach range 1 and the
speed modifier 1. It asks in the first tick of the mob, and again each 40 ticks while the navigation
has no path or its path is done. This is the request of the turtle egg goal of E-V2.

The test runs on an `EntityRows` and `Pass` scene, in the form of `test_scene.rs`, and does not
need step G1. The piglin row uses these constants: the walker category, width 0.6, height 1.95,
step height 0.6, `movement_speed` 0.23, `jump_strength` 0.42 and the gravity of the walker. The
navigation of the test stores `L = 70.629`, the follow range of the piglin of E-V2.

The blocks come from a fixture that is checked in: `crates/mcrs_minecraft_pathfind/tests/fixtures/
gold_farm_layer.txt`. A tool test with `#[ignore]` writes it with `mcrs_minecraft_anvil` from
`~/mc-worlds/upgraded/hermitcraft10`, region `dimensions/minecraft/the_nether/region/r.-1.-2.mca`.
The fixture holds one line for each block that is not air, as `x y z` and the block state with its
properties, in the box from `(-326, 196, -620)` to `(-308, 201, -598)`. It also holds the two shaft
columns `(-317, -610)` and `(-317, -608)` from `y = 174` to 195. This extract has about 1000 lines of
the build. It is not a save file: `gold-farm.md`, section 5.1, forbids only the commit of world data.

The tick 0 of E-V2 is the first game tick after the summon, or the tick after it, because the
capture read the game time over RCON. The comparison does not depend on that offset. The test
passes when all these conditions are true:

1. A zombified piglin that is summoned at `(-323.5, 199, -609.5)` goes through the block positions of
   the first E-V2 table to tick 122, in the same order. A change of `y` only is a change of the
   block position.
2. Its first move (the change at tick 45 in E-V2) occurs within 2 ticks of tick 45.
3. The interval between two neighbouring changes is within 1 tick of the interval in the E-V2 table.
4. It has a `y` below 198 in the shaft cell `(-317, -610)` within 130 ticks of the summon.
5. A second run from `(-316.5, 199, -602.5)`, with `L = 35.337`, meets conditions 1 to 3 for the
   second table of E-V2, to tick 97.

The fixtures `e_v2_west.txt` and `e_v2_north.txt` of section 16 give the position of each tick.
After the test aligns its tick of the first move with the fixture, it compares each position with the
tolerance of PHY-DEV-001. The search of the first tick also must give the node list of
`vanilla_paths.json` for that start, with the same `f` of each node, bit for bit.

### 12.3 Data tables

| Data | Source | Note |
|------|--------|------|
| path types, numbers and default maluses | PATH-TYP-001 | a constant table |
| the tags of PATH-TYP-002 and PATH-MOV-002 | the data pack; keys in `mcrs_minecraft_block::block_tag_keys` | the keys exist |
| collision shapes, fluids, the properties `open`, `lit` and `facing` | block definition corpus | exists |
| pathfindable on land, door kind, legacy solid flag | new corpus component `mcrs:navigation` | see below |
| malus table of each entity type | PATH-MAL-001 | data of the entity type |
| dimensions, step height, `movement_speed`, `follow_range` | the entity type table and the attributes (step G1) | the follow range of a spawned mob also needs the spawn bonuses of `natural-spawning.md` |
| `m` of PATH-SRC-001 for each entity type | AIT-VAN-014 | 16 for a zombified piglin |
| the state path type of each state with the vanilla tags | dump P2 | test data only |

The component `mcrs:navigation` is a JSON object with three fields. The dumper writes it for each
state, with its real values. Like the other components, it is in the components of the block when
all states have the same value, else in the permutations.

```json
"mcrs:navigation": {"pathfindable_land": true, "door": "none", "legacy_solid": false}
```

- `pathfindable_land`: the value of the reference for the computation type `LAND`, for the state,
  in an empty level at the block position `(0, 0, 0)`. The reference gives a default from the
  collision shape and overrides it in 55 block classes, so a dump is the only exact source. The
  value does not depend on the position.
- `door`: `"none"`, `"by_hand"` or `"not_by_hand"`. Only a door block has a value other than
  `"none"`. In `26.4-snapshot-7`, only `iron_door` is `"not_by_hand"`.
- `legacy_solid`: the legacy solid flag of the state.

The flags for water and for air have no consumer in step G4, so the component does not hold them.
A fence gate uses the existing flag `FENCE_GATE` of `CollisionTable`, which the tag `fence_gates`
gives (PATH-DEV-005).

The dump P2 is `crates/mcrs_minecraft_pathfind/tests/fixtures/state_path_types.json`: a JSON
array, with one path type name of PATH-TYP-001 for each state id, in the order of the state ids.

### 12.4 Tests to write

Each test checks one behaviour that a regression can break. Load the skill `efficient-testing`
before the tests are written.

| Test | Rules | Kind |
|------|-------|------|
| The state path types of each corpus state with the vanilla tags equal the values of a dump of the reference (P2). | PATH-TYP-002 | fixture, all states |
| A table of scenes with the expected block path type and mob path type: air over magma, over an open trapdoor, in an open trapdoor, next to a cactus, next to water, a closed fence gate, a rail. | PATH-TYP-003, PATH-TYP-004 | unit |
| The magma detour of E-V3: the search picks the stone detour; with stone in place of magma, it picks the straight route. | PATH-MAL-001, PATH-SRC-003 | scene |
| The ledge of E-V4: a drop of 3 gives a reached path; a drop of 4 gives a partial path that ends at the ledge. | PATH-NOD-008, PATH-SRC-004 | scene |
| The budget: a search in an open field with an unreachable target pops exactly `B - 1` nodes for `L` = 35 and for `L` = 94.274. | PATH-SRC-001 | unit |
| Each search of `vanilla_paths.json` (section 16) gives the same node list, the same `f`, malus and path type of each node, and the same open set and closed set, bit for bit. | PATH-TYP-003, PATH-TYP-004, PATH-SRC-003, PATH-SRC-004, PATH-SRC-005 | fixture |
| The follower advances at `|dx| < 0.45` and not at 0.45, for the width 0.6. | PATH-FOL-002 | unit |
| The stuck check drops a path after 101 ticks against a wall, not after 100. | PATH-FOL-003 | unit |
| A block change on the path at a distance less than `R` from `M` triggers one recompute, and a second change in the next 20 ticks waits. | PATH-RCP-001, PATH-RCP-002 | unit |
| The same scene with 1 and 8 threads gives the same paths and the same world hash in each tick. Reuse the harness of `tests/pass.rs`. | PATH-DET-001, PATH-DET-005 | determinism |
| The exit test of section 12.2. | all | scene |
| The guidance and `tick_entity` give the same position and velocity, bit for bit, as a test model of PHY-ORD-002 that sets the input in step 4, over 200 ticks of E-V2. | PATH-MOV-003, PATH-MOV-004 | unit |
| A benchmark of the farm search and of a 1508-node search against PATH-PRF-001. | PATH-PRF-001 | criterion |

## 13. Intentional deviations

The motion of a mob is visible, so this server keeps each deterministic accident of the reference
that changes a path, for example PATH-SRC-003 step 2.5.1, PATH-NOD-008 and PATH-FOL-004. It deviates
only where the reference depends on an order that is not part of the world, or on a stale cache.

### PATH-DEV-001: Target order by position

- **Rule:** PATH-DET-003, PATH-SRC-004.
- **Reference:** the order of the targets is the order of a hash set of nodes. The first target of
  that order wins a tie of the node count.
- **Visible to the client:** rarely. A request with two or more targets and two paths of equal
  length can give the other path. The requests of the gold farm have one target.
- **Test:** a differential test accepts each path of the fewest nodes for a request with more than
  one target.

### PATH-DEV-002: Node lookup by the full position

- **Rule:** PATH-DET-003.
- **Reference:** the node lookup uses a 32-bit key with only the low 8 bits of `y` and 15 bits of
  `x` and `z`. Two positions 256 blocks apart in `y` share one node.
- **Visible to the client:** no. The search region of a vanilla mob spans less than 256 blocks in
  each axis that a search reaches, unless a command sets a follow range of more than about 110.
- **Test:** none. A differential test does not use such a follow range.

### PATH-DEV-003: No path type cache of the whole dimension

- **Rule:** PATH-PRF-002, PATH-DET-001.
- **Reference:** each dimension keeps a cache of 4096 state path types by position. Only a block
  change that the server sends to clients clears an entry. A change without the client update flag
  leaves an old path type in the cache.
- **Visible to the client:** rarely. After such a change, a vanilla mob can walk as if the old block
  were there, until a later change at that position.
- **Test:** a differential test does not use block changes without the client update flag near a
  mob with a path.

### PATH-DEV-004: Recompute triggers at the next tick

- **Rule:** PATH-RCP-003.
- **Reference:** the trigger runs inside the block change, in the same tick, and the recompute reads
  the world at that time.
- **Visible to the client:** no, within AIT-DEV-001. The new path applies 1 to `max_lag + 1` ticks
  later than in the reference. The reference already makes a recompute wait for up to 20 ticks.
- **Test:** as AIT-DEV-001: a differential test accepts a shift of the new path of 1 to
  `max_lag + 1` ticks.

### PATH-DEV-005: Fence gates by the tag

- **Rule:** PATH-TYP-002, step 5.
- **Reference:** a closed fence gate is `FENCE` by the class of its block, not by the tag
  `fence_gates`.
- **Visible to the client:** no with the vanilla data pack, where the tag holds each fence gate
  block. A data pack that changes the tag changes the path types of this server only.
- **Test:** a differential test uses the vanilla tag `fence_gates`.

### PATH-DEV-006: A search does not change the navigation

- **Rule:** PATH-REQ-001, step 4; PATH-REQ-004, step 4.
- **Reference:** each search that gives a path stores its target and reach range, resets the
  timeout and clears the stuck state, also when no request accepts the path. A goal that only tests
  whether a path exists thus changes the running path: its timeout starts again, and a later
  recompute goes to the target of the test.
- **Visible to the client:** rarely. A mob can recompute toward a target that it did not walk to,
  or keep a path past its vanilla timeout.
- **Test:** a differential test does not run a goal that tests a path while another path runs.

### PATH-DEV-007: The cauldron rule applies once to a search result

- **Rule:** PATH-REQ-004, steps 2 to 4.
- **Reference:** the rule changes the stored path at each request, also when the path does not
  change. A node in a column of cauldrons moves up by 1 more block at each request. The stored path
  holds the moved nodes, so a new search for the same route through a cauldron differs from it and
  starts the stuck window again. A search with the node positions of a done current path keeps the
  done path, so the mob does not walk it again.
- **Visible to the client:** rarely, near cauldrons, or when a mob asks for the path that it just
  finished.
- **Test:** a differential test does not put cauldrons on a path.

### PATH-DEV-008: The timeout counts the ticks of one node

- **Rule:** PATH-FOL-004.
- **Reference:** the timer is not reset at a new next node, so it counts the ticks of the whole
  path, and the limit of the current node ends the path. The reset node is the block position
  `(0, 0, 0)`; when that is the next node, the timer adds the time since the last check before the
  reset. P4 shows the result: a piglin that walks through cobwebs stops for 1 or 2 ticks about each
  316 ticks of one path.
- **Visible to the client:** yes. A slow mob on a long path stops for 1 or 2 ticks at intervals.
- **Test:** a differential test ignores a stop of 1 or 2 ticks that a timeout explains. P4 is the
  record of the reference.

### PATH-DEV-009: A recompute is a request of the running path

- **Rule:** PATH-RCP-002.
- **Reference:** the recompute assigns the search result as the path without the acceptance: no
  cauldron rule, no reset of the stuck check. A waiting recompute also runs after the path is done
  or dropped, and gives the mob a path again.
- **Visible to the client:** rarely. A vanilla mob can walk again after its goal stopped it, when
  a block changed near its path less than 20 ticks before.
- **Test:** a differential test does not change blocks near a path in the 20 ticks before the path
  ends.

### Known vanilla quirks

This server keeps these behaviours of the reference. A test must not report them as defects.

- A fire-immune zombified piglin avoids magma with the malus 16 (E-V3).
- Each trapdoor state is `DROP_DOWN`, also a closed trapdoor (PATH-TYP-002).
- A mob jumps in the cell of an open trapdoor (PATH-MOV-002, E-V2).
- A search pops one node less than the budget (PATH-SRC-001).
- The walked distance of a node changes also when the search does not accept the node
  (PATH-SRC-003).
- A fall that is too deep marks the node at the limit as `BLOCKED`, also when the search accepted
  that node from another side before (PATH-NOD-008).
- A mob that is not on the ground gets no path (PATH-REQ-001).

## 14. Unresolved

- **U1**: the follow range of a spawned zombified piglin depends on the spawn bonuses. Step G3 must
  state them in `natural-spawning.md`: a bonus of the base by a factor `1 + t`, with `t` from a
  triangle distribution of width 0.11485 around 0, and a second factor `1 + u` when `u = r × 1.5 ×
  m` is more than 1. Here `r` is a random value in `[0, 1)` and `m` is the special multiplier of the
  local difficulty. E-V1 is consistent with this rule.
- **U2**: closed. The fixtures of section 16 hold the position of each tick in f64, without a gap,
  for E-V2 to E-V4 and P4.
- **U3**: the cost targets of PATH-PRF-001 come from no measurement of this server. The benchmark of
  section 12.4 must confirm them.
- **U4**: `physics-architecture.md`, section 15, question 65: the version of the path type table
  for each snapshot. Step G4 does not need it (PATH-PRF-002). Track A decides it.

## 15. Experiments

### 15.1 Experiments with a vanilla server

Use the vanilla dedicated server of the game version, with `difficulty=hard`,
`pause-when-empty-seconds=-1`, `spawn-monsters=false` and RCON on. The scene of E-V1 to E-V4 is a copy
of the upgraded save of `gold-farm.md`: `level.dat`, the files `game_rules.dat`, `world_clocks.dat`,
`world_gen_settings.dat` and `weather.dat`, and the nether region `r.-1.-2.mca` with its `data`
directory. Force-load the chunks of the farm with `/forceload add -333 -627 -301 -566` in the
nether. Read the blocks with `/execute if block`. Track a mob with `/tick rate 4` and read its `Pos`
and the game time in each tick over RCON.

| Experiment | Steps | Rules | Status |
|------------|-------|-------|--------|
| E-V1 | Summon 400 zombified piglins at `(-320, 253, -600)` with no data. Read the value of `follow_range` of each piglin. | PATH-SRC-001, U1 | done |
| E-V2 | Summon one zombified piglin at `(-323.5, 199, -609.5)`, and in a second run at `(-316.5, 199, -602.5)`. Record its block column and position in each tick for 200 ticks. Repeat the first run. | PATH-NOD-005, PATH-MOV-002, PATH-MOV-003, PATH-REQ-001, PATH-FOL-001 | done |
| E-V3 | Build a closed glass box at `y` 139 to 141, `x` -330 to -316, `z` -638 to -630, with air at `y` 142. Make a straight corridor of 11 blocks at `z` -634 with a magma floor in the 9 inner cells, and a detour of 17 blocks with a stone floor through `z` -637. Put a turtle egg at the end. Summon a piglin at the start. Repeat with stone in place of the magma. | PATH-MAL-001 | done |
| E-V4 | Build a corridor at `y` 134 that ends at a pit. Put a turtle egg on the floor of the pit, 3 blocks below the corridor floor, and in a second run 4 blocks below. Summon a piglin at the far end of the corridor. | PATH-NOD-008, PATH-SRC-004 | done |
| P1 | Connect the path client below as the operator `g4cap`, in spectator mode near the scene. Repeat E-V2 to E-V4 and record each path value with its open set and closed set. | PATH-TYP-003, PATH-TYP-004, PATH-SRC-003, PATH-SRC-004, PATH-SRC-005 | done |
| P2 | Add a dump of the state path type of each state, with the vanilla tags, to the data source set of `tools/vanilla-oracle`. | PATH-TYP-002 | planned with section 12.1, step 1, because it is code of the oracle |
| P3 | In a glass box, make a corridor at `y` 150 that ends at a pit `h` blocks deep, with a pig with `NoAI:1b` on the pit floor. Summon a zombified piglin at the far end with `{AngerTime:6000,angry_at:<UUID of the pig>}`, so the pig is its attack target. Use each of hard, normal and easy, with `h` at the limit and 1 more. Record for 30 s whether it drops. | PATH-NOD-008 | done |
| P4 | Make a corridor of 30 blocks at `y` 150 on stone, with cobwebs in the cells `x` -332 to -312. Summon an angry zombified piglin with a pig target 29 blocks away, as in P3. Record the position in each tick for 900 ticks. Repeat with air in place of the cobwebs. | PATH-FOL-004 | done |

The vanilla client cannot send the debug subscription request, so P1 uses a small client of the
protocol outside the repository. A tool of the repository that repeats P1 must do these steps:

1. Log in offline with compression off (`network-compression-threshold=-1`).
2. In the configuration, answer `select_known_packs` with the packs of the server, `keep_alive` and
   `ping`, then `finish_configuration`.
3. In the play state, answer `keep_alive`, `ping` and `chunk_batch_finished`. After `login`, send
   `debug_subscription_request` with the one id of `entity_paths` in the registry
   `debug_subscription` (5 in `26.4-snapshot-7`).
4. Decode each `debug/entity_value` of that id: the entity id, the subscription id, a present flag,
   then the path value. The path value is: a flag "reached", an i32 next node index, a block
   position, the node list, the open set, the closed set and the target set, then an f32 and a
   variable integer of the follower. Each list is a count and nodes. A node is three i32
   coordinates, the f32 walked distance, the f32 malus, the closed flag, the path type number and
   the f32 `f`.

The server sends a path value only to an operator, and only for a search that ran while the
subscription existed.

### 15.2 Tests for this server

The tests of section 12.4 confirm the `design` rules: PATH-DET-001 by the thread test, PATH-INV-001
and PATH-INV-002 by the search tests, and PATH-PRF-001 by the benchmark.

## 16. Captured results

The results come from a vanilla dedicated server of `26.4-snapshot-7` on 2026-11-08, with the
scene of section 15.1.

The raw captures are outside the repository: each path value of P1 with its nodes, its open set and
its closed set, and the position of each tick of E-V2 to E-V4 and P4. A test that needs them adds
only the part that it checks, as a fixture of `mcrs_minecraft_fixtures`.

The captures of the second session repeat the block positions and the ticks of E-V2 to E-V4 below
exactly, with a spectator operator near the scene.

**P1**, the searches. The `f` of each of the 74 path nodes is equal, bit for bit, to the value of
PATH-SRC-003: `g` adds the f32 distance and the malus of each node, and `h` is the f32 distance to
the target times `1.5F`, except at the start node.

| Experiment | Reached | Path nodes | Open set | Closed set | End node |
|------------|---------|------------|----------|------------|----------|
| E-V2, west start | yes | 10 | 22 | 83 | `(-317, 197, -609)` |
| E-V2, north start | yes | 7 | 16 | 49 | `(-317, 197, -609)` |
| E-V3, magma | yes | 17 | 1 | 17 | `(-318, 140, -634)` |
| E-V3, stone | yes | 11 | 1 | 10 | `(-318, 140, -634)` |
| E-V4, drop 3 | yes | 11 | 8 | 10 | `(-320, 131, -632)` |
| E-V4, drop 4 | no | 7 | 0 | 7 | `(-324, 134, -632)` |

The path of the west start goes over 8 nodes of `FIRE` with the malus 16. Its 9th node is the shaft
cell `(-317, 199, -610)` with `DROP_DOWN` and the malus 0, and its end is the egg cell, which is
`WALKABLE`. Thus the target of the search is `(-317, 198, -609)`, and the reach range 1 accepts the
egg cell below it. In E-V3, the nodes next to the magma corridor are `FIRE_IN_NEIGHBOR` with the
malus 8, because a magma block is a diagonal neighbour one block down (PATH-TYP-003, step 3). After
a piglin lands on the honey, its new search gives a partial path of 5 nodes in the chamber.

**P3**, a zombified piglin with a pig as its attack target and the health 20:

| Difficulty | Fall limit | Drop at the limit | Drop 1 block more |
|------------|------------|-------------------|-------------------|
| hard | 16 | drops (16) | stops at the ledge (17) |
| normal | 12 | drops (12) | stops at the ledge (13) |
| easy | 8 | drops (8) | does not drop in 30 s (9) |

The start of the melee goal took more than 12 s in some runs. That timing belongs to
`mob-goals.md`.

**P4**, the timeout. In the cobwebs the piglin moves `0.0196` blocks in each tick from tick 21. The
move is 0 in the ticks 337 and 338, and in the tick 654: the timeout drops the path, and the melee
goal asks for a new path. The intervals are 316 and 317 ticks of one path. A limit for each node
would not end a path here, because each node takes about 50 ticks and the limit of a node is about
310 ticks. On air, the piglin walks the 29 blocks in about 170 ticks with no stop.

**E-V1**, 400 summons in the farm on the difficulty hard:

| Group | Count | Smallest | Largest | Mean |
|-------|-------|----------|---------|------|
| follow range with only the first bonus | 276 | 31.134 | 38.315 | 34.855 |
| follow range with both bonuses | 124 | 62.533 | 94.274 | — |
| node budget `floor(16 × L)` | 400 | 498 | 1508 | — |

Of the 400 piglins, 4 have the budget 560. The share with both bonuses is 31 %. This agrees with
the chance `1/3` of `u > 1` at the special multiplier 1.

**E-V2**, the block position of the piglin from `(-323.5, 199, -609.5)`, follow range 70.629. The tick
counts from the first recorded tick. Two runs gave the same table.

| Tick | Block position | `x` | `y` | `z` |
|------|----------------|-----|-----|-----|
| 0 | `(-324, 199, -610)` | -323.5 | 199.0 | -609.5 |
| 45 | `(-323, 199, -610)` | -322.937 | 199.0 | -609.5 |
| 54 | `(-322, 199, -610)` | -321.892 | 199.0 | -609.5 |
| 62 | `(-321, 199, -610)` | -320.96 | 199.0 | -609.5 |
| 71 | `(-320, 199, -610)` | -319.911 | 199.0 | -609.5 |
| 80 | `(-320, 199, -611)` | -319.052 | 199.0 | -610.055 |
| 81 | `(-319, 199, -611)` | -318.961 | 199.0 | -610.128 |
| 90 | `(-318, 199, -611)` | -317.981 | 199.0 | -610.427 |
| 99 | `(-317, 199, -611)` | -316.936 | 199.0 | -610.483 |
| 107 | `(-317, 199, -610)` | -316.575 | 199.0 | -609.969 |
| 113 | `(-317, 200, -610)` | -316.498 | 200.001 | -609.488 |
| 119 | `(-317, 199, -610)` | -316.471 | 199.797 | -609.488 |
| 122 | `(-317, 198, -610)` | -316.461 | 198.676 | -609.488 |
| 144 | `(-317, 174, -610)` | -316.459 | 174.938 | -609.012 |

The piglin does not move until tick 40, because its first request fails before it is on the ground
(PATH-REQ-001). From tick 45 to 71 it moves 0.1165 blocks in each tick. It enters the shaft cell from
the south side and jumps in the cell of the open trapdoor (tick 113). It lands on the honey at
`y = 174.9375`.

The second start, `(-316.5, 199, -602.5)`, follow range 35.337. A change of `y` only counts as a
change of the block position, in both tables.

| Tick | Block position | `x` | `y` | `z` |
|------|----------------|-----|-----|-----|
| 0 | `(-317, 199, -603)` | -316.5 | 199.0 | -602.5 |
| 46 | `(-317, 199, -604)` | -316.436 | 199.0 | -603.06 |
| 55 | `(-317, 199, -605)` | -316.476 | 199.0 | -604.103 |
| 63 | `(-317, 199, -606)` | -316.485 | 199.0 | -605.035 |
| 72 | `(-317, 199, -607)` | -316.492 | 199.0 | -606.083 |
| 80 | `(-317, 199, -608)` | -316.495 | 199.0 | -607.015 |
| 86 | `(-317, 200, -608)` | -316.496 | 200.001 | -607.512 |
| 92 | `(-317, 199, -608)` | -316.496 | 199.797 | -607.512 |
| 95 | `(-317, 198, -608)` | -316.496 | 198.676 | -607.512 |
| 97 | `(-317, 197, -608)` | -316.496 | 197.58 | -607.512 |

**E-V3**, the route of the piglin from `(-327.5, 140, -633.5)`:

| Floor of the straight corridor | Route | Ticks to the egg |
|--------------------------------|-------|------------------|
| `magma_block` | north to `z = -637`, east 10 blocks, south to `z = -634` | 175 |
| `stone` | straight east along `z = -634` | 122 |

**E-V4**, the piglin from `(-329.5, 134, -631.5)`:

| Drop to the floor of the egg | Result |
|------------------------------|--------|
| 3 blocks | it walks off the ledge at tick 96 and reaches the egg cell at tick 126 |
| 4 blocks | it stops at `x = -323.794`, in the last corridor cell, and stays there to tick 199 |
