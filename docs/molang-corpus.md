# Molang corpus of the Bedrock vanilla packs

This note gives aggregate statistics about the Molang expressions of the Bedrock vanilla packs.
The statistics are evidence for the design of `mcrs_molang` (`docs/molang.md`). The note
contains names of queries, functions and JSON fields. It contains no expression text from the
packs, because the licence of the packs does not allow a copy.

## 1. Method

### 1.1 Source

| Field | Value |
|-------|-------|
| Packs | `Mojang/bedrock-samples`, `resource_pack` and `behavior_pack` |
| Version | `1.26.60.5`, commit `15701fa9c139` |
| `min_engine_version` | `1.26.60` in both manifests |
| Tool | `tools/molang-corpus` (crate `mcrs_molang_corpus`) |

Run the tool with this command:

```sh
cargo run --release -p mcrs_molang_corpus -- ~/src/github.com/Mojang/bedrock-samples \
  --queries docs/spec/molang-queries.md \
  --jsonl ../.corpus/molang.jsonl
```

The tool prints the statistics as Markdown. The numbers in this note come from that output.
The tool reads the MOLQ query tables from `docs/spec/molang-queries.md`, so the gap list follows
the specification.

### 1.2 Terms

- **Occurrence**: one Molang value of one JSON field in one pack file.
- **Distinct expression**: the text without blanks, after the fold of names to lower case. Two
  occurrences with the same text are one distinct expression.
- **Text expression**: an occurrence that is a JSON string, a script array or a versioned
  object. A JSON number or a JSON Boolean is not a text expression.
- **Context**: the file kind and the field of an occurrence, for example
  `animation/bones.rotation.keyframe`.
- **External variable**: a `variable.` name that an expression reads and that no expression of
  the corpus writes. The engine supplies the value.

### 1.3 Extraction

The tool parses each JSON file. The parser accepts comments and trailing commas. Each file of
the corpus parses.

The tool reads these fields as Molang:

| File kind | Fields |
|-----------|--------|
| client entity, attachable | `scripts.initialize`, `scripts.pre_animation` (script arrays); `scripts.animate` and `render_controllers` conditions; `scripts.scale*`, `scripts.parent_setup`, `scripts.should_update_*`, each other key of `scripts` except `variables`; attachable `item` conditions |
| animation | `anim_time_update`, `blend_weight`, `start_delay`, `loop_delay`, `loop` (a string other than `hold_on_last_frame`), bone channels `rotation`, `position` and `scale` (static values and keyframes, `pre` and `post`), `timeline`, `particle_effects.*.pre_effect_script` |
| animation controller | state `animations` and `transitions` conditions, `on_entry`, `on_exit`, `parameters`, `variables.*.input`, `particle_effects.*.pre_effect_script` |
| render controller | `arrays`, `geometry`, `textures`, `materials`, `part_visibility`, `color`, `overlay_color`, `on_fire_color`, `is_hurt_color`, `uv_anim`, each other scalar member |
| particle | each string of `components`, except names, modes and colours; `curves.*.input`, `horizontal_range` and `nodes`; `events.*.expression` |
| behaviour block | `permutations[].condition` |

After these fields, the tool sweeps each file for other strings with a namespace, a `.` and a
name (for example `q.x`). The sweep gives the fields with the prefix `other:`. The sweep finds
the behaviour-pack fields, the geometry `binding` field and one bone channel with an unusual
form. The tool does not read `ui`, `texts`, `textures` and `sounds`.

The tool does not read JSON numbers in particle components, behaviour-pack files and other
fields that the sweep finds. Thus the count of constants is a lower bound.

### 1.4 Rate classes

The tool gives each occurrence one rate class of `docs/molang.md`, section 6:

1. A query of the MOLQ tables has the rate class of its table row.
2. A query that MOLQ does not define gets a guess from the words of its name. The guess applies
   to 1,060 of 3,048 query calls.
3. A written variable has the join of the rates of its writes, to a fixed point. A write in
   `scripts.initialize` gives at most `EntityTick`, because it runs one time for each subject.
4. An external variable is `EntityFrame`.
5. `this` is `AnimationFrame` in an animation and `EntityFrame` in each other file kind.
6. A `context.` value is `EntityTick`. A resource reference is `Load`.
7. A random call does not change the rate class. The column `random` counts these occurrences.

