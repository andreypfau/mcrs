# mcrs block definition corpus

Generated from a live Minecraft 26.3-snapshot-9 server (dataVersion 5011) by
`mcrs-block-dumper`. One file per registry block, named after the registry path.

The shape is Bedrock's block definition with three additions under
`description`: `properties` (Java `StateDefinition` declaration order, values in
`getPossibleValues()` order), `base_state_id` and `default_state_id`. State ids
are contiguous: state `n` of a block is `base_state_id + n`.

## Deviations from the Bedrock schema

- `minecraft:selection_box` is an array of boxes, not a single object. A Java
  `VoxelShape` is a union of boxes and splitting the encoding from
  `minecraft:collision_box` would be worse.
- `description.properties` values are typed: integer properties are JSON numbers
  and boolean properties are JSON booleans. Molang conditions compare against the
  matching literal type.
- No `minecraft:geometry`. Block models come from the Java resource pack under
  `assets/minecraft/blockstates/`.

## `mcrs:` components

Java data Bedrock has no faithful equivalent for.

| Component | Java source |
| --- | --- |
| `mcrs:hardness` | `BlockState.getDestroySpeed` (Bedrock's `seconds_to_destroy` is a different quantity) |
| `mcrs:requires_correct_tool_for_drops` | `BlockState.requiresCorrectToolForDrops` |
| `mcrs:is_air` | `BlockState.isAir` |
| `mcrs:replaceable` | `BlockState.canBeReplaced` |
| `mcrs:ignited_by_lava` | `BlockState.ignitedByLava` |
| `mcrs:push_reaction` | `BlockState.getPistonPushReaction` |
| `mcrs:instrument` | `BlockState.instrument` |
| `mcrs:use_shape_for_light_occlusion` | `BlockState.useShapeForLightOcclusion` |
| `mcrs:render_shape` | `BlockState.getRenderShape` |
| `mcrs:fluid_state` | `BlockState.getFluidState`, absent when the state holds no fluid |
| `mcrs:occlusion_shape` | `BlockState.getOcclusionShape`, face culling |
| `mcrs:propagates_skylight_down` | `BlockState.propagatesSkylightDown` |
| `mcrs:emissive_rendering` | `BlockState.emissiveRendering` |
| `mcrs:is_solid_render` | `BlockState.isSolidRender` |
| `mcrs:is_collision_shape_full_block` | `BlockState.isCollisionShapeFullBlock` |
| `mcrs:has_block_entity` | `BlockState.hasBlockEntity` |
| `mcrs:is_signal_source` | `BlockState.isSignalSource` |
| `mcrs:has_analog_output_signal` | `BlockState.hasAnalogOutputSignal` |
| `mcrs:state_components` | dense per-state components, indexed by `state_id - base_state_id` |

`minecraft:light_dampening` comes from `BlockState.getLightDampening`, which is
what 26.3 calls the value older versions exposed as `getLightBlock`.

`mcrs:instrument` is the instrument a note block placed *above* this block plays
(`BlockBehaviour.Properties.instrument`). It is unrelated to the `instrument`
block state property that note blocks themselves carry, which lives in
`description.properties` like any other property.

## Permutations

A component that is the same for every state sits in `components`. A component
that varies sits in `permutations`, keyed by the properties it actually depends
on. Conditions use only:
`q.block_state('name')`, `==`, `!=`, `&&`, `||`, `!`, parentheses, string and
number literals.

When a component depends on every property of a block that has more than one
property, its permutation table would be as dense as the state table, so that
component moves to `mcrs:state_components` instead. Only those components move;
the rest of the block still uses `permutations`. A loader applies `components`
first, then every matching permutation, then `mcrs:state_components` at index
`state_id - base_state_id`.
