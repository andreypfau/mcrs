# Molang corpus of the Bedrock vanilla packs

This note states how `tools/molang-corpus` extracts the Molang expressions of the Bedrock vanilla
packs. The tool prints the statistics. The note contains no expression text from the packs,
because the licence of the packs does not allow a copy.

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
cargo run --release -p mcrs_molang_corpus -- <path to bedrock-samples> \
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
   `scripts.initialize` gives at most `entity_tick`, because it runs one time for each subject.
4. An external variable is `entity_frame`.
5. `this` is `animation_frame` in an animation and `entity_frame` in each other file kind.
6. A `context.` value is `entity_tick`. A resource reference is `load`.
7. A random call does not change the rate class. The column `random` counts these occurrences.

The rate of an occurrence is the join of the rates of all its reads. Particle variables have
their own store, because each particle is a separate subject.

## 12. Parse coverage gate

The tool writes the corpus to `.corpus/molang.jsonl`
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
`versioned_object`. A line with the form `number` or `bool` is a constant (MOL-LOAD-001) and needs no
parse.
