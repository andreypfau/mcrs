# mcrs block definition corpus

Generated from the game's own registries by the `dumpDefinitions` task of
`tools/vanilla-oracle`, at the game version that `assets/minecraft/version.json`
states. One file per registry block, named after the registry path.

The shape is Bedrock's block definition with three additions under
`description`: `properties` (in the order of the game's block-state report, values
in the order of that report), `base_state_id` and `default_state_id`. State ids
are contiguous: state `n` of a block is `base_state_id + n`. `protocol_id` is the
block's index in the `minecraft:block` registry, which is what the network
protocol names a block by.

## Deviations from the Bedrock schema

- `minecraft:selection_box` is an array of boxes, not a single object. A Java
  block shape is a union of boxes and splitting the encoding from
  `minecraft:collision_box` would be worse.
- `description.properties` values are typed: integer properties are JSON numbers
  and boolean properties are JSON booleans. Molang conditions compare against the
  matching literal type.
- `minecraft:destructible_by_mining.seconds_to_destroy` is a decimal. It is
  documented as an integer, but it states hardness rather than seconds, and
  hardness is fractional for most of the game.
- `minecraft:movable.movement_type` gains `ignore_entity`, the fifth Java piston
  reaction. No vanilla block uses it, but the enum would otherwise be lossy.
- `minecraft:instrument_sound` carries the Java instrument name (`harp`) rather
  than a Bedrock sound alias (`note.harp`). The name selects the sound and also
  decides whether the instrument is tunable by the note property.
- No `minecraft:geometry`. Block models come from the Java resource pack under
  `assets/minecraft/blockstates/`.

## Bedrock components and what they state

| Component | What it states |
| --- | --- |
| `minecraft:light_emission` | the light level that the state emits |
| `minecraft:light_dampening` | the light levels that the state removes from light that passes through it |
| `minecraft:friction` | the friction of the block |
| `minecraft:map_color` | the map colour of the state |
| `minecraft:collision_box` | the collision shape of the state |
| `minecraft:selection_box` | the outline shape of the state |
| `minecraft:destructible_by_mining` | the hardness: `false` for an unbreakable state, `true` for an instant one, `seconds_to_destroy` otherwise |
| `minecraft:destructible_by_explosion` | the explosion resistance of the block |
| `minecraft:redstone_conductivity` | whether the state conducts redstone power |
| `minecraft:redstone_producer` | whether the state is a power source, with `power` its own signal; absent when the state is not a source |
| `minecraft:movable` | how a piston moves the state; `sticky` is slime and honey, the blocks that pull their neighbours |
| `minecraft:instrument_sound` | the instrument of a note block next to the state, under `up` when the instrument sounds from a note block above it and `down` otherwise |
| `minecraft:replaceable` | present when placing a block can replace the state |
| `minecraft:flammable` | `lava_flammable` when lava can set the state on fire; absent otherwise |
| `minecraft:block_entity` | present when the state has a block entity, with `container.slot_count` when the block entity is a container |
| `minecraft:loot` | the loot table of the block, absent when the block has none |
| `minecraft:placement_filter` | where the neighbours must be for the block to stay, see below |
| `minecraft:support` | the support shape, when it is not the collision shape, see below |

A producer whose power lives in its block entity (comparator, sculk sensor)
reports `power: 0`, which is what it emits with no block entity present.

`item_specific_speeds` states which items harvest a state that needs the right
tool, asked of the game rather than assumed: a tool family goes in as its tag
when every item in that tag harvests the state, and anything left over goes in
by identifier. Stone gets `q.any_tag('minecraft:pickaxes')`; obsidian, which no
wooden pickaxe harvests, gets the diamond and netherite pickaxes by name. The
list is absent exactly when the state drops its loot without the right tool, and
`destroy_speed` repeats the hardness because Java applies its harvest bonus as a
divisor rather than a second hardness.

## Placement filter

`minecraft:placement_filter` holds the support predicate of a block: where its
neighbours must be for the block to stay (`docs/spec/block-support.md`). It is
derived by asking the game, never written by hand.

For each state of each block with its own survival test, the dump runs the test
with a stand-in level. The level answers the state of the block itself and of
its six neighbours, air at five of them, and says that no neighbour has a block
entity. The dump puts each state of each block at the sixth neighbour, for each
of the six. The states that let the block stay are the accepted sets of that
state. A state whose test asks the level anything else (the light, a fluid
further away, a block two steps away) gets no component. Nothing is answered
with a made-up value.

