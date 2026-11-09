# Entity rendering on the GPU

This note describes how mcrs draws mobs in the deferred renderer of `mcrs_minecraft_render`.
The target is 100,000 mobs and more in one world, with no frame drop. The behaviour of a mob
(pose, overlay, shading) is stated in `docs/spec/`. The Molang side is in `docs/molang.md`.
The survey behind this design is `.research/entity-render-perf.md`. The workload model, the
measurements and the numbers behind each size and threshold of this note are in
`docs/entity-render-budget.md` (the budget note).

## 1. Principles

1. The ECS holds the state of each mob. The GPU holds a projection of that state in a
   persistent slot.
2. The CPU writes a slot only when a value changes: a packet, a tick of the client, or a change
   of a script input. No CPU system visits all mobs in each frame. The CPU evaluates the
   state-dependent Molang of a visible mob in a frame only when an input of that evaluation
   changed (`docs/spec/client-entity-definition.md`, CENT-SCR-006).
3. The GPU interpolates between ticks, culls, computes poses, selects faces and draws. It does
   this work only for visible mobs. On a backend with no compute shaders, the CPU does the cull
   and the pose (section 6).
4. One data model serves all mobs and all backends: the model tables, the slot records and the
   compiled channels. The data of a model selects what the path does.
5. The renderer selects an execution path from the capabilities of the adapter at startup, not
   from the name of the platform. Each path uses the best features that the adapter has. No
   path depends on bindless textures or push constants.

## 2. Why the CPU does not upload poses

| Data per frame | 200,000 mobs, 10 bones |
|---|---|
| one `mat4` per bone | 128 MB, or 7.7 GB/s at 60 FPS |
| one 16-byte transform per bone | 32 MB |
| tick records of moving mobs only (section 4) | 6.4 MB per tick, and only when all mobs move |

Thus the CPU uploads the inputs of a pose, not the pose. The GPU computes poses, and only for
visible mobs.

## 3. Static data

The loader writes these buffers once per pack load. A hot reload rewrites them.

A model is one baked geometry. A client entity definition can draw several models at the same
time, one for each layer of its render controllers (for example the pig body and the saddle).

| Buffer | Record | Content |
|---|---|---|
| `definitions` | 32 B per client entity definition | bone palette size, first palette entry, pose program id, first curve, input count, maximum active bone count |
| `models` | 64 B per model | bone count, first bone, first cuboid, cuboid count, bound sphere, texture size |
| `bones` | 48 B per bone | parent index (parents come before children), palette index, pivot, rest rotation, rest scale |
| `palette_bones` | 16 B per palette entry | channel classes and first channel of the bone in the pose program |
| `cuboids` | 32 B per cuboid | bone index, origin, size, inflate, UV origin, mirror flag, face mask; or a reference to explicit faces |
| `explicit_faces` | 64 B per face | one face of a cube that the 32 B record cannot hold; it shares the buffer of `cuboids` |
| `curves` | variable | keyframe tables: time, `pre` and `post` value, interpolation mode |
| `variants` | 16 B per variant | texture array, layer in the array, UV scale, first byte of the cutout mask (section 5.3) |

Encodings of the model records:

- **Bound sphere** of a model: the sphere around the visible bounds of the model
  (`docs/spec/entity-geometry.md`, EGEO-BAKE-002): its centre is the centre of the box, and its
  radius is half the diagonal of the box. The unit is the block, in the space of the model
  before the placement transform. The loader uses it to compute the cull radius of the cull
  record (section 4.3) with the scale of the entity.
- **Rest rotation** of a bone: three `f32` values in degrees, in internal space, in the Euler
  order of EGEO-POSE-001. Animation channels are in degrees too (`entity-animation.md`), so the
  pose pass adds them and converts the sum to radians one time for each bone. The degree value
  also keeps the file value of `this` exact.
- **Cuboid record**, 32 B: origin and size before inflate (`3 × f32` each), the box-UV origin
  in whole texels (`2 × i16`), the inflate as `i16` in steps of `1/8192` block (range ±4), the
  bone index (`u8`), and flags (`u8`: bits 0–5 face mask in the face order E, N, W, S, U, D,
  bit 6 mirror, bit 7 explicit).
- **Explicit cuboid**: a cube with its own rotation, a cube with per-face UV, a cube with a
  box UV origin that is not a whole texel, or a cube whose box UV origin or inflate is out of
  the range of its `i16` field, cannot use the 32 B form. Its cuboid record sets
  bit 7. Then the first 4 B of the record are the index of its first explicit face, and the
  face mask gives the faces that it has, in face order. The explicit faces of a model follow
  its last cuboid record: the face `k` takes the 32 B slots `2k` and `2k + 1` after it. The other fields of the record are not
  used.
