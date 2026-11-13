# Block support: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. All captured results of section 8 come from a
vanilla server of this version.

Some blocks need support from a neighbour. This document states, for each such block, where it can stay,
and what occurs when a neighbour changes and the block cannot stay. It states only what a player, a
client and a save file can see. It does not prescribe an architecture.

The machinery that delivers a shape update to a block is in `block-simulation.md`, section 4
(SIM-SHP-001 to SIM-SHP-004). The destroy of a block is SIM-SET-004. This document gives the shape
function of the blocks of section 3. `block-loot.md` gives the items that a removed block drops.

This document has parts. Part 1 (sections 2 to 4) gives the blocks that a player meets first. Section 5
lists the support families of the parts that follow.

## 0. Conventions

### 0.1 Terms

- **Shape update**, **shape function**, **set**, **destroy**: as `block-simulation.md`, sections 0.1 and
  4, defines them.
- **Support shape**: as SIM-FIRE-003 defines it. It is the collision shape of a state, with the
  exceptions of the table of SIM-FIRE-003.
- **Directions**: `W`, `E`, `N`, `S`, `D` and `U` are west, east, north, south, down and up, as
  `block-simulation.md`, section 0.3, defines them. P + D is the position below P.
- **Subject**: a block of section 3, at the position P.
- **Supporting position**: the position whose state decides if the subject can stay. For a block on a
  floor, it is P + D. For a block on a wall, it is the neighbour of P on the side opposite to the
  `facing` property of the subject.
- **Supporting face**: the face of the support shape of the state at the supporting position that
  touches P. For a block on a floor, it is the face on the plane `y = 16` sixteenths. For a block on a
  wall, it is the face of that block on the side of P.
- **Face area**: the union of the boxes of the support shape that touch the plane of the supporting face,
  projected on that plane. A face area is a set of points on a square of 16 × 16 sixteenths.
- **Can stay**: the result of the support test of a subject (section 3) for the states of the world at
  that time.
- **Air**: one of `minecraft:air`, `minecraft:cave_air` and `minecraft:void_air`.
- **Removal for support**: the destroy of a subject that cannot stay (SUP-RUN-001).

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 8.
- `source_suggested`: the author's knowledge of the game, with no captured result. Section 7 attaches an
  experiment.
- `unresolved`: not decided. Section 6 collects these items.

### 0.3 ID prefix

Rules of this document have the prefix `SUP-`. The topics are `FACE` (face tests), `RUN` (when the test
runs and what occurs), and one topic for each support family of section 3.

## 1. Face tests

**SUP-FACE-001** (`black_box_confirmed`, semantic, confidence high). A supporting face passes one of
these tests. The coordinates are in sixteenths on the square of the face.

| Test | The face area must cover |
|------|--------------------------|
| full | the full square, from 0 to 16 on both axes |
| center | the square from 7 to 9 on both axes |
| rigid | the ring between the full square and the open square from 2 to 14 on both axes |

A support shape that does not touch the plane of the face has an empty face area. An empty face area
passes no test. The full test is the "sturdy top" of SIM-FIRE-003 when the face is the top face. Results
R1 and R2 confirm the three tests for each state of the survey. For example, a bottom slab has an empty
top face, and a top slab passes all three tests. A fence or a wall passes the center test and fails the
full test and the rigid test. Leaves pass no test, because their support shape is empty.

**SUP-FACE-002** (`black_box_confirmed`, semantic, confidence high). A block type either holds a sign or
does not hold a sign. All states of one block type give the same result. No face test decides it. A block
type holds a sign when it is not air and not in this list:

