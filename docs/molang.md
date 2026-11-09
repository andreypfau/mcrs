# Molang: compiler and runtime

This note describes the design of `mcrs_molang`. The crate parses, compiles and evaluates
Molang for every host in mcrs: block definitions and client entities. The language itself is
stated in `docs/spec/molang.md`. This note states how mcrs executes it.

## 1. Goals and limits

- One implementation for every host. The block-condition subset in
  `crates/mcrs_minecraft_block/src/definition/molang.rs` goes away (section 12).
- Modern semantics only. A pack with an older `min_engine_version` loads with the modern rules,
  and the loader logs one warning for the pack. The crate does not emulate old versions.
- No expression text survives into a frame. Every expression becomes a program before the first
  frame that uses it.
- Many entities share one program. The hot path evaluates one program for a block of entities.
- The same rules hold on native and on wasm. The crate has no JIT, no `unsafe` and no
  platform code.
- A channel that only reads time and motion inputs runs on the GPU (section 10).

Workload facts from the pinned Bedrock vanilla packs, `bedrock-samples` `1.26.60.5`
(`docs/molang-corpus.md`):

- 40,006 occurrences reduce to 5,011 distinct expressions. 32,814 occurrences are JSON numbers
  or Booleans, and 99.8% of keyframe values are numbers.
- 39,709 of 39,990 parsed occurrences are simple expressions. The p99 size of a distinct
  expression is 65 tokens and 31 AST nodes. The largest is one `pre_animation` script with
  176 statements and 4,626 tokens.
- The packs have no `loop`, no `for_each`, no `break` and no `continue`. 11 expressions use
  `->` (13 uses), always with `c.other` or `c.owning_entity` on the left side.
- Four queries have 39% of the query uses. 293 of 307 calls with arguments have only literal
  arguments.
- The rate classes of the 7,176 text expressions: `Constant` 13.7%, `Load` 18.5%, `Frame`
  0.1%, `EntityTick` 16.4%, `EntityFrame` 32.0%, `AnimationFrame` 19.3%. In bone channels, 93%
  of text expressions need a value in each frame, and 55.7% are `AnimationFrame`. In render
  controllers, 73.1% are `Load`.
- `math.random` occurs in 10.4% of text expressions, nearly all in particles.

## 2. Crates

| Crate | Content | Depends on |
|---|---|---|
| `mcrs_molang` | lexer, parser, IR, passes, bytecode, scalar VM, batch VM, math library, the classification of channels | none of the game crates |
| `mcrs_molang_wgsl` | lowering of a pure program to WGSL, the WGSL math library, the CPU twin | `mcrs_molang` |

The core crate knows no query. Each host declares its queries:

- `mcrs_minecraft_block` declares `query.block_state`.
- The entity animation crate declares the entity queries of `docs/spec/molang-queries.md`.

The source tree of `mcrs_molang` is flat. One file holds one primary item:

```
lexer.rs  syntax.rs  parse.rs  diagnostic.rs  value.rs  layout.rs  query.rs  resource.rs
ir.rs  lower.rs  lower_branch.rs  lower_call.rs  pass.rs  bytecode.rs  emit.rs  compile.rs
math.rs  subject.rs  scalar.rs  batch_subject.rs  batch.rs
input_range.rs  pure_program.rs  channel_class.rs
```

The classification needs the bytecode, so it stays in `mcrs_molang`. `mcrs_molang_wgsl` is a
separate crate because its GPU tests need `wgpu` and `naga`.

`intern.rs`, `random.rs` and `disassemble.rs` come with the first consumer that needs them.

The crate follows the workspace style:

- It has no `dyn` and no `Arc`. Programs live in an arena and are referenced by `ProgramId`.
- It does not panic on input. A diagnostic carries a span.
- Hosts plug in through generic parameters, not through trait objects.

## 3. Pipeline

```
text ─lex─► tokens ─parse─► syntax tree (spans) ─intern─► ExpressionId
     ─lower(layout, queries)─► IR (SSA, typed, rated)
     ─passes─► IR ─emit─► bytecode ─► scalar VM │ batch VM
                                   └─(pure only)─► WGSL + CPU twin
```