- **Explicit face**, 64 B: corner `c₀` (`3 × f32`), the edge vectors `e₁` and `e₂`
  (`3 × f32` each), the texture coordinate of `c₀` and the texture edge vectors `t₁` and `t₂`
  (`2 × f32` each, normalised by the texture size), the bone index (`u8`), a flag byte (bit 0
  mirror) and 2 B of padding. The loader bakes the cube rotation into the corners, in the rest
  space of the bone. The four corners `c₀`, `c₀ + e₁`, `c₀ + e₁ + e₂` and `c₀ + e₂` are the
  corners `c0`, `c2`, `c3` and `c1` of EGEO-UV-005. The normal is `normalize(e₁ × e₂)`.

The pig and the frog use only the 32 B form. Block-entity models and Bedrock packs use the
explicit form.

The bone palette of a client entity definition is the union of the bone names of all its
models. The loader builds it, and maps each bone of each model to a palette index. The animations
of the definition write to palette entries, so all models of one entity share one pose by bone
name (`docs/spec/client-entity-definition.md`, CENT-RC-001). A palette entry that no animation
writes gives each model the rest pose of its own bone. The rest pose stays in the model, because
two models with one bone name can have different rest poses: the adult and the baby pig body.

Limits per model: 128 bones, a depth of 16 and 2048 cuboids (EGEO-LIM-001). The loader rejects a
model over a limit with a diagnostic.

The entity textures go into `texture_2d_array` layers. The model crate already pads entity
textures to square layers (`crates/mcrs_minecraft_model/src/atlas.rs`). The layer count of one
array comes from the adapter: `min(2048, max_texture_array_layers)`. WebGL2 guarantees only 256.
Thus the constant `ARRAY_LAYERS = 2048` of `atlas.rs` becomes a value that the renderer gives
at startup, and the entity textures split across several arrays when one array is full. A
`variants` record names its array. The draw groups by array (section 5.4).

## 4. Instance slots

Each mob has one slot in a paged arena. A page has 64 slots and a 64-bit validity mask.
The slot index lives in a component of the mob. Component hooks allocate the slot when the
model component arrives and free the slot when it goes.

A slot has a tick record, a cull record and a state record, and can have an input record. The
CPU writes a record only when one of its fields changes. The tick systems write each record as
one `#[repr(C)]` `Pod` struct into the CPU mirror of its page. They do not build a record from
byte slices: a struct store costs 2.1–2.8 ns per mob, and byte packing costs 24–30 ns (budget
note, section 4.12). On native, the tick systems split the work by page across the compute task
pool when more than 50,000 mobs move in one tick.

### 4.1 Tick record, 32 B

The CPU writes only the current values, in the tick that changes them.

| Offset | Size | Field |
|---|---|---|
| 0 | 12 | block position of the current tick, `vec3<i32>` |
| 12 | 6 | fraction inside the block, `u16 ×3`, in 1/65536 of a block |
| 18 | 2 | body yaw, `u16`, in 1/65536 of a turn |
| 20 | 2 | head yaw, `u16` |
| 22 | 2 | pitch, `snorm16` |
| 24 | 4 | walk position, `f32` |
| 28 | 4 | walk speed, `f32` |

Each page also has an array of 64 `u32` values: the client tick of the tick record of each slot.

The GPU keeps the previous values. It holds a second array of tick records, the previous
records, with the same layout. The scatter pass of section 4.7 writes each dirty tick record in
two steps: it copies the current record of the slot to the previous record, then it writes the
new current record and its client tick. The CPU never uploads a previous value. The 64 B record
with both values sent each value twice: 7.7 MB against 3.8 MB per tick for 120,000 moving mobs
(budget note, section 4.10).

The tick record backs these queries of `docs/spec/molang-queries.md`: the body yaw, the head yaw,
the head pitch, and the walk position and speed. A channel that reads them reads the tick record.
The GPU interpolates the previous and current records with the partial tick, as MOLQ-HOST-006
states for the kinds `lerp`, `alerp` and `walk`. These queries use no input slot (section 4.5).
When the client tick of the record is older than the previous tick of the client, the previous
values equal the current values.

An integer block position keeps full precision far from the origin. The shader subtracts the
camera section of `CameraOrigin` (`crates/mcrs_minecraft_render/src/frame.rs`) in `i32` first,
then converts to `f32` and adds the fraction. The pose and the bone matrices stay in internal
space of the entity. The vertex shader adds the entity transform relative to the camera, the same
as the terrain does. At 10⁵ blocks, the step of an `f32` world position is 0.125 model unit,
which is 125 times the position tolerance of EANIM-GPU-003 (budget note, section 4.7). A 16-bit angle has a step of
0.0055 degree, which is inside the pose tolerance of `docs/spec/entity-animation.md`,
EANIM-GPU-003. The walk values use `f32`, because a 16-bit float has a step of about `1e-3` near
1, and the leg swing multiplies the walk speed by 80 degrees.

The walk position grows by up to 1 in each tick of a moving mob. After about 10⁶ ticks of
movement, the step of an `f32` is more than 0.06, and the leg phase becomes coarse. This is a
known limit of the layout.