```text
acacia_button acacia_sapling activator_rail allium attached_melon_stem attached_pumpkin_stem
azalea azure_bluet bamboo_button beetroots big_dripleaf big_dripleaf_stem birch_button
birch_sapling black_candle black_carpet blue_candle blue_carpet blue_orchid brain_coral
brain_coral_fan brain_coral_wall_fan brown_candle brown_carpet brown_mushroom bubble_column
bubble_coral bubble_coral_fan bubble_coral_wall_fan bush cactus_flower candle carrots cave_vines
cave_vines_plant cherry_button cherry_sapling chorus_flower chorus_plant closed_eyeblossom cocoa
comparator copper_golem_statue copper_torch copper_wall_torch cornflower creeper_head
creeper_wall_head crimson_button crimson_fungus crimson_roots cyan_candle cyan_carpet dandelion
dark_oak_button dark_oak_sapling dead_bush detector_rail dragon_head dragon_wall_head end_gateway
end_portal end_rod exposed_copper_golem_statue fern fire fire_coral fire_coral_fan
fire_coral_wall_fan firefly_bush flower_pot flowering_azalea frogspawn glow_lichen
golden_dandelion gray_candle gray_carpet green_candle green_carpet hanging_roots heavy_core
horn_coral horn_coral_fan horn_coral_wall_fan ice_crystal jungle_button jungle_sapling kelp
kelp_plant ladder large_fern lava leaf_litter lever light light_blue_candle light_blue_carpet
light_gray_candle light_gray_carpet lilac lily_of_the_valley lily_pad lime_candle lime_carpet
magenta_candle magenta_carpet mangrove_button mangrove_propagule melon_stem moss_carpet
nether_portal nether_sprouts nether_wart oak_button oak_sapling open_eyeblossom orange_candle
orange_carpet orange_tulip oxeye_daisy oxidized_copper_golem_statue pale_hanging_moss
pale_moss_carpet pale_oak_button pale_oak_sapling peony piglin_head piglin_wall_head pink_candle
pink_carpet pink_petals pink_tulip pitcher_crop pitcher_plant player_head player_wall_head
polished_blackstone_button poplar_button poplar_sapling poppy potatoes potted_acacia_sapling
potted_allium potted_azalea_bush potted_azure_bluet potted_bamboo potted_birch_sapling
potted_blue_orchid potted_brown_mushroom potted_cactus potted_cherry_sapling
potted_closed_eyeblossom potted_cornflower potted_crimson_fungus potted_crimson_roots
potted_dandelion potted_dark_oak_sapling potted_dead_bush potted_fern potted_flowering_azalea_bush
potted_golden_dandelion potted_jungle_sapling potted_lily_of_the_valley potted_mangrove_propagule
potted_oak_sapling potted_open_eyeblossom potted_orange_tulip potted_oxeye_daisy
potted_pale_oak_sapling potted_pink_tulip potted_poplar_sapling potted_poppy potted_red_mushroom
potted_red_tulip potted_spruce_sapling potted_torchflower potted_warped_fungus potted_warped_roots
potted_white_tulip potted_wither_rose powder_snow powered_rail pumpkin_stem purple_candle
purple_carpet rail red_candle red_carpet red_mushroom red_shrub red_tulip redstone_torch
redstone_wall_torch redstone_wire repeater resin_clump rose_bush scaffolding sea_pickle seagrass
shelf_mushroom short_dry_grass short_grass skeleton_skull skeleton_wall_skull small_dripleaf snow
soul_fire soul_torch soul_wall_torch spore_blossom spruce_button spruce_sapling stone_button
structure_void sugar_cane sunflower sweet_berry_bush tall_dry_grass tall_grass tall_seagrass torch
torchflower torchflower_crop tripwire tripwire_hook tube_coral tube_coral_fan tube_coral_wall_fan
twisting_vines twisting_vines_plant vine wall_torch warped_button warped_fungus warped_roots water
waxed_copper_golem_statue waxed_exposed_copper_golem_statue waxed_oxidized_copper_golem_statue
waxed_weathered_copper_golem_statue weathered_copper_golem_statue weeping_vines
weeping_vines_plant wheat white_candle white_carpet white_tulip wildflowers wither_rose
wither_skeleton_skull wither_skeleton_wall_skull yellow_candle yellow_carpet zombie_head
zombie_wall_head
```

