# mcrs block definition corpus

Generated from the game's own registries by the `dumpDefinitions` task of
`tools/vanilla-oracle`, at the game version that `assets/minecraft/version.json`
states. One file per registry block, named after the registry path.

The shape is Bedrock's block definition with three additions under
`description`: `properties` (Java `StateDefinition` declaration order, values in
`getPossibleValues()` order), `base_state_id` and `default_state_id`. State ids
are contiguous: state `n` of a block is `base_state_id + n`. `protocol_id` is the
block's index in `BuiltInRegistries.BLOCK`, which is what the network protocol
names a block by.

## Deviations from the Bedrock schema

- `minecraft:selection_box` is an array of boxes, not a single object. A Java
  `VoxelShape` is a union of boxes and splitting the encoding from
  `minecraft:collision_box` would be worse.
- `description.properties` values are typed: integer properties are JSON numbers
  and boolean properties are JSON booleans. Molang conditions compare against the
  matching literal type.
- `minecraft:destructible_by_mining.seconds_to_destroy` is a decimal. It is
  documented as an integer, but it states hardness rather than seconds, and
  hardness is fractional for most of the game.
- `minecraft:movable.movement_type` gains `ignore_entity`, the fifth Java
  `PushReaction`. No vanilla block uses it, but the enum would otherwise be lossy.
- `minecraft:instrument_sound` carries the Java `NoteBlockInstrument` name
  (`harp`) rather than a Bedrock sound alias (`note.harp`). The name selects the
  sound and also decides whether the instrument is tunable by the note property.
- No `minecraft:geometry`. Block models come from the Java resource pack under
  `assets/minecraft/blockstates/`.

## Bedrock components and their Java source

| Component | Java source |
| --- | --- |
| `minecraft:light_emission` | `BlockState.getLightEmission` |
| `minecraft:light_dampening` | `BlockState.getLightDampening`, what older versions exposed as `getLightBlock` |
| `minecraft:friction` | `Block.getFriction` |
| `minecraft:map_color` | `BlockState.getMapColor` |
| `minecraft:collision_box` | `BlockState.getCollisionShape` |
| `minecraft:selection_box` | `BlockState.getShape` |
| `minecraft:destructible_by_mining` | `BlockState.getDestroySpeed`: `false` for an unbreakable state, `true` for an instant one, `seconds_to_destroy` otherwise |
| `minecraft:destructible_by_explosion` | `Block.getExplosionResistance` |
| `minecraft:redstone_conductivity` | `BlockState.isRedstoneConductor` |
| `minecraft:redstone_producer` | `BlockState.isSignalSource`, `power` from `getOwnSignal`, absent when the state is not a source |
| `minecraft:movable` | `BlockState.getPistonPushReaction`; `sticky` is slime and honey, as `PistonStructureResolver.isSticky` has them |
| `minecraft:instrument_sound` | `BlockState.instrument`, under `up` when `worksAboveNoteBlock` and `down` otherwise |
| `minecraft:replaceable` | `BlockState.canBeReplaced`, absent when the state is not replaceable |
| `minecraft:flammable` | `BlockState.ignitedByLava` as `lava_flammable`, absent when lava cannot ignite it |
| `minecraft:block_entity` | `BlockState.hasBlockEntity`, with `container.slot_count` when the block entity is a `Container` |
| `minecraft:loot` | `Block.getLootTable`, absent when the block has none |
| `minecraft:placement_filter` | `BlockState.canSurvive`, see below |

A producer whose power lives in its block entity (comparator, sculk sensor)
reports `power: 0`, which is what it emits with no block entity present.

`item_specific_speeds` states which items harvest a state that needs the right
tool, asked of the game rather than assumed: a tool family goes in as its tag
when every item in that tag harvests the state, and anything left over goes in
by identifier. Stone gets `q.any_tag('minecraft:pickaxes')`; obsidian, which no
wooden pickaxe harvests, gets the diamond and netherite pickaxes by name. The
list is absent exactly when `requiresCorrectToolForDrops` is false, and
`destroy_speed` repeats the hardness because Java applies its harvest bonus as a
divisor rather than a second hardness.

## Placement filter

`minecraft:placement_filter` states which supporting blocks let a block stay
where it is: one condition whose `allowed_faces` is `up` and whose
`block_filter` is one block tag. It is derived by asking the game, never
written by hand.

For each block that overrides `BlockState.canSurvive` the dump calls it on the
default state with a stand-in level that answers one question, the state of the
block directly below, and tries every state of every block there. The blocks it
accepts are the accepted set. A block whose survival test asks the level
anything else (a fluid, the light, a neighbour, the block above) gets no
component, and neither does a block whose accepted set is no tag's members;
nothing is answered with a made-up value. The protected `mayPlaceOn` is not
called: a class can inherit it and never use it, so only the survival test says
what the game accepts.

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