### 4.2 Entity clock

Each mob has one clock, `C = age + pt`. The age counts only the ticks that the entity ran, and
`pt` is the partial tick of the frame. The hurt start, the death start and the start of each clip
are values of `C`. The `time` pose inputs of `docs/molang.md`, section 10.2, come from `C`.

The GPU computes `C` with no upload in each tick. The state record holds a clock offset `O`, the
client tick at which the age was `0`. While the entity ticks, `C = T − O + pt`, where `T` is the
client tick of the frame uniforms. The state record has no flag that stops the clock. When the
entity stops (the entity is outside the ticking area, or the tick rate freezes it), the CPU writes
no new offset, and `C` continues to follow `T`.

### 4.3 Cull record, 16 B

The entity cull reads only this record for a slot that it rejects.

| Offset | Size | Field |
|---|---|---|
| 0 | 12 | block position of the current tick, `vec3<i32>` |
| 12 | 2 | cull radius, `f16`, in blocks, rounded up (section 5.1) |
| 14 | 2 | flags, `u16`: valid, full rig only |

The CPU writes the cull record when the block position, the radius or a flag changes. The flag
`full rig only` is set while the entity dies or is upside down
(`docs/spec/client-entity-render.md`, CEREN-VIS-003, item 7). The cull needs no fraction, no
previous position and no partial tick. For 300,000 slots on the web tier, the cull reads 4.8 MB
in 0.19 ms with this record, against 29 MB in 1.15 ms with the full tick record (budget note,
section 4.11). A mob that walks crosses a block boundary every 5 to 10 ticks, so the cull
record adds about 2–3 B per moving mob and tick to the upload.

### 4.4 State record, 48 B

The CPU writes the state record when the state changes. This occurs less often than once per
tick.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | client entity definition `u16`, active layer mask `u8`, spare `u8` |
| 4 | 16 | four layers: model `u16` and variant `u16` for each layer |
| 20 | 4 | flags: baby and dying in the low half, the hidden palette entries 0 to 15 in the high half |
| 24 | 4 | hurt start, a value of `C`, `f32` |
| 28 | 4 | death start, a value of `C`, `f32` |
| 32 | 4 | block light `u8`, sky light `u8`, spare `u16` |
| 36 | 4 | slot of the section in the section table |
| 40 | 4 | clock offset `O`, `i32`; the fixed age when the clock is stopped |
| 44 | 4 | index of the input record, or `u32::MAX` |

A layer is one draw of a render controller: one model with one texture. A mob has at most four
layers. The render controllers select the model and the texture of each layer, and the result
goes into the state record when it changes. A bit of the active layer mask that is clear means
that the layer draws nothing in this state: for example a pig with no saddle, or a texture
expression with the value `''`.

### 4.5 Input record, 96 B

The input record holds the values that the CPU computes and the GPU channels read:

- up to sixteen `f32` values (64 B). Each value is a query of the pose input kind `tick` or
  `tick_pt` that a `Pure` channel reads, or the result of a `State` channel
  (`docs/molang.md`, section 10.2);
- four clip slots, each with a clip id `u16`, a flags word `u16` and a start time `f32`, a value
  of `C` (32 B).

A mob gets an input record only when its client entity definition needs one: it has a `State`
channel, a `Pure` channel that reads a `tick` or `tick_pt` query, or an animation controller.
The pig needs none of these. Its channels read only the tick record, the state record and the
clock.

The CPU batch VM evaluates the `State` channels, the scripts, the animation controllers and the
render controllers in each frame, for each visible mob. It skips an evaluation when no input of
that evaluation changed, as `docs/spec/client-entity-definition.md`, CENT-SCR-006, allows. It
writes a value of the input record in each frame in which the value changes. The values are
those of the frame, so the GPU does not interpolate them.

An evaluation that depends on the number of frames (a variable that the script reads and writes,
a controller during a transition or a cross-fade) runs in each frame, also for a mob that is not
visible. For another mob that is not visible, the CPU can skip the evaluation. It brings the mob
up to date before the next draw of the mob, or before a CPU reader needs the values.

A controller writes the clip slots when its state changes. A cross-fade needs only the start
time and the duration, so the GPU computes the weight.

### 4.6 Light

The CPU samples block light and sky light at the eye of the mob at tick rate. It writes them to
the state record only when they change. The coloured light tint needs no data in the slot: the
lighting pass reads it from the light volume of `mcrs_minecraft_light_render` at the position of
the pixel, the same as for terrain.

### 4.7 Upload

The render world receives only the dirty pages and their bytes. It does not extract a component
of each mob. The upload uses the `Upload` and `Uploads` path of the render crate:

- The tick records always go through a scatter compute pass, because the pass rotates the
  previous record (section 4.1). The render world writes the dirty tick records and their slot
  indices to one staging region.
- With few dirty pages, the render world writes the cull, state and input records directly.
  With many, they go through the same scatter pass.