The list holds 256 of the 1,307 block types that the survey tested (R3). Each name has the namespace
`minecraft`. A sign, a banner, a pressure plate, a cake, a lantern and an amethyst bud hold a sign,
although they have no full face. A snow layer does not hold a sign, also with 8 layers. `moving_piston`
was not tested.

## 2. When the test runs, and what occurs

**SUP-RUN-001** (`black_box_confirmed`, semantic, confidence high). The shape function of a subject
(SIM-SHP-001 step 3) must give this result:

1. If the shape update comes from a direction that the family of the subject does not test (section 3),
   the result is the state of the subject. Nothing changes.
2. If not, and the subject can stay, the result is the state of the subject.
3. If not, the result is air. SIM-SHP-001 step 4 then destroys P.

The destroy is SIM-SET-004 with these results for an observer:

- A subject with `waterlogged` true changes to a water source. All other subjects change to air. Result
  R5 shows a sign and a wall sign.
- The subject drops the items of its block table, with the context of the second row of LOOT-SRC-002:
  the empty stack as the tool, and no entity. The item entities appear in the same tick (R5). The packet
  order to a player is BRK-DRP-005.
- The destroy is a set with flags 3. Thus it makes neighbour updates and shape updates around P. A
  subject that stands on the removed subject is tested in the same tick, and so on up the chain (R5).

A set with *keep-shape* makes no shape updates (SIM-SHP-001). A player sees a subject that lost its
support through such a set stay in its position. It goes at the next shape update from a tested
direction (R4, the column "none").

**SUP-RUN-002** (`source_suggested`, semantic, confidence medium). The support test of section 3 is also
the support test of PLC-SUP-001. A placement of a subject where the subject cannot stay is refused. The
results of PLC-PRT-004 for tall grass and the sunflower agree with SUP-VEG-001. Section 6 (U1 and U3)
states what remains unknown.

## 3. Support families of part 1

Each rule gives the support test and the directions that the shape function tests. "Any direction" means
each of the six directions. "Down" means a shape update whose neighbour position is P + D. "The attached
side" means a shape update whose neighbour position is the supporting position of a wall block.

### 3.1 Plants on a base

**SUP-VEG-001** (`black_box_confirmed`, semantic, confidence high). A plant of this family can stay when
the block at P + D is in the block tag of the plant. The test runs for a shape update from any direction
(R4). The block definition corpus states the tag of each plant in the component
`minecraft:placement_filter` (`assets/mcrs/block_definition/README.md`). This table gives the plants of
part 1. Result R1 confirms the tag of `short_grass` for each state of the survey. R4 tests the directions
for `short_grass`, `oak_sapling`, `poppy` and `pink_petals`. The directions of the other plants are
`source_suggested`.

| Tag | Plants |
|-----|--------|
| `#minecraft:supports_vegetation` | `short_grass`, `fern`, `bush`, `firefly_bush`, `red_shrub`, `dandelion`, `golden_dandelion`, `poppy`, `blue_orchid`, `allium`, `azure_bluet`, `red_tulip`, `orange_tulip`, `white_tulip`, `pink_tulip`, `oxeye_daisy`, `cornflower`, `lily_of_the_valley`, `torchflower`, `open_eyeblossom`, `closed_eyeblossom`, `pink_petals`, `wildflowers`, `oak_sapling`, `spruce_sapling`, `birch_sapling`, `jungle_sapling`, `acacia_sapling`, `dark_oak_sapling`, `pale_oak_sapling`, `cherry_sapling`, `poplar_sapling` |
| `#minecraft:supports_wither_rose` | `wither_rose` |

At this version, `#minecraft:supports_vegetation` holds `dirt`, `coarse_dirt`, `rooted_dirt`, `mud`,
`muddy_mangrove_roots`, `moss_block`, `pale_moss_block`, `grass_block`, `podzol`, `mycelium` and
`farmland`. The data pack gives the members. An implementation reads them from the data pack.