The dump then tries the candidate predicates of section 5 of the specification,
in its order, and writes the first one whose result at each neighbour is the
accepted set: a block tag below; the `center` face test, then the `full` face
test, on each side; the snow layer predicate; not a block tag below; the `ring`
or `center` face test on each side; `solid` on each side. A state that can never
stay, a state that always stays, and a state that no candidate matches get no
component. The face tests and `solid` are asked of the game for each state.

A candidate that reads a tag can match several tags, because several tags can
hold the same members and a tag can differ only in blocks the rest of the
predicate decides. The dump probes each tag it could write: it adds one marker
block to that tag alone, picked so that the candidate would answer differently
for it, rebinds the block tags, asks the game again, and keeps the tag only when
the game follows it. The tags are rebound unchanged before anything else reads
them. A carpet follows no tag at all: it tests that the block below is not air.
The dump then writes `#minecraft:air`, the tag that the specification names for
air, when its members are exactly the blocks the game reports as air. Any other
choice that stays open gives no component.

`tested_directions` is derived as well: for each matched state, with every
neighbour air, the dump sends a shape update from each of the six directions and
records which ones remove the block. The field is written only when that set
differs from the directions of the offsets that the predicate reads. A block
that no shape update removes gets no component.

A block tag below keeps the short form of Bedrock, one condition whose
`allowed_faces` is `up` and whose `block_filter` is the tag. Every other
predicate has the predicate form, a block predicate of the data pack under
`predicate`. A component in `permutations` gives the predicate of the states it
selects, so a block on a wall names its supporting side for each `facing`, and
the upper half of a tall plant or a door carries none.

The blocks that carry the component. The supporting position is the side of the
block that the predicate reads, one entry for each state that differs:

| Block | Predicate | Supporting position | Tested directions |
| --- | --- | --- | --- |
| `minecraft:acacia_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:acacia_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:acacia_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:acacia_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:acacia_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:acacia_sign` | `solid` | down | default |
| `minecraft:acacia_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:allium` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:amethyst_cluster` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:azalea` | `matching_block_tag` `#minecraft:supports_azalea` | down | all |
| `minecraft:azure_bluet` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:bamboo_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:bamboo_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:bamboo_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:bamboo_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:bamboo_sapling` | `matching_block_tag` `#minecraft:supports_bamboo` | down | all |
| `minecraft:bamboo_sign` | `solid` | down | default |
| `minecraft:bamboo_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:bell` | `has_sturdy_face` `center` | up | default |
| `minecraft:bell` | `has_sturdy_face` `full` | down; east; north; south; west | default |
| `minecraft:birch_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:birch_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:birch_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:birch_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:birch_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:birch_sign` | `solid` | down | default |
| `minecraft:birch_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:black_banner` | `solid` | down | default |
| `minecraft:black_candle_cake` | `solid` | down | default |
| `minecraft:black_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:black_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:blue_banner` | `solid` | down | default |
| `minecraft:blue_candle_cake` | `solid` | down | default |
| `minecraft:blue_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:blue_orchid` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:blue_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:brain_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:brain_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:brain_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:brown_banner` | `solid` | down | default |
| `minecraft:brown_candle_cake` | `solid` | down | default |
| `minecraft:brown_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:brown_mushroom` | `has_sturdy_face` `center` | down | all |
| `minecraft:brown_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:bubble_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:bubble_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:bubble_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:bush` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:cake` | `solid` | down | default |
| `minecraft:candle_cake` | `solid` | down | default |
| `minecraft:cherry_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:cherry_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:cherry_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:cherry_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:cherry_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:cherry_sign` | `solid` | down | default |
| `minecraft:cherry_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:closed_eyeblossom` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:copper_torch` | `has_sturdy_face` `center` | down | default |
| `minecraft:copper_wall_torch` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:cornflower` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:crimson_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:crimson_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:crimson_fungus` | `matching_block_tag` `#minecraft:supports_crimson_fungus` | down | all |
| `minecraft:crimson_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:crimson_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:crimson_roots` | `matching_block_tag` `#minecraft:supports_crimson_roots` | down | all |
| `minecraft:crimson_sign` | `solid` | down | default |
| `minecraft:crimson_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:cyan_banner` | `solid` | down | default |
| `minecraft:cyan_candle_cake` | `solid` | down | default |
| `minecraft:cyan_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:cyan_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:dandelion` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:dark_oak_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:dark_oak_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:dark_oak_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:dark_oak_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:dark_oak_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:dark_oak_sign` | `solid` | down | default |
| `minecraft:dark_oak_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:dead_brain_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_brain_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_brain_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:dead_bubble_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_bubble_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_bubble_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:dead_bush` | `matching_block_tag` `#minecraft:supports_dry_vegetation` | down | all |
| `minecraft:dead_fire_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_fire_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_fire_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:dead_horn_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_horn_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_horn_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:dead_tube_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_tube_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:dead_tube_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:exposed_copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:exposed_copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:fern` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:fire_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:fire_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:fire_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:firefly_bush` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:flowering_azalea` | `matching_block_tag` `#minecraft:supports_azalea` | down | all |
| `minecraft:golden_dandelion` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:gray_banner` | `solid` | down | default |
| `minecraft:gray_candle_cake` | `solid` | down | default |
| `minecraft:gray_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:gray_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:green_banner` | `solid` | down | default |
| `minecraft:green_candle_cake` | `solid` | down | default |
| `minecraft:green_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:green_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:hanging_roots` | `has_sturdy_face` `full` | up | default |
| `minecraft:heavy_weighted_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:horn_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:horn_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:horn_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:ice_crystal` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:iron_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:jungle_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:jungle_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:jungle_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:jungle_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:jungle_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:jungle_sign` | `solid` | down | default |
| `minecraft:jungle_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:ladder` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:large_amethyst_bud` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:large_fern` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:leaf_litter` | `has_sturdy_face` `full` | down | all |
| `minecraft:lever` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:light_blue_banner` | `solid` | down | default |
| `minecraft:light_blue_candle_cake` | `solid` | down | default |
| `minecraft:light_blue_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:light_blue_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:light_gray_banner` | `solid` | down | default |
| `minecraft:light_gray_candle_cake` | `solid` | down | default |
| `minecraft:light_gray_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:light_gray_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:light_weighted_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:lilac` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:lily_of_the_valley` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:lime_banner` | `solid` | down | default |
| `minecraft:lime_candle_cake` | `solid` | down | default |
| `minecraft:lime_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:lime_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:magenta_banner` | `solid` | down | default |
| `minecraft:magenta_candle_cake` | `solid` | down | default |
| `minecraft:magenta_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:magenta_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:mangrove_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:mangrove_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:mangrove_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:mangrove_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:mangrove_propagule` | `matching_block_tag` `#minecraft:supports_mangrove_propagule` | down | all |
| `minecraft:mangrove_sign` | `solid` | down | default |
| `minecraft:mangrove_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:medium_amethyst_bud` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:melon_stem` | `matching_block_tag` `#minecraft:supports_melon_stem` | down | all |
| `minecraft:moss_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:nether_sprouts` | `matching_block_tag` `#minecraft:supports_nether_sprouts` | down | all |
| `minecraft:nether_wart` | `matching_block_tag` `#minecraft:supports_nether_wart` | down | all |
| `minecraft:oak_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:oak_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:oak_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:oak_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:oak_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:oak_sign` | `solid` | down | default |
| `minecraft:oak_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:open_eyeblossom` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:orange_banner` | `solid` | down | default |
| `minecraft:orange_candle_cake` | `solid` | down | default |
| `minecraft:orange_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:orange_tulip` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:orange_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:oxeye_daisy` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:oxidized_copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:oxidized_copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:pale_moss_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:pale_oak_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:pale_oak_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:pale_oak_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:pale_oak_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:pale_oak_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:pale_oak_sign` | `solid` | down | default |
| `minecraft:pale_oak_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:peony` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:pink_banner` | `solid` | down | default |
| `minecraft:pink_candle_cake` | `solid` | down | default |
| `minecraft:pink_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:pink_petals` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:pink_tulip` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:pink_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:pitcher_plant` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:polished_blackstone_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:polished_blackstone_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:poplar_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:poplar_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:poplar_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:poplar_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:poplar_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:poplar_sign` | `solid` | down | default |
| `minecraft:poplar_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:poppy` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:pumpkin_stem` | `matching_block_tag` `#minecraft:supports_pumpkin_stem` | down | all |
| `minecraft:purple_banner` | `solid` | down | default |
| `minecraft:purple_candle_cake` | `solid` | down | default |
| `minecraft:purple_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:purple_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:red_banner` | `solid` | down | default |
| `minecraft:red_candle_cake` | `solid` | down | default |
| `minecraft:red_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:red_mushroom` | `has_sturdy_face` `center` | down | all |
| `minecraft:red_shrub` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:red_tulip` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:red_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:redstone_torch` | `has_sturdy_face` `center` | down | default |
| `minecraft:redstone_wall_torch` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:rose_bush` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:shelf_mushroom` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:short_dry_grass` | `matching_block_tag` `#minecraft:supports_dry_vegetation` | down | all |
| `minecraft:short_grass` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:small_amethyst_bud` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:small_dripleaf` | `matching_block_tag` `#minecraft:supports_small_dripleaf` | down | all |
| `minecraft:snow` | snow layer (SUP-SNOW-001) `#minecraft:cannot_support_snow_layer`, `#minecraft:support_override_snow_layer` | down | all |
| `minecraft:soul_fire` | `matching_block_tag` `#minecraft:soul_fire_base_blocks` | down | all |
| `minecraft:soul_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:soul_torch` | `has_sturdy_face` `center` | down | default |
| `minecraft:soul_wall_torch` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:spore_blossom` | `has_sturdy_face` `center` | up | default |
| `minecraft:spruce_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:spruce_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:spruce_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:spruce_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:spruce_sapling` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:spruce_sign` | `solid` | down | default |
| `minecraft:spruce_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:stone_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:stone_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:sunflower` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:sweet_berry_bush` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:tall_dry_grass` | `matching_block_tag` `#minecraft:supports_dry_vegetation` | down | all |
| `minecraft:tall_grass` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:torch` | `has_sturdy_face` `center` | down | default |
| `minecraft:torchflower` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:tripwire_hook` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:tube_coral` | `has_sturdy_face` `full` | down | default |
| `minecraft:tube_coral_fan` | `has_sturdy_face` `full` | down | default |
| `minecraft:tube_coral_wall_fan` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:wall_torch` | `has_sturdy_face` `full` | east; north; south; west | default |
| `minecraft:warped_button` | `has_sturdy_face` `full` | down; east; north; south; up; west | default |
| `minecraft:warped_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:warped_fungus` | `matching_block_tag` `#minecraft:supports_warped_fungus` | down | all |
| `minecraft:warped_hanging_sign` | `has_sturdy_face` `center` | up | default |
| `minecraft:warped_pressure_plate` | `has_sturdy_face` `ring` or `center` | down | default |
| `minecraft:warped_roots` | `matching_block_tag` `#minecraft:supports_warped_roots` | down | all |
| `minecraft:warped_sign` | `solid` | down | default |
| `minecraft:warped_wall_sign` | `solid` | east; north; south; west | default |
| `minecraft:waxed_copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:waxed_copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:waxed_exposed_copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:waxed_exposed_copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:waxed_oxidized_copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:waxed_oxidized_copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:waxed_weathered_copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:waxed_weathered_copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:weathered_copper_door` | `has_sturdy_face` `full` | down | down, up |
| `minecraft:weathered_copper_lantern` | `has_sturdy_face` `center` | down; up | default |
| `minecraft:white_banner` | `solid` | down | default |
| `minecraft:white_candle_cake` | `solid` | down | default |
| `minecraft:white_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:white_tulip` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:white_wall_banner` | `solid` | east; north; south; west | default |
| `minecraft:wildflowers` | `matching_block_tag` `#minecraft:supports_vegetation` | down | all |
| `minecraft:wither_rose` | `matching_block_tag` `#minecraft:supports_wither_rose` | down | all |
| `minecraft:yellow_banner` | `solid` | down | default |
| `minecraft:yellow_candle_cake` | `solid` | down | default |
| `minecraft:yellow_carpet` | `not` `matching_block_tag` `#minecraft:air` | down | all |
| `minecraft:yellow_wall_banner` | `solid` | east; north; south; west | default |