The upload volume is bounded by the number of mobs that change in a tick: 200,000 moving mobs
give 6.4 MB of tick records per tick. On the web tier, the scatter pass reads 64 B and writes
64 B for each moving mob: 6 MB and 0.24 ms for 47,000 mobs, in the frame of each tick (budget
note, section 4.10). Idle mobs cost no upload, because the clock needs no write. The state
records add 48 B for each mob with a changed state. The input records add up to 96 B for each
visible mob with a changed value in a frame.

## 5. Passes

The entity passes go into a new set `WorldPass::Entities`, after `WorldPass::OpaqueSecond` and
before `WorldPass::Lighting`. At that point, `draw_gbuffer_second` has rebuilt the depth pyramid
from the full depth of the current frame. Mobs therefore need no second culling phase. Mobs do
not write to the pyramid, because they are poor occluders.

```
Upload ─► Cull ─► Opaque ─► Occlusion ─► OpaqueSecond ─► Entities ─► Lighting ─► Forward
                                                         │
                     entity cull ─► pose ─► expand ─► entity gbuffer draw
```

### 5.1 Entity cull (compute)

One lane per slot. The dispatch strides over the pages with a capped grid, the same as
`cull.wgsl`. Entity workgroups have at most 128 invocations, the limit of WebGPU compatibility
mode. A lane reads the cull record and:

1. drops a slot with a clear `valid` flag;
2. reads the visibility bit of the section of the mob from the frame bitset of `cave_visible`;
3. tests the cull sphere against the frustum. The centre of the sphere is the centre of the
   block of the cull record. The radius is the cull radius plus `√3 / 2` block;
4. tests the screen rectangle of the sphere against the depth pyramid;
5. computes the projected height `h_px = h · H / (2 · d · tan(fov_y / 2))` of the entity box,
   with `h` the box height of the entity type times the baby scale and the scale of the state
   record, `H` the viewport height in pixels and `d` the distance of the centre of the sphere from
   the camera. The cull sphere is only for the cull: its radius also covers the movement margin
   and the visible bounds, so it is too large for the tier;
6. selects the tier. The mob is a full rig when `h_px` is at least the height threshold of the
   frame uniforms, when the flag `full rig only` is set, or when a layer has no image set (section
   5.6). Else it is an impostor (CEREN-VIS-003);
7. for a full rig, reads the active layers of the state record and reserves its bone matrices:
   the sum of the bone counts of the models of its active layers. The workgroup sums its
   reservations with a scan and does one `atomicAdd` on the bone arena. A reservation that
   passes the end of the arena or of the visible list makes the mob an impostor when its layers
   have image sets. Else the mob draws nothing;
8. appends the slot index and the first bone matrix of a full rig to the visible list of its
   client entity definition, and the slot index of an impostor to the impostor list, each with
   a scan inside the workgroup.

The pass also writes the dispatch arguments of the next two passes, and the counts of full rigs
and impostors to a readback buffer.

The height threshold starts at 12 px. The CPU reads the full-rig count back a few frames late,
from a ring of mapped copies of the counters. When the count is more than the full-rig cap of
section 6.1, the CPU raises the threshold by the factor `sqrt(count / cap)`, because the
full-rig count falls with the square of the threshold. When the count is less than 0.8 of the
cap, the CPU lowers the threshold by the same rule, and never below 12 px. The threshold stops
at 16384 px, because mobs with the flag `full rig only` can keep the count over the cap.

The CPU computes the cull radius when a layer, the scale, the script scale or the upside-down
flag changes. The radius covers the culling volume of `docs/spec/client-entity-render.md`,
CEREN-VIS-002: the union of the entity box and the visible bounds of the models of all active
layers. A sphere about the entity origin does not change with a rotation, so it also covers the
death tilt and the upside-down rotation. The radius is the largest distance of a corner of the
union from the entity origin, times the largest axis of the script scale, times the scale, plus
the upside-down offset `(h + 0.1)` when the flag is set. The `√3 / 2` of item 3 covers the
distance of the entity position from the centre of its block. The radius also grows by the
largest step of the mob in the last 20 ticks, rounded up to 0.25 block, because the frame
position interpolates from the previous position. The CPU rewrites the cull record only when that
rounded value changes.

The CPU sets the flag `full rig only` while the entity dies or is upside down.

### 5.2 Pose (compute)

One lane per full rig, grouped by client entity definition. Each definition with full rigs gets
one indirect dispatch of its own pose pipeline. An impostor gets no pose. The pose pipeline contains the WGSL
function that `mcrs_molang_wgsl` generates for the definition. A lane:

1. interpolates the previous and current tick records with the partial tick, computes the
   clock `C` (section 4.2),
   and reads the input record when the mob has one;
2. computes the time and motion inputs of the spec;
3. evaluates the `Curve` and `Pure` channels, and reads the `State` channels from the input
   record;