Each stage is a pure function of its inputs. A hot reload repeats the pipeline for the changed
pack and swaps the programs in the arena.

## 4. Front end

### 4.1 Lexer and parser

- The lexer folds identifiers and keywords to lower case. It keeps string literals as they are.
- The parser is a precedence-climbing parser over the modern precedence table of the spec.
- Every syntax node carries a byte span into the source text.
- The parser does not stop at the first error. It records diagnostics with spans and continues
  at the next `;` or closing brace. The loader reports all diagnostics of a file together.
- `loop`, `for_each`, `break`, `continue` and `->` parse in every milestone. Until a pack needs
  them, lowering rejects them with a diagnostic that names the construct.

### 4.2 Joined script arrays

A client entity gives `initialize` and `pre_animation` as arrays of strings. One brace block
can open in one entry and close in a later entry. Thus the loader joins the entries of one array
with no text between them (`docs/spec/molang.md`, MOL-LOAD-002) and parses the result as one
program. A separator would change the meaning of an entry that ends inside an expression. A span maps back to the entry and the column.

### 4.3 Interning

The interner keys an expression by its normalised text and by the layout that resolves it:

- Normalisation removes whitespace outside string literals and folds case outside string
  literals.
- The layout key is the slot layout of the host type (section 5).

Equal keys give one `ExpressionId` and one program. The vanilla pack compiles to about 3,100
programs. A full compile costs milliseconds.

## 5. Slots

### 5.1 Variable layout

Molang variables have names. The evaluator reads them by index:

1. At load, the compiler collects every `variable.*` name that a script of a host type reads or
   writes.
2. The compiler flattens a struct path (`v.a.b`) to one name.
3. Each name gets a dense `u16` slot in the `VariableLayout` of the host type.
4. An entity stores its values as a column of `f32` in the layout order.

Each slot has an "is set" bit. A read of a slot with a clear bit is a missing value: it gives 0,
and inside the left side of `??` it selects the right side. A write sets the bit.

A hot reload builds a new layout. The runtime moves each value to the slot with the same name
in the new layout. A name that the new layout does not have is dropped.

### 5.2 Other storage

| Storage | Lifetime | Index |
|---|---|---|
| `temp.*` | one run of a program | SSA value; no memory slot |
| `context.*` | one call site of the host | `ContextId` that the host declares |
| sampled query | one rate period (section 6) | query slot in the column of the host |
| `this` | one bone channel | program input 0 |

### 5.3 Block hosts

A block host has no variables. Its query slots are the properties of the block:

- `query.block_state('<name>')` with a literal name resolves to the slot of property `<name>`
  at compile time.
- A name that the block does not have is a compile error.
- A comparison with a literal that is not in the domain of the property is a compile error.

These are the two checks of the current subset. Section 12 states the migration.

## 6. Rate classes

Each IR value has a rate class. The rate class states how often the value can change:

| Rate | The value is fixed for | Examples |
|---|---|---|
| `Constant` | always | literals, folded math |
| `Load` | one pack load and one host key | block property values of one block state, resource ids, literal query arguments |
| `Frame` | one frame, for all entities | partial tick, time of day, camera position |
| `EntityTick` | one client tick of one entity | `is_baby`, `variant`, `is_on_ground`, synced data |
| `EntityFrame` | one frame of one entity | `life_time`, `modified_distance_moved`, head rotations |
| `AnimationFrame` | one frame of one animation instance of one entity | `anim_time`, `key_frame_lerp_time`, `this` |

The rate of a value is the highest rate of its operands. A query declares its rate. A write to a
variable gives the variable the rate of the program that writes it.

The compiler hoists each value to its rate:

- A `Load` value is computed once per host key. Block conditions end here (section 12).
- An `EntityTick` value is computed in the tick stage, not in each frame.
- An `EntityFrame` value that depends only on tick values and the partial tick is interpolated.
  It is not recomputed from the entity.