## Support shape and solid

`minecraft:support` holds the support shape of a state under `boxes`, in the
form of `minecraft:selection_box`: the shape whose faces the face tests of a
placement filter read. It is written only for a state whose support shape is
not its collision shape, for example leaves (empty), snow layers, fence gates
and hanging signs. The shape is asked of the game with no block entity, so a
shulker box has its closed shape.

`mcrs:solid` is written only for a state whose solid value is not the one that
its collision shape gives: not solid when the shape is empty; solid when the
mean of the three sizes of its bounds is `0.7291666666666666` or more, or its
height is a full block; not solid otherwise. A loader computes the value from
`minecraft:collision_box` with exactly that rule.

## What the loader recomputes

Values the game itself derives, or that another asset already states, are not
written here. The dump asserts the first three identities for every state, so a
version that breaks one fails the dump instead of the game.

| Value | Where it comes from instead |
| --- | --- |
| the state is an opaque full cube | the occlusion shape covers the whole block |
| the collision shape is a full block | the collision shape covers the whole block |
| sky light passes down through the state | `light_dampening == 0` |
| the state is air | the identifier, as Bedrock has it: `air`, `cave_air`, `void_air` |
| how the state is drawn | the resource pack. Every state that the game does not draw has a model with no elements, and so do the blocks that a block-entity renderer draws, so the model is the authority on what the block draws |
| the state gives a comparator signal | block behaviour. The flag alone says nothing without the signal value, which is code: container fullness, cake bites, composter level. The container half of it is `minecraft:block_entity.container` |