4. clamps each sampled channel value to `[−1e6, 1e6]`. WGSL does not guarantee NaN and ∞, so a
   NaN gives a finite value that is not specified, and no non-finite value reaches a vertex;
5. adds the channels of the active animations in the order of the client entity, and keeps one
   animated offset (rotation, position, scale) for each palette entry;
6. for each active layer, applies the offsets to the rest pose of each bone of the model of the
   layer, by the palette index of the bone, and composes the bone hierarchy of that model;
7. writes one `mat3x4<f32>` per bone of each active layer (48 B) to the `bone_matrices` buffer,
   from the first bone matrix that the cull reserved.

The bone arena has the size of the full-rig cap, not of the slot count: the cap times the
largest bone count of a loaded definition with all its layers. For the frog at the native cap,
that is `75,000 · 16 · 48 B = 58 MB`. With no cap, 300,000 frogs need 230 MB, which is more than
the guaranteed binding size of 128 MiB (budget note, section 4.2).

The gbuffer stores no motion today (`gbuffer()` writes zero motion). Thus the pass computes only
the current pose, and the entity draw writes zero motion, the same as terrain. The bone storage
(the `bone_matrices` buffer, or the bone texture of section 6) has a size and an index scheme
that allow two copies, current and previous, that swap in each frame. A later change that adds
motion vectors adds the second copy and changes no record layout.

### 5.3 Expand (compute)

One lane per visible mob, active layer and cuboid of the model of that layer. A lane selects the
faces of the cuboid:

1. A cuboid with no cutout texel emits the faces that face the camera, at most three.
2. A cuboid that has a cutout texel (alpha less than 0.1) in one of its face rectangles emits all
   its faces with an area. The camera sees the inner side of a back face through the hole in a
   front face (CEREN-SURF-001).
3. A cuboid with zero thickness on one axis emits the face of the pair that CEREN-SURF-001,
   item 4, selects. When the selected face has a cutout texel, the lane emits the other face of
   the pair first, then the selected face.

A lane of an explicit cuboid reads the normal of each explicit face from its edge vectors,
and applies the same three rules. Its face records name the face by its position in the face
order, as for a 32 B cuboid.

The cutout mask depends on the texture. The loader computes it when it loads a texture: one byte
for each pair of texture and cuboid, with one bit for each face. The `variants` record points to
the first byte of its texture.

A lane writes its face records in the order of the rules above. The workgroup reserves the face
records of all its lanes with one scan inside the workgroup and one `atomicAdd` for each texture
array that its lanes use. The records of one lane stay contiguous and in order. On a GPU that
serializes atomics on one address, one atomic per lane would take about 1.4 ms for 2.1 M lanes
(budget note, section 4.13). The rasterizer keeps the order of the primitives of one draw. With the depth
compare `GreaterEqual`, the later face of a coplanar pair is visible where both faces pass the
cutout test. The lane appends one face record per face:

| Field | Size |
|---|---|
| visible index | `u32` |
| cuboid index inside the model of the layer | `u16` |
| layer index | `u8` |
| face | `u8` |

The layer index selects the model and the variant of the state record. Thus a face record is
8 B for each layer. The lane appends the record to the face list of the texture array of the
variant. It adds six vertices per face to the vertex count of the draw arguments of that array,
with the same aggregation per workgroup.

### 5.4 Draw

Two `draw_indirect` per texture array into the gbuffer targets, with `LoadOp::Load` and depth
test and write. The expand pass writes the face records with no cutout texel from the start of
the range of the array, and the others from its end. The first draws have no discard, so a
tile-based GPU removes their hidden fragments; the draws of the cutout faces follow. A draw has
one instance of a strip of four vertices for each face record: on the reference machine this
costs 0.11 ms of vertex time in S1, against 0.20 ms for six vertices of a triangle list.

- The vertex shader pulls the face record, then the layer of the state record, the cuboid, the
  bone matrix and the corner. It computes the position and the UV from these. For an explicit
  cuboid, it reads the explicit face (section 3) and takes the corner and the UV from it.
- The fragment shader samples the texture array of the variant and discards a texel under the
  cutout alpha of the spec.
- The fragment shader computes the colour of the entity in the order of
  `docs/spec/client-entity-render.md`, CEREN-SURF-006, up to the light (section 5.5).
- The fragment shader writes `gbuffer(albedo, ao, normal, block, sky)` from `deferred.wgsl`,
  with that colour as `albedo`, `ao = 1`, the real normal of the face, and the block and sky
  light of the state record.
- For a face that the camera sees from the back, the fragment shader flips the normal
  (`front_facing`), as CEREN-SURF-002, item 3, states. The back faces of a cuboid with a cutout
  texel and the faces of a zero-thickness plane are such faces.
- The block and sky light are the levels of the eye. The coloured light tint comes from the light
  volume at the pixel, in the lighting pass (CEREN-SURF-003, item 4, and CEREN-DEV-002).