The time of an entity is one clock, `C = age + pt`, with the age in the ticks that the entity
ran. `life_time`, `anim_time` and the start of each animation instance are values of this clock.
`docs/entity-render.md`, section 4.2, states how the GPU gets it.

The rate also selects the processor for an entity channel (section 10).

## 7. IR

The IR is SSA over basic blocks. Value types:

| Type | Representation |
|---|---|
| `Num` | `f32` |
| `Str` | 64-bit FNV-1a hash of the text; only `==` and `!=` |
| `Res` | resource kind (`geometry`, `material`, `texture`) and a `u32` id |
| `Entity` | an entity handle that the host gives |

Booleans are `Num` values `0.0` and `1.0`.

Operations:

- `LoadVar(slot)`, `StoreVar(slot)`, `LoadQuery(slot)`, `CallQuery(id, args)`,
  `LoadContext(id)`, `This`, `Frame(id)`, `Const(value)`.
- `Unary`, `Binary`, `Compare`, `Math(fn, args)`.
- `Random(kind, call_site, args)`. It has an effect, and no pass folds it or merges it.
- Control: `Branch`, `Select`, `Loop { max: 1024 }`, `Return`.
- `Coalesce(region, fallback)`. The region has an error edge to the fallback block. A missing
  value, an invalid entity reference or a guard error inside the region takes the edge.
- `WithEntity(reference, region)`. The region runs as another entity, and reads its public
  snapshot.

`&&`, `||`, `?:`, `? ` without else and `??` lower to control flow, so their right sides stay
lazy. The select pass (section 8) turns a cheap, pure diamond back into a `Select`.

## 8. Passes

The scalar VM without passes is the reference. A pass must give results that are equal bit for
bit to the reference for every input, NaN, ±∞ and −0 included.

Allowed passes:

1. Constant folding of pure operations with `Constant` operands. Random, queries, writes and
   division are excluded from folding when the guard can change the result.
2. Deletion of code after `return`, and of branches with a constant condition.
3. Rate analysis and hoisting (section 6).
4. Value numbering and CSE over all outputs of one program. Random is never merged.
5. Copy propagation of temps.
6. Exact identities only: `x*1`, `x/1`, `x+(-0)`, `x-0`.
7. Select formation for a diamond whose arms are pure and cheap.

Forbidden rewrites:

| Rewrite | Why it is wrong |
|---|---|
| `x*0 → 0` | NaN and ∞ give NaN; a random draw or a write disappears |
| `x-x → 0` | the same reason |
| `x+0 → x` | `-0 + 0` is `+0` |
| reassociation or FMA contraction | rounding changes |
| eager evaluation of a lazy arm with an effect | the effect order changes |

A program groups the outputs of one unit, so that CSE shares work across them:

- an animation: all bone channels, the weight, and the time update;
- a client entity script: `initialize`, `pre_animation`, `scale`;
- a controller state: all transitions and blend conditions;
- a render controller: all selections and visibility rules.

## 9. Execution

### 9.1 Bytecode

The bytecode is register based. A register holds one `f32`, or a `u64` for a string hash. An
instruction is 8 bytes: one opcode and up to three register operands. A side table maps each
instruction to a source span.

### 9.2 Scalar VM

The scalar VM runs one program for one host instance. It is:

- the reference for tests;
- the only executor for a program with `Loop`, `WithEntity` or a struct copy;
- the debug tool: `disassemble` prints the bytecode with the source span of each line.

### 9.3 Batch VM

The batch VM runs one program for a block of lanes. A lane is one entity, or one entity with one
animation.

- A block has 64 to 256 lanes. The block size is a constant; measure before you change it.
- A varying register is a column `[f32; LANES]`. A uniform register (`Constant`, `Load`,
  `Frame`) stays one scalar and broadcasts.
- Each instruction is one loop over the columns. The loops must vectorise on x86-64, AArch64 and
  wasm `simd128`. Check this with `cargo asm` on each target.
- A `Select` evaluates both arms and selects by mask.
- A lazy arm with an effect runs under a lane mask. When a mask is sparse, the VM compacts the
  active lanes into an index list.