The rate of an occurrence is the join of the rates of all its reads. Particle variables have
their own store, because each particle is a separate subject.

## 2. Files and format versions

| File kind | Files | `format_version` values (files) |
|-----------|-------|-------------------------------|
| animation | 143 | `1.8.0` (136), `1.10.0` (7) |
| animation controller | 73 | `1.10.0` (73) |
| attachable | 55 | `1.8.0` (25), `1.10.0` (29), `1.10` (1) |
| client entity | 180 | `1.8.0` (78), `1.10.0` (95), `1.26.0` (7) |
| render controller | 173 | `1.8.0` (142), `1.10.0` (28), `1.10` (3) |
| particle | 192 | `1.10.0` (4), `1.26.10` (180), `1.26.30` (5), `1.26.50` (3) |
| geometry | 198 | `1.8.0` to `1.26.50`, eight values |
| behaviour entity | 128 | `1.21.90` to `1.26.60` |
| behaviour block | 98 | `1.26.50` |

Most client files use the old `format_version` values `1.8.0` and `1.10.0`. The language
rules come from `min_engine_version` (MOL-LOAD-003), and both manifests have `1.26.60`. Thus
the corpus gives no `old_engine_version` warning. A differential test can use the rules of
1.19.60 for each expression of the corpus (MOL-DEV-001).

## 3. Expressions by context

The corpus has 40,006 occurrences and 5,011 distinct expressions. 32,814 occurrences are JSON
numbers or Booleans. 7,192 occurrences are text expressions.

| File kind | Occurrences | JSON numbers | JSON Booleans | Strings | Script arrays | Versioned objects |
|-----------|-------------|--------------|---------------|---------|---------------|-------------------|
| animation | 35,033 | 32,597 | 0 | 2,436 | 0 | 0 |
| render controller | 2,009 | 75 | 128 | 1,806 | 0 | 0 |
| particle | 1,564 | 14 | 0 | 1,517 | 0 | 33 |
| animation controller | 756 | 0 | 0 | 756 | 0 | 0 |
| client entity | 366 | 0 | 0 | 231 | 135 | 0 |
| behaviour entity | 157 | 0 | 0 | 157 | 0 | 0 |
| attachable | 82 | 0 | 0 | 77 | 5 | 0 |
| behaviour item | 16 | 0 | 0 | 16 | 0 | 0 |
| geometry | 10 | 0 | 0 | 10 | 0 | 0 |
| behaviour block | 9 | 0 | 0 | 9 | 0 | 0 |
| behaviour other | 4 | 0 | 0 | 4 | 0 | 0 |

The largest contexts:

| Context | Occurrences | Distinct |
|---------|-------------|----------|
| `animation/bones.rotation.keyframe` | 15,282 | 1,639 |
| `animation/bones.position.keyframe` | 10,287 | 738 |
| `animation/bones.scale.keyframe` | 3,762 | 23 |
| `animation/bones.rotation` | 3,561 | 769 |
| `animation/bones.position` | 1,851 | 510 |
| `render_controller/arrays.textures` | 763 | 384 |
| `animation_controller/states.transitions` | 578 | 241 |
| `render_controller/part_visibility` | 300 | 56 |
| `render_controller/textures` | 265 | 89 |
| `particle/…particle_appearance_tinting.color.gradient` | 262 | 62 |
| `particle/…particle_appearance_billboard.size` | 257 | 39 |
| `render_controller/materials` | 222 | 29 |
| `render_controller/geometry` | 213 | 30 |
| `animation_controller/states.animations` | 178 | 78 |
| `particle/…particle_lifetime_expression.max_lifetime` | 160 | 56 |
| `client_entity/scripts.animate` | 114 | 26 |
| `client_entity/scripts.pre_animation` | 107 | 63 |

These fields have no occurrence in the corpus: animation `loop` as Molang, `blend_weight`,
`loop_delay` and `timeline`, and controller `on_entry`, `on_exit`, `parameters` and
`variables`. `anim_time_update` has 31 occurrences.

## 4. Queries

The corpus calls 177 distinct queries 3,048 times. 307 calls have arguments. 293 of these calls
have only literal arguments. 9 calls go through `->`.

The queries with the most uses:

| Query | Uses | Expressions | MOLQ | Rate | File kinds (expressions) |
|---|---|---|---|---|---|
| `life_time` | 359 | 326 | yes | EntityFrame | animation 278, animation_controller 5, client_entity 24, render_controller 19 |
| `is_baby` | 300 | 296 | yes | EntityTick | animation 6, animation_controller 101, bp_entity 15, client_entity 75, particle 3, render_controller 96 |
| `modified_move_speed` | 281 | 230 | yes | EntityFrame | animation 116, animation_controller 49, client_entity 65 |
| `modified_distance_moved` | 238 | 208 | yes | EntityFrame | animation 157, client_entity 51 |
| `last_hit_by_player` | 84 | 84 | no | EntityTick (guess) | bp_entity 84 |
| `target_x_rotation` | 77 | 77 | yes | EntityFrame | animation 55, client_entity 22 |
| `anim_time` | 74 | 74 | yes | AnimationFrame | animation 73, client_entity 1 |
| `get_equipped_item_name` | 74 | 40 | no | EntityTick (guess) | animation 4, animation_controller 19, attachable 1, render_controller 16 |
| `variant` | 72 | 47 | yes | EntityTick | animation_controller 4, bp_entity 2, client_entity 11, render_controller 30 |
| `target_y_rotation` | 68 | 68 | yes | EntityFrame | animation 62, client_entity 6 |
| `is_saddled` | 60 | 58 | yes | EntityTick | render_controller 58 |
| `frame_alpha` | 58 | 52 | yes | Frame | animation 6, animation_controller 2, attachable 3, client_entity 31, particle 2, render_controller 8 |
| `is_in_water` | 55 | 51 | yes | EntityTick | animation_controller 40, client_entity 9, particle 2 |
| `delta_time` | 48 | 33 | yes | EntityFrame | animation 6, client_entity 27 |
| `is_spectator` | 44 | 44 | no | EntityTick (guess) | client_entity 4, render_controller 40 |
| `model_scale` | 40 | 37 | yes | EntityTick | animation 27, client_entity 10 |
| `is_on_ground` | 39 | 39 | yes | EntityTick | animation_controller 36, bp_entity 1, client_entity 2 |
| `head_y_rotation` | 31 | 31 | yes | EntityFrame | animation 31 |
| `is_riding` | 29 | 29 | yes | EntityTick | animation 1, animation_controller 22, client_entity 6 |
| `get_root_locator_offset` | 28 | 28 | no | EntityFrame (guess) | animation 28 |
| `equipment_count` | 25 | 25 | no | EntityTick (guess) | bp_entity 24, client_entity 1 |
| `is_sitting` | 25 | 20 | no | EntityTick (guess) | animation_controller 18, client_entity 2 |
| `item_is_charged` | 25 | 17 | no | EntityTick (guess) | animation 5, animation_controller 4, render_controller 8 |
| `owner_identifier` | 25 | 25 | no | EntityTick (guess) | attachable 25 |
| `has_rider` | 24 | 21 | yes | EntityTick | animation_controller 1, client_entity 1, render_controller 19 |

The four queries `life_time`, `is_baby`, `modified_move_speed` and `modified_distance_moved`
have 39% of all query uses. The 32 MOLQ queries that the corpus uses have 1,988 uses (65%).

### 4.1 Queries that MOLQ does not define

145 query names, with 1,060 uses, are not in MOLQ. The list is in the order of the count of
uses. A name with the count of the behaviour pack only is a server query.