### 5.5 Shading of entities

The channels `light.x` (emissive) and `light.w` (roughness) of the light target are reserved
(`crates/mcrs_minecraft_render/src/gbuffer.rs`). The entity pass does not use them for shading
data. Thus the fragment shader of the entity draw computes, in the order of CEREN-SURF-006:

1. `c` = the texel times the tint of the layer;
2. `f` = the two-light factor of CEREN-SURF-002 from the normal of the face, flipped for a back
   face;
3. `c = c · f`;
4. with the red overlay or the white flash of CEREN-SURF-004 and CEREN-SURF-005, with colour `o`
   and alpha `a`: `c = mix(o, c, a)`. The GPU computes the overlay from the hurt start, the
   death start and `C`.

The fragment shader writes `c` as the albedo. The lighting pass then multiplies it by the
lightmap and the light tint, the same as for terrain, with no change to the lighting pass. The
overlay goes after the face factor and before the light, as CEREN-SURF-006 requires.

The gbuffer still holds the real normal of the face. A later lighting model (shadows, point
lights, shader packs) can use it. Such a model then moves the face factor out of the albedo.

An emissive layer (for example eyes) writes `light.x = 1`, the emissive value of the gbuffer. A
variant of the `deferred.wgsl` helper takes the emissive value; its old form writes `0`.

The first pipeline is opaque cutout. Translucent layers later go into `WorldPass::Forward` and
reuse the face records.

### 5.6 Impostors

An image set holds the views of one model with one texture: 16 yaws and the 9 elevations 0°,
5°, 10°, 15°, 20°, 30°, 40°, 50° and 60°, at rest. A view is a tile of 64 × 64 texels. An image
of the wrong elevation shows more or less of the top faces, which are brighter than the sides.
With the elevations 0°, 30° and 60° of CEREN-VIS-003, a camera 5° to 15° above a far field sees
the 0° images: the impostors are up to 8% darker than the full rigs. With steps of 10°, a field
seen from 9 blocks above shows bands from 11% darker to 7% brighter in the final image. The
steps of 5° up to 20° keep each band within 5%. A set takes 4.7 MB. The set is one layer of two
`texture_2d_array` targets: the colour after the tint, and the normal in internal space with the
depth toward the camera. The arrays start with 8 layers and double up to
`min(2048, max_texture_array_layers)`.

The loader bakes no set. The upload notes each pair of a model and a texture of an active layer
of a state record that it writes. Before the cull of that frame, a bake renders the set of each
new pair with the vertex stage of the full-rig draw and its face records: the bones at rest, one
instance for each view, and all faces of each cuboid with a depth test. A bake costs one render
pass. The render controllers can hide bones, so the set keeps the hidden mask of the first state
record that names the pair. A slot with another hidden mask draws its full rig. A table of
`u32`, one for each model and texture, gives the layer of the set and its hidden mask.

The quad of an impostor stands on the vertical plane through the entity position and faces the
camera. A view image is the projection of the model onto that plane along the direction of the
view, so the quad shows each point where the camera sees it. Each model has a rectangle for each
view, and the quad covers the rectangles of the active layers. The models of one definition
share the depth scale. Thus the fragment shader takes, for each pixel, the covered texel of the
layers that is nearest to the camera, the later layer on a tie, and shades it with section 5.5.

A prepare pass after the expand pass has one lane for each impostor. It interpolates the position
and the body yaw of the tick records, selects the nearest view, and writes a draw record of
128 B: the position, the axis of the quad, its rectangle, the image sets and the rectangles of
the layers, the light and the overlay. The vertex stage of the impostor draw reads only the
record. In S1 this takes the vertex time of the impostors from 0.23 ms to 0.06 ms.

The impostor draws follow the face records in the same render pass: six vertices for each draw
record, with no instance. A view image holds the pixels of a cutout, so the fragment shader
discards the pixels that no layer covers. A fragment shader that discards prevents the removal
of hidden surfaces of a tile-based GPU: in S1 the discard costs about 0.16 ms of the 0.26 ms of
the impostor fragments.

An impostor under 3 px draws with no discard. At bake, a compute pass measures the share of the
covered texels of each view image and their centroid. The quad of a small impostor is the
rectangle with that share of the area of the view rectangle and its aspect, about the centroid.
A pixel that no layer covers takes the colour at the centroid. The small impostors take their
draw records from the end of the record buffer down. This takes 0.05 ms of fragment time in S1.
The FLIP error at 2 and 3 px is 0.07 to 0.19, against 0.02 to 0.05 with the discard.

The budget note, section 4.15, gives the measurements of each step and the choices that did not
help.

## 6. Execution paths

The renderer supports each backend that wgpu supports, and uses the strongest features of the
adapter. At startup it reads the features, the limits and the downlevel capabilities of the
adapter and selects one path. A test can force a weaker path on a strong adapter, so each path
runs in CI on native.

