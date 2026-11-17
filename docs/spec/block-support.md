# Block support: behavioural specification

| Field | Value |
|-------|-------|
| Version | see `assets/minecraft/version.json` |

Some blocks need support from a neighbour. This document states how the data of a block gives the
positions where the block can stay. It also states what occurs when the block cannot stay after a
neighbour changes. It states only what a player, a client, a data file and a save file can see.

The machinery that delivers a shape update to a block is in `block-simulation.md`, section 4
(SIM-SHP-001 to SIM-SHP-004). The destroy of a block is SIM-SET-004. `block-loot.md` gives the items that a
removed block drops.

Sections 1 to 4 give the model. A supporting state has two properties, a
subject has a support predicate, and the corpus carries both. Section 5 gives the support
predicates of the vanilla blocks that a player meets first.

## 0. Conventions

### 0.1 Terms

- **Shape update**, **shape function**, **set**, **destroy**: as `block-simulation.md`, sections 0.1 and
  4, defines them.
- **Support shape**: the shape of a state that the face tests of section 1 read. SUP-FACE-002 gives it.
- **Directions**: `W`, `E`, `N`, `S`, `D` and `U` are west, east, north, south, down and up, as
  `block-simulation.md`, section 0.3, defines them. P + D is the position below P.
- **Subject**: a block with a support predicate, at the position P.
- **Supporting state**: the state at a supporting position.
- **Support predicate**: the condition on the neighbours of P that lets the subject stay. Section 2 gives
  its form.
- **Supporting position**: a position that the support predicate of a subject reads.
- **Face area**: the union of the boxes of a shape that touch the plane of one face of the block,
  projected on that plane. A face area is a set of points on a square of 16 × 16 sixteenths.
- **Solid**: a property of a block state, with the value true or false. SUP-SOLID-001 gives it.
- **Tested direction**: a direction from which a shape update runs the support predicate (SUP-PRED-003).
- **Can stay**: the result of the support predicate of a subject for the states of the world at that time.
- **Air**: a block of the tag `#minecraft:air`.
- **Corpus**: the block definition corpus, `assets/mcrs/block_definition/`.
- **Removal for support**: the destroy of a subject that cannot stay (SUP-RUN-001).

### 0.2 Provenance

- `black_box_confirmed`: shown by an experiment on a vanilla server.
- `source_suggested`: the author's knowledge of the game, with no experiment result. Section 8 attaches an
  experiment.
- `design`: a choice of mcrs, for example a data form. No observer of the game decides it. The rule
  states the reason.
- `unresolved`: not decided. Section 7 collects these items.

### 0.3 ID prefix

Rules of this document have the prefix `SUP-`. The topics are these:

- `FACE` and `SOLID`: the properties of a supporting state.
- `PRED` and `DATA`: the support predicate and its data form.
- `RUN`: when the predicate runs, and what occurs.
- one topic for each family of section 5.

## 1. Properties of a supporting state

A support predicate reads two properties of a supporting state. These are the face tests of its support
shape and its solid value. Both are properties of the state alone. A subject does not change them.

**SUP-FACE-001** (`black_box_confirmed`, semantic, confidence high). A face of a state passes one of these
tests. The coordinates are in sixteenths on the square of the face.

| Test | The face area covers |
|------|--------------------------|
| full | the full square, from 0 to 16 on both axes |
| center | the square from 7 to 9 on both axes |
| ring | the ring between the full square and the open square from 2 to 14 on both axes |

A shape that does not touch the plane of the face has an empty face area. An empty face area passes no
test. The full test of the top face of the support shape is the "sturdy top" of SIM-FIRE-003. For example, a bottom slab has an empty top
face, and a top slab passes all three tests. A fence or a wall passes the center test and fails the full
test and the ring test. Leaves pass no test, because their support shape is empty.

**SUP-FACE-002** (`source_suggested`, semantic, confidence high). The support shape of a state is the
value of the corpus component `minecraft:support` for that state (SUP-DATA-004). When the state has no
such value, the support shape is the collision shape. For the vanilla blocks, the table of SIM-FIRE-003
gives the states whose support shape is not the collision shape.