`last_hit_by_player` 84 (behaviour pack only), `get_equipped_item_name` 74, `is_spectator` 44,
`get_root_locator_offset` 28, `equipment_count` 25 (24 in the behaviour pack), `is_sitting` 25,
`item_is_charged` 25, `owner_identifier` 25, `is_item_equipped` 24, `property` 22,
`is_sleeping` 21, `armor_color_slot` 20, `is_celebrating` 16, `is_grazing` 16,
`is_riding_any_entity_of_type` 16, `swell_amount` 16, `is_angry` 15, `is_standing` 15,
`time_stamp` 14, `has_target` 13, `mark_variant` 13, `has_dash_cooldown` 12,
`standing_scale` 12, `main_hand_item_use_duration` 11, `bone_origin` 10, `is_admiring` 10,
`item_slot_to_bone_name` 10, `lie_amount` 10, `has_any_family` 9,
`main_hand_item_max_duration` 9, `blocking` 8, `invulnerable_ticks` 8, `is_charging` 8,
`is_chested` 8, `is_gliding` 8, `is_in_lava` 8, `is_item_name_any` 8, `roll_counter` 8,
`sit_amount` 8, `all_tags` 7, `armor_texture_slot` 7, `equipped_item_any_tag` 7,
`head_is_in_water` 7, `is_casting` 7, `is_using_item` 7, `max_durability` 7,
`remaining_durability` 7, `get_default_bone_pivot` 6, `is_crawling` 6, `is_digging` 6,
`is_eating` 6, `is_interested` 6, `is_shield_powered` 6, `item_remaining_use_duration` 6,
`previous_squish_value` 6, `anger_level` 5, `bone_aabb` 5, `bone_rotation` 5, `is_dancing` 5,
`is_jump_goal_jumping` 5, `is_playing_dead` 5, `is_shaking_wetness` 5, `is_tamed` 5,
`kinetic_weapon_damage_duration` 5, `kinetic_weapon_delay` 5,
`kinetic_weapon_dismount_duration` 5, `kinetic_weapon_knockback_duration` 5,
`sleep_rotation` 5, `surface_particle_color` 5, `surface_particle_texture_coordinate` 5,
`surface_particle_texture_size` 5, `facing_target_to_range_attack` 4, `get_animation_frame` 4,
`is_eating_mob` 4, `is_laying_egg` 4, `is_pregnant` 4, `is_roaring` 4, `is_stunned` 4,
`position_delta` 4, `sneeze_counter` 4, `structural_integrity` 4, `swelling_dir` 4,
`walk_distance` 4, `current_squish_value` 3, `fuse_time` 3, `is_emerging` 3, `is_powered` 3,
`is_searching` 3, `is_shaking` 3, `is_sheared` 3, `is_stalking` 3, `skin_id` 3,
`swim_amount` 3, `timer_flag_1` 3, `timer_flag_2` 3, `any_tag` 2, `body_x_rotation` 2,
`camera_rotation` 2, `eye_target_x_rotation` 2, `eye_target_y_rotation` 2, `get_name` 2,
`has_player_rider` 2, `hurt_direction` 2, `is_carrying_block` 2, `is_celebrating_special` 2,
`is_charged` 2, `is_croaking` 2, `is_delayed_attacking` 2, `is_leashed` 2, `is_levitating` 2,
`is_resting` 2, `is_scared` 2, `is_sniffing` 2, `is_sonic_boom` 2, `life_span` 2,
`movement_direction` 2, `texture_frame_index` 2, `ticks_since_last_kinetic_weapon_hit` 2,
`time_since_last_vibration_detection` 2, `timer_flag_3` 2, `wing_flap_position` 2, and 24 names
with one use each: `can_damage_nearby_mobs`, `cape_flap_amount`, `had_component_group`,
`has_any_leashed_entity_of_type`, `has_head_gear`, `head_roll_angle`, `health`,
`heartbeat_phase`, `is_attached_to_entity`, `is_elder`, `is_emoting`, `is_ghost`, `is_in_ui`,
`is_jumping`, `is_persona_or_premium_skin`, `overlay_alpha`, `player_level`, `shake_angle`,
`shake_time`, `show_bottom`, `tail_angle`, `trade_tier`, `unhappy_counter`, `wing_flap_speed`.

The particle query `spellcolor` is not in this list, because its expressions do not parse
(section 9).

### 4.2 MOLQ queries that the corpus does not use

`all`, `any`, `approx_eq`, `in_range`, `distance_from_camera`, `equipment_asset`,
`equipment_texture`, `is_on_fire`, `is_sprinting`, `pose`, `time_of_day`, `variant_asset` and
`variant_model`. Some of these, for example `pose` and `variant_asset`, are mcrs extensions.

### 4.3 Arguments

- 14 calls have an argument that is not a literal. 10 of them are
  `item_slot_to_bone_name(c.item_slot)` in the geometry field `binding`.
- Most calls have 1 to 4 arguments. Three queries have more: `has_any_family` (14 or 15),
  `any_tag` (up to 12) and `has_any_leashed_entity_of_type` (9). These are list tests.