| Path | Selected when | Cull | Pose | Bone storage | Draw |
|---|---|---|---|---|---|
| Full | `DownlevelFlags::COMPUTE_SHADERS` and `DownlevelFlags::VERTEX_STORAGE`, and `max_storage_buffers_per_shader_stage ≥ 8` | compute (section 5.1) | compute (section 5.2) | `bone_matrices` storage buffer | vertex pulling from face records; `multi_draw_indirect_count` with `Features::MULTI_DRAW_INDIRECT_COUNT`, else one `draw_indirect` per texture array |
| Compatibility | compute shaders, but no `VERTEX_STORAGE` (WebGPU compatibility mode) | compute | compute | bone texture, `rgba32float`, written as a storage texture | the face records of the expand pass as a vertex buffer with the step mode `Instance`, six vertices per instance; the vertex stage reads the cuboids and the bones with `textureLoad` |
| Minimal | no `COMPUTE_SHADERS` (WebGL2, GLES 3.0) | CPU: section visibility and frustum, no depth pyramid | CPU batch VM | bone texture, `rgba32float`, uploaded with `write_texture` | instanced draw per model, the same as Compatibility |

The three paths share:

- the static data of section 3 and the slot records of section 4;
- the compiled channels: the WGSL of `mcrs_molang_wgsl` for the compute paths, and the CPU batch
  VM of `mcrs_molang` for the same channels on the Minimal path (`docs/molang.md`, section 10).
  The Minimal path does not use the CPU twin. Its poses agree with the GPU paths within
  EANIM-GPU-003, not bit for bit;
- the fragment shader and the shading of section 5.5;
- the tier selection of section 5.1. On the Minimal path, the CPU selects the tier with the same
  rule.

The vertex shader reads bones and instances through one fetch interface, `fetch_bone(slot,
bone)` and `fetch_instance(slot)`. A shader def selects its body: a storage buffer read for
Full, and a `textureLoad` for Compatibility and Minimal. The rest of the shader does not change.

In the Compatibility path, the expand pass writes the face records to a buffer with the usages
`STORAGE | VERTEX`. The draw of one texture array finds its part of the face list with the offset
of `set_vertex_buffer`, so it needs no `indirect-first-instance`. Integer varyings use
`@interpolate(flat, either)`, because compatibility mode has no other flat mode. An instanced
cuboid mesh would draw 168 vertices and 84 triangles for a pig, and the face records draw 126
vertices and 42 triangles. On the web tier, that halves the triangles: 0.38 ms less for 10,000
pigs (budget note, section 4.5).

In the Minimal path, the instance buffer of a model holds one record per full rig and active
layer that uses the model: the slot index, the layer index and the first texel of the bones of
that layer. The CPU writes it. A model with no visible instance gets no draw. The pose,
the layers, the clock and the shading of sections 4 and 5 are the same on all paths.

### 6.1 Limits and costs of each path

| Path | Full-rig cap | Main cost | Limit that binds |
|---|---|---|---|
| Full | 75,000 native, 20,000 web | GPU passes, section 8 | 128 MiB per storage binding: `bone_matrices` at the native cap holds `75,000 · 16 · 48 B = 58 MB` of frog bones |
| Compatibility | 20,000 | one more texture write per bone; the face records go through the vertex input instead of a storage read | `max_texture_dimension_2d`, 4096 in WebGPU compatibility mode: a bone texture of 4096 × 4096 `rgba32float` texels holds 5.6 M bones at 3 texels per bone, but uses 256 MiB. The texture has the size of the full-rig cap of `docs/entity-render-budget.md`, section 4.2: 1024 × 1024 texels (16 MiB) hold 20,000 frogs |
| Minimal | 2,000 | CPU pose: the batch VM evaluates every channel of every visible mob in each frame, and the CPU uploads the bones | upload per frame: 2,000 mobs × 10 bones × 48 B = 0.96 MB |

Impostors have no cap. The caps are the frog counts at which the draw of each tier reaches its
share of the frame, rounded down (budget note, sections 3 and 4.2).

In the Minimal path, the CPU limits the work with the visible set and with the frame budget of
section 8. In this path only, a full rig that is far from the camera can use a lower pose rate.

### 6.2 Limits of the web

| Limit | Value | Consequence |
|---|---|---|
| storage buffers per stage | 8 in WebGPU core | the Full vertex stage binds `faces`, `cuboids`, `bone_matrices`, `slots`, `variants`; the model tables share one buffer |
| storage binding size | 128 MiB | a larger `bone_matrices` splits by page range |
| texture array layers | 256 in WebGL2 | several entity texture arrays, one draw per array (section 3) |
| buffer size | 256 MiB | |
| multi-draw-indirect | the WebGPU backend loops single draws | one draw per pipeline, not per model |
| `first_instance` in indirect draws | `Features::INDIRECT_FIRST_INSTANCE`, optional | not used by Full; the instanced paths use direct draws |
| push constants | not in all browsers | uniforms with dynamic offsets |
| storage buffers in the vertex stage | none in WebGPU compatibility mode | the Compatibility path |
| compute shaders | none in WebGL2 | the Minimal path |