**SUP-SOLID-001** (`black_box_confirmed`, semantic, confidence high). The solid value of a state is the
value of the corpus component `mcrs:solid` for that state (SUP-DATA-004). When the state has no such value,
the solid value comes from the bounds of its collision shape. The bounds are the smallest box that holds
all boxes of the shape, in blocks:

1. If the collision shape is empty, the state is not solid.
2. If the mean of the three sizes of the bounds is 0.7291666666666666 or more, the state is solid.
3. If the height of the bounds is 1 or more, the state is solid.
4. If not, the state is not solid.

The data pack names the same property in the block predicate `minecraft:solid`. The navigation of mobs
also reads it (`pathfinding.md`). For about one hundred vanilla blocks, the solid value is not
the result of steps 1 to 4. Examples are signs, banners and pressure plates (solid), and snow layers,
ladders and candles (not solid). Steps 1 to 4 are `source_suggested`.

## 2. The support predicate

**SUP-PRED-001** (`design`, semantic, confidence high). A support predicate is a block predicate
in the form of the data pack (`minecraft:block_predicate_type`). Each leaf reads the state at P plus its
`offset`. A data pack and the world generation use the same form, so one evaluator serves the
placement, the shape update and the world generation. The predicate can use only these types:

| Type | The leaf is true when |
|------|-----------------------|
| `matching_blocks` | the block at the offset is in `blocks`, and its state matches `state` |
| `matching_block_tag` | the block at the offset is in `tag` |
| `matching_fluids` | the fluid at the offset is in `fluids`, and its state matches `state` |
| `has_sturdy_face` | the face `direction` of the state at the offset passes the test `face_test` on the shape `shape` |
| `solid` | the state at the offset is solid (SUP-SOLID-001) |
| `replaceable` | the state at the offset is replaceable |
| `not`, `any_of`, `all_of`, `true` | as the data pack defines them |

Three fields extend the form of the data pack. Each field has a default value that gives the meaning of the
data pack. Thus each predicate that SUP-PRED-002 accepts has the meaning of the data pack.

| Type | Field | Values | Default |
|------|-------|--------|---------|
| `has_sturdy_face` | `face_test` | `full`, `center`, `ring` (SUP-FACE-001) | `full` |
| `has_sturdy_face` | `shape` | `support` (SUP-FACE-002), `collision` | `support` |
| `matching_blocks` | `state` | a state properties predicate, as the field `state` of `matching_fluids` | matches each state |

**SUP-PRED-002** (`design`, data, confidence high). A support predicate with a type that is not
in the table of SUP-PRED-001 is an error at load. Examples are `would_survive`, `unobstructed` and
`height_range`. A support predicate reads only the neighbours of P and P itself, so that a shape update can
decide it. An offset other than `[0, 0, 0]` and the six offsets of one step along one axis is also an
error at load.

**SUP-PRED-003** (`black_box_confirmed`, semantic, confidence high). The tested directions of a subject are
the value of the field `tested_directions` of its data (SUP-DATA-001). When the field is absent, the tested
directions are the directions of the offsets of the leaves of the predicate. An offset of P itself gives no
direction. A torch has the default, `D`. A plant has the value `"all"`, because it
also tests the directions that its predicate does not read.

## 3. Data

**SUP-DATA-001** (`design`, data, confidence high). The corpus component
`minecraft:placement_filter` holds the support predicate of a block. It has one of two forms:

- the form of Bedrock, with the field `conditions` (SUP-DATA-002);
- the predicate form, with the field `predicate` that holds a support predicate (SUP-PRED-001).