## `mcrs:` components

What is left is Java data no Bedrock component can carry. Each row states why.

| Component | What it states | Why it has no Bedrock component |
| --- | --- | --- |
| `mcrs:occlusion_shape` | the shape that hides the faces of neighbours and blocks light | `minecraft:geometry.culling_shape` names a registered voxel shape definition, and vanilla registers only `minecraft:unit_cube` and `minecraft:empty`. A per-state box union has no name to give |
| `mcrs:use_shape_for_light_occlusion` | whether the occlusion shape blocks light at all | The light engine reads it to decide whether the occlusion shape counts at all, so it is behaviour and not a hint. Bedrock's light model is one integer per block and has no occlusion shape to switch on |
| `mcrs:emissive_rendering` | the state is drawn at full brightness | The nearest Bedrock field, `material_instances.face_dimming`, turns off directional shading rather than lighting, and lives in a component that must accompany `minecraft:geometry` |
| `mcrs:occlusion_shade` | the factor by which the state darkens the corners of the faces next to it | Bedrock has no per-state value for the darkening of corners. Written only when the value differs from 0.2 for a full collision box and 1.0 otherwise |
| `mcrs:fluid_state` | the fluid in the state | `liquid_detection.can_contain_liquid` states that a block *can* be waterlogged. It carries no level, no source flag, and `liquid_type` accepts only water, so lava and flowing water have nowhere to go |
| `mcrs:experience_drop` | the experience range that breaking the block drops | Bedrock has no experience component at all; its ores drop experience from the loot table's `minecraft:furnace_smelt`-era behaviour rather than a declared range. The value is an int provider, so a constant is a bare integer and anything else is the dispatched object. Absent for a block without a range: the spawner and the creaking heart compute their amount in code |
| `mcrs:solid` | the solid value, when the collision shape does not give it, see below | Bedrock has no solid flag; the value is the one that the data pack predicate `minecraft:solid` and the placement filters read |
| `mcrs:block_sound_set` | The key of the state's entry in the `minecraft:block_sound_set` registry | `minecraft:sound` names a Bedrock sound group, and Java's sound set names are not Bedrock's. Absent for a block with no sound set: water, lava and bubble column |

## Java block properties this corpus does not write

Expressible in Bedrock, not yet dumped:

| Java property | Bedrock component | Why it is not here yet |
| --- | --- | --- |
| the random offset of the model | `minecraft:random_offset` | Java's offset is continuous and derived from the position hash. Bedrock states `steps` and a range, which has no Java counterpart, and the max offsets are only recoverable by sampling the offset |
| the terrain particles on break | `minecraft:destruction_particles` | A boolean against a particle count; the mapping is `false` to `particle_count: 0` and nothing else, and no reader wants it yet |
| the translation key | `minecraft:display_name` | It is `block.<namespace>.<path>` by convention for every vanilla block, so the identifier already states it |

No Bedrock component exists for these, and nothing in mcrs reads them yet: the
speed factor, the jump factor, the bounce, the fall distance reduction, whether
the state suffocates, whether it blocks the view, whether mobs can spawn on it,
whether it ticks randomly, whether it needs post-processing, and the features
that it requires.

## Permutations

A component that is the same for every state sits in `components`. A component
that varies sits in `permutations`, keyed by the properties it actually depends
on. Conditions use only:
`q.block_state('name')`, `==`, `!=`, `&&`, `||`, `!`, parentheses, string and
number literals.

A component absent from a state is absent from that state's permutation, and a
permutation that would state nothing is not written. A loader applies
`components` first, then every matching permutation in order.