Values Java itself derives, or that another asset already states, are not written
here. The dump asserts the first three identities for every state, so a version
that breaks one fails the dump instead of the game.

| Value | Where it comes from instead |
| --- | --- |
| `BlockState.isSolidRender` | `Block.isShapeFullBlock(occlusion shape)` |
| `BlockState.isCollisionShapeFullBlock` | `Block.isShapeFullBlock(collision shape)` |
| `BlockState.propagatesSkylightDown` | `light_dampening == 0` |
| `BlockState.isAir` | the identifier, as Bedrock has it: `air`, `cave_air`, `void_air` |
| `BlockState.getRenderShape` | the resource pack. Every state Java calls `INVISIBLE` has a model with no elements, and so do the block-entity-rendered blocks it calls `MODEL`, so the model is the authority on what the block draws |
| `BlockState.hasAnalogOutputSignal` | block behaviour. The flag alone says nothing without `getAnalogOutputSignal`, which is code — container fullness, cake bites, composter level. The container half of it is `minecraft:block_entity.container` |

## `mcrs:` components

What is left is Java data no Bedrock component can carry. Each row states why.

| Component | Java source | Why it has no Bedrock component |
| --- | --- | --- |
| `mcrs:occlusion_shape` | `BlockState.getOcclusionShape` | `minecraft:geometry.culling_shape` names a registered voxel shape definition, and vanilla registers only `minecraft:unit_cube` and `minecraft:empty`. A per-state box union has no name to give |
| `mcrs:use_shape_for_light_occlusion` | `BlockState.useShapeForLightOcclusion` | `LightEngine.isEmptyShape` reads it to decide whether the occlusion shape counts at all, so it is behaviour and not a hint. Bedrock's light model is one integer per block and has no occlusion shape to switch on |
| `mcrs:emissive_rendering` | `BlockState.emissiveRendering` | The nearest Bedrock field, `material_instances.face_dimming`, turns off directional shading rather than lighting, and lives in a component that must accompany `minecraft:geometry` |
| `mcrs:fluid_state` | `BlockState.getFluidState` | `liquid_detection.can_contain_liquid` states that a block *can* be waterlogged. It carries no level, no source flag, and `liquid_type` accepts only water, so lava and flowing water have nowhere to go |
| `mcrs:experience_drop` | `xpRange` of `DropExperienceBlock` or of `DropExperienceEntityBlock`, encoded by `IntProviders.CODEC` | Bedrock has no experience component at all; its ores drop experience from the loot table's `minecraft:furnace_smelt`-era behaviour rather than a declared range. The value is Java's `IntProvider`, so a constant is a bare integer and anything else is the dispatched object. Absent for a block that is neither: `SpawnerBlock` and `CreakingHeartBlock` compute their amount in code rather than holding a range |

## Java block properties this corpus does not write

Expressible in Bedrock, not yet dumped:

| Java | Bedrock component | Why it is not here yet |
| --- | --- | --- |
| `Properties.soundType` | `minecraft:sound` | Java's `SoundType` is an object of five sound events with volume and pitch, and has no name to give the component's one string. Naming the constants takes reflection over `SoundType`'s fields, and the Bedrock sound group names are not Java's |
| `Properties.offsetFunction` | `minecraft:random_offset` | Java's offset is continuous and derived from the position hash. Bedrock states `steps` and a range, which has no Java counterpart, and the max offsets are only recoverable by sampling `getOffset` |
| `Properties.spawnTerrainParticles` | `minecraft:destruction_particles` | A boolean against a particle count; the mapping is `false` to `particle_count: 0` and nothing else, and no reader wants it yet |
| `Properties.descriptionId` | `minecraft:display_name` | It is `block.<namespace>.<path>` by convention for every vanilla block, so the identifier already states it |

No Bedrock component exists for these, and nothing in mcrs reads them yet:
`speedFactor`, `jumpFactor`, `bounceRestitution`, `fallDistanceReduction`,
`isSuffocating`, `isViewBlocking`, `isValidSpawn`, `isRandomlyTicking`,
`postProcess` and `requiredFeatures`.

## Permutations

A component that is the same for every state sits in `components`. A component
that varies sits in `permutations`, keyed by the properties it actually depends
on. Conditions use only:
`q.block_state('name')`, `==`, `!=`, `&&`, `||`, `!`, parentheses, string and
number literals.

A component absent from a state is absent from that state's permutation, and a
permutation that would state nothing is not written. A loader applies
`components` first, then every matching permutation in order.