- The corpus has no call with an empty argument list, such as `q.is_baby()`.

## 5. Math functions

| Function | Uses | Function | Uses |
|----------|------|----------|------|
| `random` | 787 | `max` | 30 |
| `cos` | 481 | `sqrt` | 25 |
| `sin` | 419 | `lerprotate` | 7 |
| `clamp` | 177 | `pi` | 7 |
| `lerp` | 122 | `ease_in_circ` | 5 |
| `mod` | 104 | `ease_in_out_elastic` | 5 |
| `pow` | 74 | `ease_out_circ` | 5 |
| `abs` | 46 | `die_roll` | 3 |
| `round` | 40 | `ceil` | 2 |
| `inverse_lerp` | 37 | `min_angle` | 2 |
| `min` | 36 | `ln` | 1 |
| `floor` | 31 | | |

- Each name is in MOL-MATH-002. Each call has the arity of MOL-MATH-002.
- `math.pi` has no argument list in each use.
- The corpus does not use `exp`, `trunc`, `asin`, `acos`, `atan`, `atan2`, `sign`,
  `copy_sign`, `hermite_blend`, `random_integer`, `die_roll_integer` and 27 of the 30 easing
  functions.
- `random` has 787 uses. 638 occurrences with a random call are in particles.

## 6. Constructs

| Construct | Uses | Expressions | File kinds |
|-----------|------|-------------|------------|
| `*` | 4,696 | 1,974 | all except geometry and behaviour block |
| read `v.` | 4,183 | 2,366 | animation, controller, client entity, particle, render controller |
| query call | 3,048 | 1,993 | all |
| math call | 2,446 | 1,686 | all except geometry and behaviour block |
| `+` | 1,994 | 1,318 | |
| `-` | 1,845 | 1,217 | |
| unary `-` | 1,696 | 1,565 | |
| `this` | 1,390 | 1,353 | animation, render controller, behaviour entity |
| assign `v.` | 1,017 | 275 | animation, controller, attachable, client entity, particle |
| `texture.` reference | 988 | 955 | render controller |
| `? :` | 722 | 541 | |
| read `v.a.b` (struct path) | 704 | 493 | animation, controller, client entity, particle, render controller |
| string literal | 522 | 196 | |
| `==` | 416 | 362 | |
| unary `!` | 402 | 329 | |
| `/` | 395 | 325 | |
| `&&` | 316 | 214 | |
| `geometry.` reference | 277 | 237 | render controller |
| `material.` reference | 248 | 226 | render controller |
| `>` | 184 | 171 | |
| read `t.` | 170 | 9 | client entity |
| string compare `==` or `!=` | 148 | 100 | |
| `<` | 130 | 79 | |
| `array.` reference, index `[]` | 123 | 93 | render controller |
| `\|\|` | 115 | 88 | |
| assign `t.` | 89 | 9 | client entity |
| read `c.` | 59 | 57 | animation, controller, attachable, geometry, behaviour item |
| `!=` | 34 | 28 | |
| `return` | 33 | 33 | animation, controller, particle |
| `??` | 31 | 18 | animation, controller, client entity, particle |
| struct write `v.a.b =` | 31 | 5 | client entity |
| `<=` | 27 | 23 | |
| `>=` | 21 | 21 | |
| `->` | 13 | 11 | animation, controller, behaviour item |
| binary `?` | 5 | 5 | client entity |
| block branch `? { }` | 5 | 5 | client entity |

- The corpus has no `loop`, no `for_each`, no `break` and no `continue`.
- The corpus has no block statement outside a branch of `?`.
- A string literal is an operand of `==` or `!=`, or a query argument. No string is an operand
  of another operator.
- No `t.a.b` path occurs. No assignment to `c.` occurs.
- `this` occurs outside animations 47 times: in the render-controller channels `color`,
  `overlay_color` and `is_hurt_color`, and in four behaviour-entity conditions.
- 281 occurrences (135 distinct) are complex expressions. Each other occurrence is a simple
  expression.

## 7. Variables

- The entity store has 550 distinct names. The particle store has 139 distinct names.
- 145 distinct names are struct paths (`v.a.b`): 83 in entity files and 62 in particles. Many
  are the members `x`, `y` and `z` of a vector.
