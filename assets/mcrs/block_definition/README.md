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
| `minecraft:placement_filter` | the supporting blocks that let the block stay, see below |

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

`minecraft:placement_filter` states which supporting blocks let a block stay
where it is: one condition whose `allowed_faces` is `up` and whose
`block_filter` is one block tag. It is derived by asking the game, never
written by hand.

For each block with its own survival test, the dump runs the test on the
default state with a stand-in level that answers one question, the state of the
block directly below, and tries every state of every block there. The blocks it
accepts are the accepted set. A block whose survival test asks the level
anything else (a fluid, the light, a neighbour, the block above) gets no
component, and neither does a block whose accepted set is no tag's members;
nothing is answered with a made-up value. Only the survival test says what the
game accepts.

Several block tags can hold the same members, and equal members do not say
which tag the game tests. The dump groups the block tags by members on every
run and probes each group: it rebinds the block tags with one distinct marker
block added to each tag of the group, asks the game again, and names the one tag
whose members under that overlay equal the accepted set under it. The tags are
rebound unchanged before anything else reads them. When no tag matches, the
block gets no component.

The blocks that carry the component:

| Block | Tag |
| --- | --- |
| `minecraft:acacia_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:allium` | `#minecraft:supports_vegetation` |
| `minecraft:attached_melon_stem` | `#minecraft:supports_melon_stem` |
| `minecraft:attached_pumpkin_stem` | `#minecraft:supports_pumpkin_stem` |
| `minecraft:azalea` | `#minecraft:supports_azalea` |
| `minecraft:azure_bluet` | `#minecraft:supports_vegetation` |
| `minecraft:bamboo` | `#minecraft:supports_bamboo` |
| `minecraft:bamboo_sapling` | `#minecraft:supports_bamboo` |
| `minecraft:birch_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:blue_orchid` | `#minecraft:supports_vegetation` |
| `minecraft:bush` | `#minecraft:supports_vegetation` |
| `minecraft:cherry_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:closed_eyeblossom` | `#minecraft:supports_vegetation` |
| `minecraft:cornflower` | `#minecraft:supports_vegetation` |
| `minecraft:crimson_fungus` | `#minecraft:supports_crimson_fungus` |
| `minecraft:crimson_roots` | `#minecraft:supports_crimson_roots` |
| `minecraft:dandelion` | `#minecraft:supports_vegetation` |
| `minecraft:dark_oak_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:dead_bush` | `#minecraft:supports_dry_vegetation` |
| `minecraft:fern` | `#minecraft:supports_vegetation` |
| `minecraft:firefly_bush` | `#minecraft:supports_vegetation` |
| `minecraft:flowering_azalea` | `#minecraft:supports_azalea` |
| `minecraft:golden_dandelion` | `#minecraft:supports_vegetation` |
| `minecraft:jungle_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:large_fern` | `#minecraft:supports_vegetation` |
| `minecraft:lilac` | `#minecraft:supports_vegetation` |
| `minecraft:lily_of_the_valley` | `#minecraft:supports_vegetation` |
| `minecraft:mangrove_propagule` | `#minecraft:supports_mangrove_propagule` |
| `minecraft:melon_stem` | `#minecraft:supports_melon_stem` |
| `minecraft:nether_sprouts` | `#minecraft:supports_nether_sprouts` |
| `minecraft:nether_wart` | `#minecraft:supports_nether_wart` |
| `minecraft:oak_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:open_eyeblossom` | `#minecraft:supports_vegetation` |
| `minecraft:orange_tulip` | `#minecraft:supports_vegetation` |
| `minecraft:oxeye_daisy` | `#minecraft:supports_vegetation` |
| `minecraft:pale_oak_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:peony` | `#minecraft:supports_vegetation` |
| `minecraft:pink_petals` | `#minecraft:supports_vegetation` |
| `minecraft:pink_tulip` | `#minecraft:supports_vegetation` |
| `minecraft:pitcher_plant` | `#minecraft:supports_vegetation` |
| `minecraft:poplar_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:poppy` | `#minecraft:supports_vegetation` |
| `minecraft:pumpkin_stem` | `#minecraft:supports_pumpkin_stem` |
| `minecraft:red_shrub` | `#minecraft:supports_vegetation` |
| `minecraft:red_tulip` | `#minecraft:supports_vegetation` |
| `minecraft:rose_bush` | `#minecraft:supports_vegetation` |
| `minecraft:short_dry_grass` | `#minecraft:supports_dry_vegetation` |
| `minecraft:short_grass` | `#minecraft:supports_vegetation` |
| `minecraft:soul_fire` | `#minecraft:soul_fire_base_blocks` |
| `minecraft:spruce_sapling` | `#minecraft:supports_vegetation` |
| `minecraft:sunflower` | `#minecraft:supports_vegetation` |
| `minecraft:sweet_berry_bush` | `#minecraft:supports_vegetation` |
| `minecraft:tall_dry_grass` | `#minecraft:supports_dry_vegetation` |
| `minecraft:tall_grass` | `#minecraft:supports_vegetation` |
| `minecraft:torchflower` | `#minecraft:supports_vegetation` |
| `minecraft:warped_fungus` | `#minecraft:supports_warped_fungus` |
| `minecraft:warped_roots` | `#minecraft:supports_warped_roots` |
| `minecraft:white_tulip` | `#minecraft:supports_vegetation` |
| `minecraft:wildflowers` | `#minecraft:supports_vegetation` |
| `minecraft:wither_rose` | `#minecraft:supports_wither_rose` |


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