**SUP-VEG-002** (`black_box_confirmed`, semantic, confidence high). A tall plant has two parts, with the
property `half`. The parts are at P (`lower`) and P + U (`upper`). These blocks are tall plants of part 1:
`tall_grass`, `large_fern`, `sunflower`, `lilac`, `rose_bush`, `peony` and `pitcher_plant`.

- The lower part can stay when the block at P + D passes SUP-VEG-001 for the tall plant, and P + U holds
  the upper part of the same block.
- The upper part can stay when P + D holds the lower part of the same block.

The removal for support of one part removes the other part in the same tick, through the chain of
SUP-RUN-001 (R5). Each removed part drops the items of its own block table (SUP-RUN-001). The tables
decide which part gives items, and LOOT-SRC-006 states them. For `sunflower`, one item drops when the
base goes and when the upper part goes first. Nothing drops when the lower part is set to air directly
(R6). The tables of `tall_grass` and `large_fern` give `wheat_seeds` by chance, or nothing. R5 and R6 test
`sunflower`, `tall_grass` and `large_fern`. The other tall plants are `source_suggested`. The test runs for
a shape update from any direction (`source_suggested`; R4 tests only plants with one part).

**SUP-VEG-003** (`black_box_confirmed`, semantic, confidence high). `brown_mushroom` and `red_mushroom`
can stay when the supporting face at P + D passes the center test, or when the block at P + D is in
`#minecraft:overrides_mushroom_light_requirement`. The test runs for a shape update from any direction
(R4). The light at P does not change the result of a shape update (R7). Section 6 (U1) asks about the
light for a placement.

**SUP-VEG-004** (`black_box_confirmed`, semantic, confidence high). `leaf_litter` can stay when the
supporting face at P + D passes the full test. The test runs for a shape update from any direction (R4).
The block corpus has no `minecraft:placement_filter` for `leaf_litter`, because no tag decides it.

### 3.2 Torches

**SUP-TORCH-001** (`black_box_confirmed`, semantic, confidence high). A torch on a floor can stay when
the supporting face at P + D passes the center test. The test runs only for a shape update from down
(R4). These blocks are torches on a floor: `torch`, `soul_torch`, `copper_torch` and `redstone_torch`.
This rule states only the support of `redstone_torch`, not its signal. The survey (R1) and R4 test
`torch`. The other three are `source_suggested`.

**SUP-TORCH-002** (`black_box_confirmed`, semantic, confidence high). A torch on a wall can stay when
the supporting face passes the full test. The supporting position is P minus the `facing` vector. The
test runs only for a shape update from the attached side (R4). These blocks are torches on a wall:
`wall_torch`, `soul_wall_torch`, `copper_wall_torch` and `redstone_wall_torch`. A torch on a wall drops
the item of the torch on a floor, for example `torch` for `wall_torch` (R6). The survey (R1) and R4 test
`wall_torch`. The other three are `source_suggested`.

### 3.3 Thin blocks on a floor

**SUP-SNOW-001** (`black_box_confirmed`, semantic, confidence high). `snow` (the snow layer) can stay when
one of these conditions is true, in this order:

1. If the block at P + D is in `#minecraft:cannot_support_snow_layer`, the snow layer cannot stay.
2. If the block at P + D is in `#minecraft:support_override_snow_layer`, the snow layer can stay.
3. If not, the snow layer can stay when the top face of the **collision shape** at P + D covers the full
   square, or when P + D holds `snow` with 8 layers.

Step 3 uses the collision shape, not the support shape. Thus leaves hold a snow layer, although their
support shape is empty. A snow layer with less than 8 layers does not hold a snow layer. The test runs for
a shape update from any direction (R4). With the empty stack as the tool, a snow layer drops nothing,
for each number of layers (R6).