- The `context.` names are `is_first_person`, `item_slot`, `other`, `owning_entity` and
  `player_offhand_arm_height`. `c.other` and `c.owning_entity` are entity references on the
  left side of `->`.
- 242 names are external variables. The external entity variables with the most reads are
  `stand_anim` (202), `attack_time` (118), `swim_amount` (52), `gliding_speed_value` (32),
  `first_person_item_rotation_factor` (20) and `is_brandishing_spear` (19). The external
  particle variables with the most reads are `particle_random_1` (343), `particle_age` (180),
  `particle_random_2` (175), `particle_random_3` (86) and `particle_lifetime` (69).
- The written variables by rate class: entity store `Constant` 53, `EntityTick` 67,
  `EntityFrame` 287, `AnimationFrame` 1; particle store `Constant` 25, `EntityFrame` 14.

## 8. Sizes

| Measure | Set | Count | p50 | p90 | p99 | max |
|---|---|---|---|---|---|---|
| tokens | occurrences | 39,990 | 1 | 5 | 27 | 4,626 |
| AST nodes | occurrences | 39,990 | 2 | 4 | 14 | 1,880 |
| statements of complex expressions | occurrences | 281 | 2 | 8 | 36 | 176 |
| tokens | distinct | 5,011 | 3 | 19 | 65 | 4,626 |
| AST nodes | distinct | 5,011 | 2 | 11 | 31 | 1,880 |
| statements of complex expressions | distinct | 135 | 3 | 10 | 38 | 176 |

The tool counts one AST node for each operator, operand, call, path, statement and block. A
path such as `v.a.b` is one node. The largest expression is one `pre_animation` script array of
23 KB. Four `pre_animation` scripts have more than 4,000 characters.

## 9. Rate classes

All JSON forms:

| Group | Total | Constant | Load | Frame | EntityTick | EntityFrame | AnimationFrame | random |
|---|---|---|---|---|---|---|---|---|
| all | 39,990 | 84.5% | 3.3% | 0.0% | 2.9% | 5.7% | 3.5% | 749 |
| animation keyframe | 29,331 | 99.8% | 0.0% | 0.0% | 0.0% | 0.0% | 0.2% | 0 |
| animation bone channel | 5,660 | 59.4% | 0.0% | 0.1% | 2.5% | 15.2% | 22.8% | 0 |

Text expressions only:

| Group | Total | Constant | Load | Frame | EntityTick | EntityFrame | AnimationFrame | random |
|---|---|---|---|---|---|---|---|---|
| all | 7,176 | 13.7% | 18.5% | 0.1% | 16.4% | 32.0% | 19.3% | 749 |
| animation bone channel | 2,314 | 0.7% | 0.0% | 0.1% | 6.2% | 37.2% | 55.7% | 0 |
| animation keyframe | 80 | 12.5% | 0.0% | 0.0% | 2.5% | 5.0% | 80.0% | 0 |
| animation other | 42 | 9.5% | 0.0% | 0.0% | 11.9% | 61.9% | 16.7% | 7 |
| animation controller | 756 | 20.9% | 0.0% | 0.3% | 52.4% | 23.5% | 2.9% | 2 |
| client entity and attachable | 448 | 27.0% | 0.0% | 0.0% | 35.7% | 37.1% | 0.2% | 7 |
| render controller | 1,806 | 3.2% | 73.1% | 0.0% | 17.3% | 6.2% | 0.2% | 0 |
| particle | 1,534 | 38.1% | 0.0% | 0.0% | 0.3% | 61.5% | 0.0% | 638 |
| other | 196 | 15.3% | 4.6% | 0.0% | 76.0% | 4.1% | 0.0% | 95 |

- 99.8% of keyframe values are JSON numbers. A keyframe value is constant, but the sampler of
  the channel reads `anim_time`.
- In bone channels, 93% of text expressions need a value for each frame. 56% are
  `AnimationFrame`: they read `this`, `anim_time` or a variable that an animation writes.
- In render controllers, 73% of text expressions are `Load`: a resource reference, or an
  array element with a constant index.
- In particles, the `EntityFrame` share comes from external particle variables, for example
  `particle_age`. These values change for each particle, not for each entity.
- `Frame` alone is rare: 5 text expressions read only frame values such as `frame_alpha`.