Both forms can have the field `tested_directions`. Its value is `"all"`, or a list of the direction
names of the data pack (`down`, `up`, `north`, `south`, `west`, `east`). An empty list, or a list with a
direction two times, is an error at load. A
component with both `conditions` and `predicate`, or with neither, is an error at load. A component in
`permutations` gives the predicate of the states that the permutation selects. The predicate of a block
on a wall therefore names its supporting position for each value of `facing`. The predicate form is
necessary because the form of Bedrock cannot read a face test or a solid value (SUP-DATA-002).

```json
"minecraft:placement_filter": {
  "predicate": {"type": "minecraft:has_sturdy_face", "offset": [0, -1, 0], "direction": "up", "face_test": "center"}
}
```

**SUP-DATA-002** (`design`, data, confidence high). The form of Bedrock is a short form of a
support predicate. It changes to a predicate as follows:

1. The list `conditions` becomes `any_of`.
2. Each condition becomes `any_of` with one leaf for each pair of a face in `allowed_faces` and an entry
   in `block_filter`.
3. A face `f` gives the offset opposite to `f`. Thus `up` reads P + D, the block that the subject stands
   on. When `allowed_faces` is absent, it holds the six faces.
4. An entry `"<block>"` becomes `matching_blocks`. An entry `"#<tag>"` becomes `matching_block_tag`.

An empty `conditions`, an empty `allowed_faces` or an empty `block_filter` is an error at load. An entry of
another form, for example an object with `states` or `tags`, is also an error at load. The predicate form
gives the same result for these entries. The short form cannot read a face test, a solid value or a fluid.
A block that needs one of them uses the predicate form.

**SUP-DATA-003** (`design`, data, confidence high). A block without `minecraft:placement_filter`
has no support predicate. It can always stay, unless a behaviour removes it.

**SUP-DATA-004** (`design`, data, confidence high). Two corpus components give the properties
of section 1. Each component is in the components of the block when all states have the same value, and
in `permutations` if not.

- `minecraft:support`: an object with the field `boxes`, a list of boxes in the form of
  `minecraft:selection_box`. The boxes are the support shape. The Bedrock field `shape` (`fence`,
  `stair`) is an error at load.
- `mcrs:solid`: `true` or `false`, the solid value.

The update tool writes `minecraft:support` only for a state whose support shape is not its collision
shape. It writes `mcrs:solid` only for a state whose solid value is not the result of SUP-SOLID-001 steps 1
to 4.

**SUP-DATA-005** (`design`, data, confidence medium). The update tool derives the vanilla values
of this section from the game, and never from a list that a person writes:

1. For each state of a block with a survival test, it puts each state of each block at each candidate
   supporting position. The states that let the subject stay are the accepted set of that state.
2. It tries the candidate predicates of section 5 in the order of that section. It writes the first
   candidate whose result is the accepted set, for each state of the subject.
3. It tests the tested directions as experiment E4 does. It writes `tested_directions` when the result is
   not the default of SUP-PRED-003.
4. A block that no candidate matches gets no component. Its support is a behaviour.
5. For each state, it asks the game for the support shape and the solid value. It writes
   `minecraft:support` and `mcrs:solid` as SUP-DATA-004 states.

The tag probe of `assets/mcrs/block_definition/README.md`, section "Placement filter", picks the tag of a
candidate that reads a tag.

| Data | Source | Status |
|------|--------|--------|
| `minecraft:placement_filter`, short form with one tag | corpus | exists, without `tested_directions`; the plants need `"all"` |
| `minecraft:placement_filter`, predicate form | corpus | new |
| `minecraft:support` | corpus | new; SIM-FIRE-003 states the vanilla values |
| `mcrs:solid` | corpus | new |
| the tags of section 5 | the data pack | exists |

The section "Placement filter" of `assets/mcrs/block_definition/README.md` describes the dump before this
document. The recapture that adds the components of this section also changes that text.

## 4. When the test runs, and what occurs

**SUP-RUN-001** (`black_box_confirmed`, semantic, confidence high). The shape function of a subject
(SIM-SHP-001 step 3) must give this result:

1. If the shape update does not come from a tested direction (SUP-PRED-003), the result is the state of
   the subject. Nothing changes.