## 7. Exports from `mcrs_minecraft_render`

The entity crate (`mcrs_minecraft_entity_render`) needs these items, which are private today:

| Item | Today | Need |
|---|---|---|
| `DeferredFrame` and its `targets` | `pub(crate)`, `gbuffer.rs:42` | the gbuffer targets for the entity draw |
| `gbuffer_pass` | `pub(crate)`, `pass.rs:454` | a pass on the targets with `LoadOp::Load` |
| `GBUFFER_FORMATS` | `pub(crate)`, `gbuffer.rs:17` | the formats of the entity pipeline |
| the depth pyramid view and its sampler | inside `Terrain.hiz` | the occlusion test of the entity cull |
| the frame bitset of `cave_visible` | inside `Terrain` | the section test of the entity cull |
| `WorldPass::Entities` | does not exist | the slot of the entity passes |

The render crate exports a small read-only view of these items. It does not export the
`Terrain` internals.

## 8. Stress scenes and gates

The stress scenes S1 to S4 and their gates are in the budget note, section 5. A debug spawner
creates the client entities from a seed, so a scene needs no server. Each gate becomes a CI gate
when its scene and the entity passes exist. Only the gates of the reference machine run locally;
the other tiers need such an adapter in CI or a forced path.

Measure each compute pass with `PassTimestamps`: entity cull, pose and expand. Also record the
bytes uploaded per tick and the counts of slots, visible mobs, full rigs, impostors and emitted
faces. Write the results to `docs/perf/`.

- On Metal, the timestamps at the bounds of a render pass do not bracket the work of the pass.
  The draw time is the difference of two runs, with and without the entity draw, under
  `MCRS_GPU_HOT` (budget note, section 4.14).
- In Chrome, a gate reads the timestamps only with the flag
  `enable-webgpu-developer-features`, or it takes the mean of at least 64 frames, because
  Chrome quantizes a timestamp to 100 µs.

## 9. Order of work

1. Static data, the bone palette, slots with layers, the tick and state records, the clock, and
   one draw with poses at rest. Pigs stand in the benchmark scene, some with a saddle.
2. The cull record and the entity cull with sections, the frustum, the depth pyramid and the
   tier selection, with the height threshold and its feedback from the full-rig count.
3. The pose pass with `Curve` and `Pure` channels. Pigs walk and look around.
4. The expand pass and the compaction.
5. The impostor pipeline of CEREN-VIS-003: the image sets, the quad draw and the impostor list
   of the cull (section 5.6).
6. The input records, the CPU batch VM in each frame with the skip of CENT-SCR-006, and the
   controller clip slots. The frog uses them; the pig does not.
7. The Compatibility path: the bone texture, the fetch interface and the draw of face records
   as an instance-rate vertex buffer. CI forces this path on a native adapter.
8. The Minimal path: the CPU cull and the CPU pose with the batch VM. It is ready before the
   WebGL2 backend of the client, and CI forces it on a native adapter.

The shading of section 5.5, with the face factor and the overlay in the entity fragment shader,
is part of step 1.

## 10. Deferred

- A lower pose rate for far full rigs, on the Minimal path only.
- The previous pose and motion vectors (section 5.2 keeps room for them).
- Shadow views and extra views. They would reuse `bone_matrices` and redo only the cull and the expand.
- Sounds and particles from animation timelines. They run on the CPU.

## 11. Choices that the numbers reject

These choices look useful but cost more than they save at the sizes of the budget note. A change
that adds one of them brings new measurements first.

| Choice | Number against it (budget note) |
|---|---|
| a lower pose rate for far mobs on the GPU paths | the pose is 6.5% of a full-rig pig and 16% of a frog on the reference machine; a lower rate needs a persistent bone store of 230 MB for 300,000 slots (4.3) |
| a shared pose for mobs with identical state | about 0.07 ms of 1.62 ms in S1 with no tiers, and less than 0.005 ms with the tiers (4.4) |
| a second occlusion phase, or mobs in the depth pyramid | the cull tests the pyramid of the current frame, so it has no false negative to correct; a pyramid texel at the mip of a far pig is 0.8 block, and crowd gaps are 0.4 block, so mobs hide nothing; a second build costs 0.073 ms (4.6) |
| the previous bone palette for motion vectors | the renderer has no TAA and no plan for it; at the web cap it adds 15 MB of bone writes per frame, 0.6 ms (4.8) |
| new gbuffer targets for entities | 12 B of the 32 B per sample remain; the overlay needs no target (4.9) |
| a compute rasterizer for small faces | it needs 64-bit atomics, which exist only on native (4.1) |
| one atomic per lane in the expand pass | the same cost on the reference machine, and about 1.4 ms on a GPU that serializes atomics (4.13) |