## 10. Gaps in MOL

The parser of the tool follows MOL-SYN-001. 16 occurrences do not parse, all in particles.
Each item names the rule that does not cover the construct.

1. **`??` with a block on the right side.** 12 occurrences in
   `emitter_initialization.creation_expression` have the form `v.d ?? { v.d.x = …; … };`. The
   block sets a default struct when the caller gives none. MOL-SYN-001 allows only a
   `coalesce` operand on the right side of `??`.
2. **A member of a query result.** 3 occurrences in `particle_appearance_tinting.color.gradient`
   read `query.spellcolor.r`, `.g` and `.b`. MOL-SYN-001 gives `query_ref` one name, and
   MOL-VAL-001 has no query that returns a struct.
3. **An assignment inside parentheses as a branch.** 1 occurrence in
   `emitter_initialization.per_update_expression` has the form `v.c ? ( v.a = … );`.
   MOL-SYN-001 allows an assignment only as a statement.
4. **A versioned value object.** 33 particle values are objects with the members
   `expression` and `version` (value `12` in each). MOL-LOAD-001, item 4, makes an object a load
   error `type`.

These constructs parse, but other rules give them a later milestone or no meaning:

5. **Script arrays.** 5 arrays have 10 entries that do not parse alone. The join of
   MOL-LOAD-002 is necessary.
6. **`this` outside animations.** 47 occurrences in render-controller colour channels and
   behaviour conditions. MOL-VAR-007 leaves the meaning to the consumer. The render-controller
   consumer must state it.
7. **`->`.** 13 uses in 11 expressions. The left side is always `c.other` or
   `c.owning_entity`. MOL-VAR-006 has the milestone `later`.
8. **Struct variables.** 704 reads and 31 writes of `v.a.b` paths (MOL-VAR-005, milestone
   `later`).
9. **`return`, `??` and `math.random`.** These have 33, 31 and 787 uses. They are frequent
   enough to need a milestone.
10. **Geometry `binding`.** 10 occurrences in geometry files are Molang. No consumer
    specification lists this field.
11. **External variables.** MOL and MOLQ do not list the 242 variables that the engine
    supplies, for example `v.attack_time` and `v.particle_age`. A host must declare them like
    queries.

## 11. Facts for the IR and the VM

- Straight-line code dominates: 39,709 of 39,990 occurrences are simple expressions. The
  p99 size of a distinct expression is 65 tokens and 31 AST nodes.
- A few `pre_animation` scripts are large: up to 176 statements and 4,626 tokens. The compiler
  must not have a cost that grows faster than linearly in the expression size.
- 84.5% of all occurrences, and 99.8% of keyframes, are constants. A keyframe track can store
  numbers and skip the VM.
- The control flow is `? :`, `&&`, `||`, `??`, one level of block branches and `return`. No
  loop occurs.
- Four queries have 39% of the query uses. 293 of 307 calls with arguments have only literal
  arguments, so a host can resolve most calls at load.
- `random` is in 10.4% of text expressions, nearly all in particles. A random draw keeps an
  expression on the CPU (MOL-RAND-001).
- `t.` occurs only in 9 client-entity scripts. A temporary variable can be an SSA value.

## 12. Parse coverage gate

The tool writes the corpus to `/Users/andreypfau/orca/workspaces/mcrs/.corpus/molang.jsonl`
with the option `--jsonl`. The file is outside the repository, because it contains the
expression text of the packs. Do not commit it. Each line is one occurrence:

| Member | Content |
|--------|---------|
| `expr` | the expression text; the join for a script array; the number text for a JSON number |
| `form` | `string`, `number`, `bool`, `script_array` or `versioned_object` |
| `kind`, `field` | the context |
| `file`, `pointer` | the pack file and the JSON pointer of the value |
| `format_version` | the `format_version` of the file |
| `version` | the `version` of a versioned object |
| `entries` | the entries of a script array, before the join |
| `parse` | `ok`, or the error of the parser of the tool |

The parser of `mcrs_molang` must parse each line with the form `string`, `script_array` or
`versioned_object`. The 16 lines with a parse error need the grammar changes of section 10,
items 1 to 3. A line with the form `number` or `bool` is a constant (MOL-LOAD-001) and needs no
parse.