2. If not, and the subject can stay, the result is the state of the subject.
3. If not, the result is air. SIM-SHP-001 step 4 then destroys P.

The destroy is SIM-SET-004 with these results for an observer:

- A subject with `waterlogged` true changes to a water source. All other subjects change to air.
- The subject drops the items of its block table, with the context of the second row of LOOT-SRC-002:
  the empty stack as the tool, and no entity. The item entities appear in the same tick. The packet
  order to a player is BRK-DRP-005.
- The destroy is a set with flags 3. Thus it makes neighbour updates and shape updates around P. A
  subject that stands on the removed subject is tested in the same tick, and so on up the chain.

A set with *keep-shape* makes no shape updates (SIM-SHP-001). A player sees a subject that lost its
support through such a set stay in its position. Its removal for support occurs at the next shape update from a tested
direction.

**SUP-RUN-002** (`source_suggested`, semantic, confidence high). The support predicate is also the support
test of PLC-SUP-001. A placement of a subject where the subject cannot stay is refused. The placement
reads the same predicate and nothing else, so the light does not change the placement of a mushroom. The
results of PLC-PRT-004 for tall grass and the sunflower agree with SUP-VEG-002.

## 5. Support predicates of part 1

Each rule gives the predicate of a family as the update tool writes it (SUP-DATA-005). The offsets are
`[x, y, z]`. "Down" is `[0, -1, 0]`. For a block on a wall, `-facing` is the offset opposite to the
`facing` property of the state, and `facing` is the face that the predicate tests.

| Rule | Family | Support predicate | Tested directions |
|------|--------|-------------------|-------------------|
| SUP-VEG-001 | plants on a base | `matching_block_tag` of the tag of the plant, down | all |
| SUP-VEG-003 | mushrooms | `has_sturdy_face` `up`, `center`, down | all |
| SUP-VEG-004 | leaf litter | `has_sturdy_face` `up`, down | all |
| SUP-TORCH-001 | torches on a floor | `has_sturdy_face` `up`, `center`, down | default: `D` |
| SUP-TORCH-002 | torches on a wall | `has_sturdy_face` `facing`, at `-facing` | default: the attached side |
| SUP-SNOW-001 | snow layer | see the rule | all |
| SUP-CARPET-001 | carpets | `not` `matching_block_tag` `#minecraft:air`, down | all |
| SUP-PLATE-001 | pressure plates | `any_of` `has_sturdy_face` `up` `ring` and `center`, down | default: `D` |
| SUP-SIGN-001 | signs on a floor | `solid`, down | default: `D` |
| SUP-SIGN-002 | signs on a wall | `solid`, at `-facing` | default: the attached side |

SUP-VEG-002 (tall plants) is a behaviour, not a predicate.

### 5.1 Plants on a base

**SUP-VEG-001** (`black_box_confirmed`, semantic, confidence high). A plant of this family can stay when
the block at P + D is in the block tag of the plant. The corpus has this predicate already, in the short
form. This table gives the plants of part 1.

| Tag | Plants |
|-----|--------|
| `#minecraft:supports_vegetation` | `short_grass`, `fern`, `bush`, `firefly_bush`, `red_shrub`, `dandelion`, `golden_dandelion`, `poppy`, `blue_orchid`, `allium`, `azure_bluet`, `red_tulip`, `orange_tulip`, `white_tulip`, `pink_tulip`, `oxeye_daisy`, `cornflower`, `lily_of_the_valley`, `torchflower`, `open_eyeblossom`, `closed_eyeblossom`, `pink_petals`, `wildflowers`, `oak_sapling`, `spruce_sapling`, `birch_sapling`, `jungle_sapling`, `acacia_sapling`, `dark_oak_sapling`, `pale_oak_sapling`, `cherry_sapling`, `poplar_sapling` |
| `#minecraft:supports_wither_rose` | `wither_rose` |

The data pack gives the members of each tag. A data pack that changes a tag changes where the plant can
stay.