- Values are written back to the variable columns at the end of the block, in lane order.

The batch VM must give the same results as the scalar VM bit for bit. Both VMs call one
implementation of each math function in `math.rs`.

### 9.4 Stages

The entity host runs the stages in this order:

1. Sample the queries of each entity into its query slots.
2. Run `pre_animation` per entity type, in batches.
3. Run the transitions of the animation controllers per entity type and state, in batches.
4. Run the CPU channels per animation, in batches.
5. Run the render controllers.

Each stage reads the results of the stage before it. A stage does not read values that the same
stage writes for another lane.

### 9.6 Interface for consumers

The animation crate and the render crate need these items from `mcrs_molang`. They are part of
the public interface:

- **One query slot.** A reader for the value of one slot of a query value set, for a lane, so
  that a host can check or copy a single input without a full sample.
- **Inputs and outputs of a program.** For each compiled `Program` and `Expression`, the list of
  the variables that it reads, the variables that it writes, and the query slots that it reads.
  A host uses the lists to find the inputs of a script without a second parse, and to skip a
  script whose inputs did not change (CENT-SCR-006).
- **Lane mask of the batch VM.** A batch run takes a mask of active lanes. An inactive lane keeps
  its variables and gives no result. With the mask, a block with some inactive lanes stays in the
  batch VM instead of a fall back to the scalar VM.

### 9.5 Random

`math.random` and the functions built on it follow `docs/spec/molang.md`, MOL-RAND-001:

- Each subject has one stream. The seed is the network id of the entity.
- A program draws from the stream of its subject in source order.
- No pass folds, hoists or merges a random draw.
- A random draw runs only on the CPU. A channel with a random call is in the class `State`
  (section 10.1).

Thus a result depends only on the entity and on the sequence of runs of that entity. It does not
depend on the order of lanes or on the size of a batch. Equality with the game is not a goal.

## 10. GPU channels

### 10.1 Classification

After the passes, the classifier gives each bone channel of each animation one class:

| Class | Condition | Processor |
|---|---|---|
| `Constant` | the value is `Constant` or `Load` | baked into the model table |
| `Curve` | keyframes only, with time from the clock of the animation | GPU, from the curve table; CPU on the Minimal path |
| `Pure` | an expression over the pose inputs (section 10.2), with no write, no string, no random call, no loop, no entity switch and no domain risk (section 10.3) | GPU, lowered WGSL; the CPU batch VM on the Minimal path |
| `State` | anything else | CPU batch VM; the result goes to the GPU as a scalar pose input |

The class of a channel does not change after load. The specifications do not fix the processor of
a channel. They fix only the result: `docs/spec/entity-animation.md`, EANIM-GPU-003, gives the
one pose tolerance that every path must meet.

A variable that a `Pure` channel reads, and that `pre_animation` writes with a pure expression,
is inlined into the channel. Any other variable is a scalar pose input.

### 10.2 Pose inputs

A pose input is a value that a GPU channel reads for one entity. Each entity query has a pose
input kind in the column "Pose input" of `docs/spec/molang-queries.md` (MOLQ-HOST-006): `time`,
`frame`, `tick`, `tick_pt`, `lerp`, `alerp`, `walk` or `—`. The kind selects where the value
comes from:

| Kind | Source | Input slot |
|---|---|---|
| `lerp`, `alerp`, `walk` | the tick record of `docs/entity-render.md`, section 4.1: body yaw, head yaw, head pitch, walk position and speed. The record holds the current values. The GPU keeps the previous record and interpolates the two with the partial tick | none |
| `time` | the entity clock `C` of `docs/entity-render.md`, section 4.2, and the start values of the state record and the clip slots, which are values of `C` | none |
| `frame` | the frame uniforms | none |
| `tick`, `tick_pt` | the input record | one `f32` |
| `—` | not a pose input; a channel that reads it is `State` | — |

The input record of `docs/entity-render.md`, section 4.5, holds only two kinds of value, one
`f32` each:

- a query of the kind `tick` or `tick_pt` that a `Pure` channel reads;
- the result of a `State` channel.