**SUP-CARPET-001** (`black_box_confirmed`, semantic, confidence high). A carpet can stay when the block
at P + D is not air. The test runs for a shape update from any direction (R4). These blocks are carpets:
each block of `#minecraft:wool_carpets`, and `moss_carpet`. A carpet on a carpet is a chain: the removal
for support of the lower carpet removes the upper carpet in the same tick (R5). `pale_moss_carpet` is not
in this family (section 5).

**SUP-PLATE-001** (`black_box_confirmed`, semantic, confidence high). A pressure plate can stay when the
supporting face at P + D passes the rigid test or the center test. The test runs only for a shape update
from down (R4). These blocks are pressure plates: each block of `#minecraft:pressure_plates`. The survey
(R1) and R4 test `stone_pressure_plate`. The other plates are `source_suggested`.

### 3.4 Signs

**SUP-SIGN-001** (`black_box_confirmed`, semantic, confidence high). A sign on a floor can stay when the
block at P + D holds a sign (SUP-FACE-002). The test runs only for a shape update from down (R4). These
blocks are signs on a floor: each block of `#minecraft:standing_signs`.

**SUP-SIGN-002** (`black_box_confirmed`, semantic, confidence high). A sign on a wall can stay when the
block at its supporting position holds a sign (SUP-FACE-002). The supporting position is P minus the
`facing` vector. The test runs only for a shape update from the attached side (R4). These blocks are
signs on a wall: each block of `#minecraft:wall_signs`. The block types that hold a sign on a wall are the
block types that hold a sign on a floor (R3).

## 4. Summary of part 1

| Rule | Family | Support test | Directions | Drop with the empty stack (R6) |
|------|--------|--------------|------------|-------------------------------|
| SUP-VEG-001 | plants on a base | block tag at P + D | any | its item (`oak_sapling`, `poppy`, `pink_petals`); `short_grass` and `fern`: `wheat_seeds` by chance, or nothing |
| SUP-VEG-002 | tall plants | each part needs the other part | any | its block tables: `sunflower` one item; `tall_grass` and `large_fern` `wheat_seeds` by chance, or nothing |
| SUP-VEG-003 | mushrooms | center, or a tag | any | its item |
| SUP-VEG-004 | leaf litter | full | any | its item, one for each segment |
| SUP-TORCH-001 | torches on a floor | center | down | its item |
| SUP-TORCH-002 | torches on a wall | full, on the attached side | attached side | the torch item |
| SUP-SNOW-001 | snow layer | tags, then the collision shape | any | nothing |
| SUP-CARPET-001 | carpets | not air | any | its item |
| SUP-PLATE-001 | pressure plates | rigid or center | down | its item |
| SUP-SIGN-001 | signs on a floor | holds a sign | down | the sign item |
| SUP-SIGN-002 | signs on a wall | holds a sign, on the attached side | attached side | the sign item |

## 5. Families of the parts that follow

These blocks also need support. Each later part states its family with the same layout as section 3.

| Part | Families |
|------|----------|
| 2 | blocks that hang from the block above: lanterns, hanging signs, `spore_blossom`, `hanging_roots`, `pale_hanging_moss`, chains of vines that grow down |
| 3 | blocks on the face of any neighbour: buttons, levers, ladders, `vine`, `glow_lichen`, `sculk_vein`, `cocoa`, `tripwire_hook`, banners on a wall, heads on a wall, `shelf_mushroom`, amethyst buds, `pointed_dripstone` |
| 4 | plants that use scheduled ticks or grow: `cactus`, `sugar_cane`, `kelp`, `bamboo`, crops and stems on `farmland`, `nether_wart`, `sweet_berry_bush`, the dripleaf blocks, `twisting_vines` and `weeping_vines`, `chorus_plant` and `chorus_flower` |
| 5 | blocks that need water or another fluid: `lily_pad`, `frogspawn`, corals, `seagrass`, `sea_pickle` |
| 6 | other blocks: doors, beds, rails, redstone wire, repeaters, comparators, candles, heads and banners on a floor, `pale_moss_carpet`, `scaffolding`, the fungi and roots of the nether, `azalea`, `mangrove_propagule`, `dead_bush` and the dry grass |