**SUP-VEG-002** (`black_box_confirmed`, semantic, confidence high). A tall plant has two parts, with the
property `half`. The parts are at P (`lower`) and P + U (`upper`). These blocks are tall plants of part 1:
`tall_grass`, `large_fern`, `sunflower`, `lilac`, `rose_bush`, `peony` and `pitcher_plant`. The shape
function of a tall plant runs only for a shape update from `U` or `D`:

| Part | Shape update from | The removal for support of the part occurs when |
|------|-------------------|--------------------|
| `lower` | `D` | the `minecraft:placement_filter` of the block is false at P |
| `lower` | `U` | P + U does not hold the `upper` part of the same block |
| `upper` | `D` | P + D does not hold the `lower` part of the same block |

The tested directions are `source_suggested`. A tall plant is not a subject with a predicate, because the
removal of one part is the cause of the removal of the other part. The removal of one part removes the
other part in the same tick, through the chain of SUP-RUN-001 Each removed part drops the items of
its own block table. LOOT-SRC-006 states the tables. For `sunflower`, one item drops when the base is removed and
when the upper part is removed first. Nothing drops when the lower part is set to air directly. The tables
of `tall_grass` and `large_fern` give `wheat_seeds` by chance, or nothing.

**SUP-VEG-003** (`black_box_confirmed`, semantic, confidence high). `brown_mushroom` and `red_mushroom`
can stay when the top face of the support shape at P + D passes the center test. The light at P does not
change the result (SUP-RUN-002). The tag `#minecraft:overrides_mushroom_light_requirement` does not
change the result. Its members pass the center test, and it is a rule of the spread of mushrooms.

**SUP-VEG-004** (`black_box_confirmed`, semantic, confidence high). `leaf_litter` can stay when the top
face of the support shape at P + D passes the full test.

### 5.2 Torches

**SUP-TORCH-001** (`black_box_confirmed`, semantic, confidence high). A torch on a floor can stay when the
top face of the support shape at P + D passes the center test. These blocks are torches on a floor:
`torch`, `soul_torch`, `copper_torch` and `redstone_torch`. This rule states only the support of
`redstone_torch`, not its signal. The other three use the same
predicate (`source_suggested`).

**SUP-TORCH-002** (`black_box_confirmed`, semantic, confidence high). A torch on a wall can stay when the
face `facing` of the support shape at P minus the `facing` vector passes the full test. These blocks are
torches on a wall: `wall_torch`, `soul_wall_torch`, `copper_wall_torch` and `redstone_wall_torch`. A torch
on a wall drops the item of the torch on a floor, for example `torch` for `wall_torch`. The other three use the same predicate (`source_suggested`).

### 5.3 Thin blocks on a floor

**SUP-SNOW-001** (`black_box_confirmed`, semantic, confidence high). `snow` (the snow layer) has this
predicate. All offsets are down.

```json
{"type": "minecraft:all_of", "predicates": [
  {"type": "minecraft:not", "predicate": {"type": "minecraft:matching_block_tag", "offset": [0, -1, 0], "tag": "minecraft:cannot_support_snow_layer"}},
  {"type": "minecraft:any_of", "predicates": [
    {"type": "minecraft:matching_block_tag", "offset": [0, -1, 0], "tag": "minecraft:support_override_snow_layer"},
    {"type": "minecraft:has_sturdy_face", "offset": [0, -1, 0], "direction": "up", "shape": "collision"},
    {"type": "minecraft:matching_blocks", "offset": [0, -1, 0], "blocks": "minecraft:snow", "state": {"layers": "8"}}
  ]}
]}
```

The face test reads the collision shape, not the support shape. Thus leaves hold a snow layer, although
their support shape is empty. A snow layer with less than 8 layers does not hold a snow layer. With the
empty stack as the tool, a snow layer drops nothing, for each number of layers.

**SUP-CARPET-001** (`black_box_confirmed`, semantic, confidence high). A carpet can stay when the block at
P + D is not air. These blocks are carpets: each block of `#minecraft:wool_carpets`, and `moss_carpet`. A
carpet on a carpet is a chain: the removal for support of the lower carpet removes the upper carpet in the
same tick.