The rules for the input record:

- An entity type has at most 16 input slots. An `f16` cannot hold a time or a distance within
  the pose tolerance, so each slot is an `f32`.
- A client entity definition that needs more than 16 slots does not use the GPU pose: the CPU
  computes its whole pose on every path, as on the Minimal path. The loader logs this one time
  for each definition.
- The CPU batch VM computes the `State` results in each frame for each visible entity, with the
  skip of `docs/spec/client-entity-definition.md`, CENT-SCR-006. It writes a slot in each frame
  in which the value changes. The GPU does not interpolate a slot.
- A definition with no `State` channel and no `Pure` channel that reads a `tick` or `tick_pt`
  query, and with no animation controller, has no input record. The pig is such a definition.

### 10.3 Domain risk and division

The lowering follows `docs/spec/molang.md`, MOL-NUM-005:

- A division lowers to `select(a / b, 0.0, b == 0.0)`. WGSL does not guarantee a result for a
  division by zero, so the test of the divisor is explicit.
- An interval pass computes a range for each IR value from the ranges of the pose inputs and the
  constants. A pose input with no known range has the range of all finite values.
- A call of `math.sqrt`, `math.ln`, `math.pow`, `math.exp`, `math.asin` or `math.acos` whose
  argument range leaves the finite domain of the function is a domain risk. The channel is
  `State`, and the CPU computes it.
- The CPU writes `0.0` in place of a non-finite pose input.
- The pose pass clamps each sampled channel value to `[−1e6, 1e6]`. A NaN on the GPU gives a
  finite value that is not specified. Thus no non-finite value reaches a vertex on any path.

### 10.4 Lowering

`Expression::classify` gives the class of section 10.1. The host gives the range of each query
that is a pose input. For a `Pure` channel, the class holds a `PureProgram`: the bytecode with
number registers only, and a list of inputs (`this`, a query slot or a variable slot).

`mcrs_molang_wgsl::lower` writes one WGSL function for one `PureProgram`:

```
fn <name>(opaque_zero: u32, i0: f32, i1: f32, …) -> f32
```

- The caller binds each input to a field of the records of `docs/entity-render.md`, section 4,
  or to a value that the pose pass computes from them (section 10.2).
- The function returns the channel value limited to `[-1e6, 1e6]`, and `0.0` for a NaN. The
  pose pass needs no other clamp.
- `opaque_zero` must be a zero that the shader compiler cannot see, for example a field of the
  frame uniforms. Metal compiles with fast math: it contracts `a * b + c` to an FMA, folds and
  reorders constants, drops the sign of zero and flushes subnormal values. Each operand and each
  result of a float operation goes through an integer operation with this zero, so the compiler
  has nothing to rewrite.
- The functions call `mcrs_molang_wgsl::LIBRARY`. A module holds it once.

The CPU twin (`Twin` of the `mcrs_molang_wgsl` tests) runs a `PureProgram` with the algorithms of the library,
operation for operation. Its result has the bits of the GPU result. Each float result is
flushed to zero when it is subnormal, in the twin and on the GPU. The twin is the reference of
the GPU parity tests, and only of those tests. The scalar VM stays the reference of the
language. The Minimal path evaluates pure channels with the batch VM, which has the bits of the
scalar VM; it agrees with the GPU paths within EANIM-GPU-003. A single set of bits for all
paths would need the batch VM to use the slower algorithms of the library, and no consumer
needs it. The scalar and batch VMs do not flush subnormal values.