## 6. Unresolved

Each item names the experiment that decides it. A closed item keeps its number.

- **U1**: the placement test of a mushroom. SUP-VEG-003 shows that a shape update does not read the
  light. A placement can still refuse a mushroom in light on a block that is not in
  `#minecraft:overrides_mushroom_light_requirement`. Experiment E8.
- **U2**: the directions of the test for the tall plants of SUP-VEG-002, for `soul_torch`, `copper_torch`,
  `redstone_torch` and their wall forms, and for the pressure plates other than `stone_pressure_plate`.
  Experiment E4 with these blocks.
- **U3**: the placement test of the other subjects (SUP-RUN-002). Experiment E9.

## 7. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`, `level-type=minecraft:flat` and `pause-when-empty-seconds=-1`. The experiments send
commands to the console only. No client is necessary. Force-load the columns of each experiment with
`forceload`.

Put each case of E1 to E4 in its own column, 3 blocks from the next case. Run all commands of one family
from one function, so that one level tick runs them. A command of the form
`execute if block <P> <subject> run data modify storage ...` records each result. Use the nether, above
its bedrock roof (`y` 200): the light there is 0 at each position.

| Experiment | Steps | Result |
|------------|-------|--------|
| E1 | For each family of section 3, take one subject and a base that holds it. For each state X of the survey: set the base at the supporting position, set the subject at P, set X at the supporting position, and record if P still holds the subject. | R1, R2 |
| E2 | Compare the result of E1 with SUP-FACE-001, SUP-VEG-001 and SUP-SNOW-001, computed from the support shapes, the collision shapes and the tags. | R2 |
| E3 | Do E1 with `oak_sign` and with `oak_wall_sign[facing=east]`. For each X that removes itself (rails, redstone wire, fire, soul fire), do the step again with a block under X. | R3 |
| E4 | Put the subject on its base. Set the base to air with `setblock ... strict`. Set `cave_air` at one of the six neighbours of P, or at no neighbour. Record if P still holds the subject. | R4 |
| E5 | In the overworld, in one function: a carpet on a carpet on stone, with a snow layer on top, then remove the stone. A sunflower on dirt, then remove the dirt. A torch on stone, then remove the stone, and test for an item entity near P. A sign and a wall sign with `waterlogged=true`, then remove the support. | R5 |
| E6 | For each subject, remove the supporting block with `setblock`. Wait one second. List the item entities with `execute as @e[type=item] run data get entity @s Item`. | R6 |
| E7 | In the overworld under the open sky, put `brown_mushroom` on stone, and change a neighbour. In the nether, put `glowstone` next to P, wait 3 seconds, put `brown_mushroom` on stone at P, and change a neighbour. | R7 |
| E8 | With a client in survival mode, place `brown_mushroom` on stone under the open sky and in the dark. | not run |
| E9 | With a client, place one subject of each family of section 3 on a base that holds it, and on a base that does not hold it. | not run |

## 8. Captured results

**R1.** The survey holds 2,374 states: the default state of each block type except air and
`moving_piston`, and each state of 39 block types with different shapes (slabs, stairs, trapdoors, snow
layers, fence gates, scaffolding, pistons, piston heads, composters, hoppers, leaves, daylight detectors,
lecterns, grindstones, bells, end portal frames, chorus flowers, lanterns, anvils, campfires, farmland,
cauldrons, glass panes, fences, walls, chests, shulker boxes, sea pickles, candles, big dripleaves,
pointed dripstone, copper grates, doors, beds, cakes, turtle eggs, decorated pots, dried ghasts). The
number of states that hold each subject:

| Subject | States that hold it |
|---------|---------------------|
| `short_grass` | 18 |
| `leaf_litter` | 561 |
| `brown_mushroom` (light 0) | 1,154 |
| `torch` | 1,154 |
| `wall_torch[facing=east]` | 522 |
| `snow[layers=1]` | 606 |
| `white_carpet` | 2,369 |
| `moss_carpet` | 2,368 |
| `stone_pressure_plate` | 1,179 |
| `oak_sign` | 2,022 |
| `oak_wall_sign[facing=east]` | 2,022 |

The states that hold `brown_mushroom` at light 0 are the states that hold `torch`. Five states do not
hold `white_carpet`: `activator_rail`, `detector_rail`, `powered_rail`, `redstone_wire` and `soul_fire`,
each on air. These blocks remove themselves when they are set on air, and the carpet then stands on air.
`moss_carpet` also fails on `fire`. Fire next to `white_carpet` stays, because wool is fuel
(SIM-FIRE-003). `moss_carpet` is not fuel, so the fire removes itself.

**R2.** E2 found no difference for `short_grass`, `leaf_litter`, `torch`, `wall_torch`, `snow` and
`stone_pressure_plate`, for each of the 2,374 states.

**R3.** For a sign, all states of one block type give the same result. The block types that do not hold a
sign are the list of SUP-FACE-002. The signs on a floor and on a wall give the same list. With a block
under them, `rail`, `activator_rail`, `detector_rail`, `powered_rail`, `redstone_wire`, `fire`,
`soul_fire`, `repeater` and `comparator` stay in their positions, and the sign still goes.

**R4.** `stay` means that P still holds the subject. `goes` means that P holds air.

| Subject | W | E | N | S | D | U | none |
|---------|---|---|---|---|---|---|------|
| `short_grass` | goes | goes | goes | goes | goes | goes | stay |
| `oak_sapling` | goes | goes | goes | goes | goes | goes | stay |
| `poppy` | goes | goes | goes | goes | goes | goes | stay |
| `pink_petals` | goes | goes | goes | goes | goes | goes | stay |
| `leaf_litter` | goes | goes | goes | goes | goes | goes | stay |
| `brown_mushroom` | goes | goes | goes | goes | goes | goes | stay |
| `snow[layers=1]` | goes | goes | goes | goes | goes | goes | stay |
| `white_carpet` | goes | goes | goes | goes | goes | goes | stay |
| `torch` | stay | stay | stay | stay | goes | stay | stay |
| `stone_pressure_plate` | stay | stay | stay | stay | goes | stay | stay |
| `oak_sign` | stay | stay | stay | stay | goes | stay | stay |
| `wall_torch[facing=east]` | goes | stay | stay | stay | stay | stay | stay |
| `oak_wall_sign[facing=east]` | goes | stay | stay | stay | stay | stay | stay |

The supporting position of the two wall blocks is the neighbour to the west.

**R5.** In one level tick: the two carpets and the snow layer are air. Both parts of the sunflower are air.
An item entity is within 1.5 blocks of the torch position. The waterlogged sign and the waterlogged wall
sign changed to `water[level=0]`, a source. A sign that is not waterlogged changed to air.

**R6.** One item entity for each case, with the count shown:

```text
torch 1 torch; wall_torch 1 torch; oak_sign 1 oak_sign; oak_wall_sign 1 oak_sign
white_carpet 1; stone_pressure_plate 1; brown_mushroom 1; oak_sapling 1; poppy 1
leaf_litter[segment_amount=n] n leaf_litter; pink_petals[flower_amount=n] n pink_petals (n = 1..4)
snow[layers=n] nothing (n = 1..8)
fern nothing (1 run); short_grass nothing in 19 of 20 runs, 1 wheat_seeds in 1 run
sunflower, base removed: 1 sunflower; upper part removed: 1 sunflower; lower part removed: nothing
tall_grass, base removed: 1 wheat_seeds in 3 of 5 runs; upper or lower part removed: nothing
large_fern, base removed: nothing in 5 runs
```

**R7.** `brown_mushroom` on stone stays after a neighbour changes, under the open sky in the overworld and
next to `glowstone` in the nether.

## 9. Intentional deviations

None.