**SUP-PLATE-001** (`black_box_confirmed`, semantic, confidence high). A pressure plate can stay when the
top face of the support shape at P + D passes the ring test or the center test. These blocks are pressure
plates: each block of `#minecraft:pressure_plates`. The
other plates use the same predicate (`source_suggested`).

### 5.4 Signs

**SUP-SIGN-001** (`black_box_confirmed`, semantic, confidence high). A sign on a floor can stay when the
state at P + D is solid. These blocks are signs on a floor: each block of `#minecraft:standing_signs`.

**SUP-SIGN-002** (`black_box_confirmed`, semantic, confidence high). A sign on a wall can stay when the
state at P minus the `facing` vector is solid. These blocks are signs on a wall: each block of
`#minecraft:wall_signs`. The states that hold a sign on a wall are the states that hold a sign on a floor.

### 5.5 Summary

| Rule | Drop with the empty stack |
|------|---------------------------|
| SUP-VEG-001 | its item (`oak_sapling`, `poppy`, `pink_petals`); `short_grass` and `fern`: `wheat_seeds` by chance, or nothing |
| SUP-VEG-002 | its block tables: `sunflower` one item; `tall_grass` and `large_fern` `wheat_seeds` by chance, or nothing |
| SUP-VEG-003 | its item |
| SUP-VEG-004 | its item, one for each segment |
| SUP-TORCH-001 | its item |
| SUP-TORCH-002 | the torch item |
| SUP-SNOW-001 | nothing |
| SUP-CARPET-001 | its item |
| SUP-PLATE-001 | its item |
| SUP-SIGN-001 | the sign item |
| SUP-SIGN-002 | the sign item |

## 7. Unresolved

Each item names the experiment that decides it.

- **U3**: the placement test of the subjects other than the mushrooms (SUP-RUN-002). Experiment E9.

## 8. Experiments

Use a vanilla server of the game version of `assets/minecraft/version.json`, with
`online-mode=false`, `level-type=minecraft:flat` and `pause-when-empty-seconds=-1`. The experiments send
commands to the console only. No client is necessary. Force-load the columns of each experiment with
`forceload`.

Put each case of E1 to E4 in its own column, 3 blocks from the next case. Run all commands of one family
from one function, so that one dimension tick runs them. A command of the form
`execute if block <P> <subject> run data modify storage ...` records each result. Use the nether, above
its bedrock roof (`y` 200): the light there is 0 at each position.

| Experiment | Steps |
|------------|-------|
| E1 | For each family of section 5, take one subject and a base that holds it. For each state X of the survey: set the base at the supporting position, set the subject at P, set X at the supporting position, and record if P still holds the subject. |
| E3 | Do E1 with `oak_sign` and with `oak_wall_sign[facing=east]`. For each X that removes itself (rails, redstone wire, fire, soul fire), do the step again with a block under X. |
| E4 | Put the subject on its base. Set the base to air with `setblock ... strict`. Set `cave_air` at one of the six neighbours of P, or at no neighbour. Record if P still holds the subject. |
| E5 | In the overworld, in one function: a carpet on a carpet on stone, with a snow layer on top, then remove the stone. A sunflower on dirt, then remove the dirt. A torch on stone, then remove the stone, and test for an item entity near P. A sign and a wall sign with `waterlogged=true`, then remove the support. |
| E6 | For each subject, remove the supporting block with `setblock`. Wait one second. List the item entities with `execute as @e[type=item] run data get entity @s Item`. |
| E7 | In the overworld under the open sky, put `brown_mushroom` on stone, and change a neighbour. In the nether, put `glowstone` next to P, wait 3 seconds, put `brown_mushroom` on stone at P, and change a neighbour. |
| E9 | With a client, place one subject of each family of section 5 on a base that holds it, and on a base that does not hold it. |

## 10. Intentional deviations

None.