| Function | Algorithm in the library | Bits against the scalar VM |
|---|---|---|
| `+`, `-`, `*`, comparisons, logic, `abs`, `floor`, `ceil`, `trunc`, `round`, `sign`, `copy_sign`, `min`, `max`, `clamp` | the IEEE operation | equal |
| `/` | integer long division, round to nearest even; `0.0` for a zero divisor | equal |
| `sqrt` | integer digit-by-digit root, round to nearest | equal |
| `mod` | integer remainder, as `fmodf` | equal |
| `lerp`, `inverse_lerp`, `min_angle`, `lerprotate`, `hermite_blend`, `in_range`, `approx_eq`, easing `quad` to `quint`, `circ`, `back`, `bounce` | the formulas of MOL-MATH-002 and MOL-MATH-003 over the operations above | equal |
| `sin`, `cos` | the exact reduction of MOL-NUM-004, quadrant, Taylor polynomials to degree 9 and 10 | not equal; error less than `1e-7` |
| `exp`, `ln`, `pow` | reduction by `ln 2`, polynomial; `pow` is `exp(e · ln b)` | not equal; about 1 step, `pow` about 6 steps |
| `atan`, `atan2`, `asin`, `acos` | reduction to `[0, tan(π/8)]`, Taylor polynomial; `asin` and `acos` through `atan2` | not equal; about 2 steps |
| easing `sine`, `expo`, `elastic` | the functions above | not equal |

"Equal" excludes subnormal results, which the GPU flushes to zero. The parity test of
EANIM-GPU-003 compares the scalar VM and the GPU within the pose tolerance.

## 11. Tests

The tests catch regressions with independent oracles. Count is not a goal.

1. **Corpus gate.** `tools/molang-corpus` writes the occurrences of the pinned Bedrock vanilla
   packs to `.corpus/molang.jsonl`, outside the repository (`docs/molang-corpus.md`,
   section 12). The parser must parse 100% of the lines with the form `string`,
   `script_array` or `versioned_object`. A line with the form `number` or `bool` is a constant
   and needs no parse. The gate counts parse errors only. A load error of a later milestone
   (`unsupported`) does not fail the gate.
2. **Differential test against molangjs.** `tools/molang-diff` runs molangjs, the Molang
   implementation of Blockbench. For each generated case, compare the outputs and the final
   variables. The harness skips or normalises each known difference of
   `tools/molang-diff/README.md`, section 3. Each other difference is a finding that a person
   checks against `docs/spec/molang.md`.
3. **Batch equals scalar.** Run N lanes through the batch VM and each lane through the scalar VM.
   The outputs and the variables must be equal bit for bit.
4. **Passes on equal passes off.** Run each program with all passes and with no pass. The results
   must be equal bit for bit.
5. **WGSL equals CPU twin.** Run the lowered channels in a compute shader on a headless device.
   The results must have the bits of the CPU twin, and must agree with the scalar VM within the
   pose tolerance (`crates/mcrs_molang_wgsl/tests/gpu_fuzz.rs`).
6. **Fuzzing.** A grammar-based generator feeds checks 3, 4 and the parser round trip. A round
   trip alone does not replace check 2.
7. **Unit cases** for rules that the corpus does not use: the loop limit, `break`, `continue`,
   `->` with an invalid reference, and `??` over a nested missing value.
8. **Benchmarks** (criterion): `math.cos(q.anim_time*K)*c`, `q.is_baby ? a : b`, a 10-statement
   `pre_animation`, and an animation with nine channels, each at 1, 64 and 256 lanes.

## 12. Migration of block conditions

Block definitions keep their JSON. Only the evaluator changes:

1. `mcrs_minecraft_block` declares a block host. Its query slots are the properties of the block
   (section 5.3).
2. For each permutation, the loader compiles the condition against the property table of the
   block. All values are `Load` values.
3. The loader runs the program once for each block state, and stores the result as a bit set of
   states.
4. No program and no expression survives into a tick, the same as today.
5. `crates/mcrs_minecraft_block/src/definition/molang.rs` and its error type go away. The
   compile errors keep their meaning: unknown property, and a value that is not in the domain
   of the property.

The block corpus uses only `q.block_state('<name>') == <literal>` with `&&` (53,431 uses). The
literal is a string, `true`, `false` or an integer. The block host gives:

- a boolean property as `Num` 0 or 1;
- an integer property as `Num`;
- any other property as `Str`.

A comparison between a `Str` and a `Num` is a compile error for a block host. The corpus
must compile with no change, and the set of states that match each permutation must stay
the same. A test compares the bit sets before and after the change.
