# Player rendering: crates, inputs and seams

This note states where the code that draws a player lives, and which crate owns each value that
the entity renderer reads. The behaviour is in `docs/spec/client-player-render.md`. The passes
are in `docs/entity-render.md`.

## 1. Crate map

The entity renderer is a crate of layer 6. It must not link the network, a client crate or the
play-state packets. A skin viewer takes the renderer and the crates below it, and no connection.

| Crate | Layer | Bevy | Workspace dependencies that matter here |
|---|---|---|---|
| `mcrs_minecraft_entity` | 3 | no | owns `Pose` |
| `mcrs_minecraft_profile` | 3 | no | owns `PlayerModelType` and `SkinTextures`, the values of a profile |
| `mcrs_minecraft_entity_visual` (new) | 4 | `bevy_ecs`, `bevy_app` | `mcrs_minecraft_entity`, `mcrs_minecraft_item`, `mcrs_minecraft_item_key`, `mcrs_minecraft_registries`, `mcrs_minecraft_resource`, `mcrs_minecraft_numeric`, `mcrs_voxel` |
| `mcrs_minecraft_entity_render` | 6 | yes | `mcrs_minecraft_entity_visual`, `mcrs_minecraft_entity`, `mcrs_minecraft_render`, the model and animation crates |
| `mcrs_minecraft_client_world` | 7 | yes | `mcrs_minecraft_entity_visual` (the ticking area), the network |
| `mcrs_minecraft_client_entity` | 7 | yes | `mcrs_minecraft_entity_visual`, `mcrs_minecraft_client_world`, `mcrs_minecraft_network`, `mcrs_minecraft_protocol_game` |
| `mcrs_minecraft_client` | 7 | yes | all of the above |

`mcrs_minecraft_entity_render` loses four dependencies: `mcrs_minecraft_client_entity`,
`mcrs_minecraft_client_world`, `mcrs_minecraft_protocol_game` and `mcrs_minecraft_profile`.
The `layers` check of `xtask` holds this state.

### 1.1 `mcrs_minecraft_entity_visual`

This crate holds the state of an entity that a viewer sees, the tick that advances that state,
and the values of one frame. It has no packet and no connection. These files move into it from
`mcrs_minecraft_client_entity`, with their names:

- `placement.rs`, `path.rs`, `interpolation.rs`, `motion.rs`, `angle.rs`;
- `living.rs`, `head_turn.rs`, `body_turn.rs`, `walk.rs`;
- `synced_data.rs`, without the two functions that read packet types;
- `equipment.rs`, `server_ticking.rs`, `render_inputs.rs`, `host_entity.rs`;
- `client_entity.rs`, without `NetworkEntities`;
- `tick.rs`, with `ClientEntityTick`.

The crate adds four public names:

- `TickingArea`: a trait of a resource that tells whether the entities of a column tick.
- `EntityTickPlugin<A: TickingArea>`: the plugin that ticks each entity in `ClientEntityTick`.
- `EntityLight`: a component with the block light and the sky light at the eye of an entity.
- `EntityLightSystems`: the set that holds the writer of `EntityLight`. The renderer reads the
  light after this set.

`mcrs_minecraft_client_entity` keeps the packet side: `receive.rs`, `delta.rs`,
`NetworkEntities`, the functions that apply entity data and attributes of a packet to
`SyncedData`, and the system that writes `EntityLight`. Its plugin adds
`EntityTickPlugin<ColumnStore>`.

## 2. Owner of each render input

Each input has one writer for one entity. The renderer reads and never writes.

| Input | Type | Crate | Writer |
|---|---|---|---|
| entity type | `ClientEntity::kind` | `entity_visual` | the code that adds the entity: the packet side or a host |
| placement of a tick | `Motion` | `entity_visual` | the tick; the packet side and `move_entity` set the target |
| age | `Age` | `entity_visual` | the tick |
| yaws, walk, hurt, death | `Living` | `entity_visual` | the tick; the packet side sets the targets |
| pose, baby, scale, variant, flags | `SyncedData` | `entity_visual` | the packet side, or a host |
| equipment | `Equipment` | `entity_visual` | the packet side, or a host |
| light at the eye | `EntityLight` | `entity_visual` | the client, from its columns, for each entity; an application with no column store sets the value |
| tick rate | `ServerTicking` | `entity_visual` | the packet side, or a host |
| arm model | `PlayerModel` | `entity_render` | the client, from the profile; or a host |
| skin image | `PlayerSkin` | `entity_render` | the client, from the download; or a host |
| client tick and partial tick | `EntityClock` | `entity_render` | the renderer |

`FrameValues` is derived state. `RenderInputs::frame` computes it on each read, and no
component stores it. An entity with no `EntityLight` has full sky light and no block light.
The light of an entity changes only when its eye enters another block, as before.

## 3. Decisions

- **The render inputs and `host_living_entity` live in `mcrs_minecraft_entity_visual`.** The
  renderer and the packet side then depend on one crate below both, and a viewer takes no
  connection.
- **`Pose` moves to `mcrs_minecraft_entity`.** A pose is game data, and the packet crate
  already depends on that crate.
- **The light of an entity is an input component, not a read of the column store.** A doll and
  a viewer have no world, and their light is a value that the host gives.
- **The tick is generic over `TickingArea`.** The tick needs only one fact of the world, and a
  host with no world gives a resource that holds each column.
- **`PlayerModel` becomes an enum `{ Wide, Slim }`.** The renderer names a model and not a
  profile, so it does not link `mcrs_minecraft_profile`. The client maps `PlayerModelType` to it.
- **The player has the bones `root` and `waist`; the body, the head and the arms are children
  of `waist`.** A turn of the body then does not tilt the axis of the head yaw, so the crouch
  agrees with the game. The animation file of the player does not change.
- **The limbs are not split, and there is no bend.** A lower bone can come later as a child of
  the limb bone with no change of a name or a pivot (CPR-MODEL-005). A closed seam needs faces
  that follow two bones, which the face records do not have.

No other public name of the player work changes. `PlayerSkin`, `SkinImage`, `SkinError`,
`SKIN_SIDE`, `SKIN_SLOTS`, `HostEntity`, `host_living_entity` and `move_entity` keep their names.
The last three change their crate.

### 3.1 Bones and third-party formats

The public player formats do not agree on the parents. One family has the head and the arms as
children of the body. The other family has flat bones below one bone for the full player, and
lets each animation file give its own parents and pivots. The names and the pivots agree, except
that the body box is `body` in one family and `torso` in the other.

Thus the model fixes the names and the pivots, and it keeps the parents that the game pose
needs. A pose layer brings its own parents (section 4.4).

## 4. Later consumers

None of these is built now. Each paragraph names the seam and states why the present types do
not block it.

### 4.1 Local player in third person

The local player gets the components of `mcrs_minecraft_entity_visual`, a `PlayerModel` and a
`PlayerSkin`. The movement prediction writes `Motion`, and the same tick advances the walk and
the yaws. The camera decides whether the entity draws. That needs a visibility value for each
entity and view, which goes into the spare bits of the cull flags. Section 6.2 adds that value.
No present type changes.

### 4.2 Player figure in the inventory GUI

Section 6 holds the design of the figure. The figure work adds the view mask of section 4.1 and
the figure view that section 4.3 needs.

### 4.3 Skin viewer or editor

The viewer takes `mcrs_minecraft_entity_render` and `mcrs_minecraft_entity_visual`. It adds the
entity with `host_living_entity`, sets `PlayerModel` and `PlayerSkin`, and adds
`EntityTickPlugin` with a ticking area that holds each column. An editor writes `PlayerSkin`
again after each change, and the next frame draws the new texels. The viewer needs the second
view of section 6.1 when it draws no terrain.

### 4.4 Pose layers for emotes

A pose layer is a component of one entity that names clips and their start times. The input
record has four clip slots for each entity today. A layer needs clips that load after the pack,
and a pose program that reads a clip slot for each bone of the palette. A layer of a format
with its own parents needs those parents as data of the clip. All of this is new code in the
pose program and the clip tables. The bone names and the pivots that a layer addresses are
final (CPR-MODEL-002, CPR-MODEL-005). `PlayerModel` selects the pivots of the model. `PlayerSkin`
and `host_living_entity` have no part in the pose. A bend adds lower bones and one channel, and
it changes no present name.

## 5. Order of work

Each step is one commit of one agent. A step starts when the step before it is green. The steps
1 to 5 touch the same crates and run in sequence. Step 6 touches only the model test and two
asset files.

1. **Move `Pose`.** Move the enum from `mcrs_minecraft_protocol_game` to
   `mcrs_minecraft_entity`, and change each import. Check:
   `cargo nextest run -p mcrs_minecraft_entity -p mcrs_minecraft_protocol_game -p mcrs_minecraft_client_entity`.
2. **Add `mcrs_minecraft_entity_visual`.** Move the files of section 1.1. Add `TickingArea` and
   `EntityTickPlugin`. Implement `TickingArea` for `ColumnStore` in
   `mcrs_minecraft_client_world`. Keep the packet functions of `SyncedData` in
   `mcrs_minecraft_client_entity` as free functions. Change the imports of
   `mcrs_minecraft_client_entity`, `mcrs_minecraft_entity_render` and `mcrs_minecraft_client`.
   Check:
   `cargo nextest run -p mcrs_minecraft_entity_visual -p mcrs_minecraft_client_entity -p mcrs_minecraft_entity_render -p mcrs_minecraft_client`.
3. **Make the light an input.** Add `EntityLight` to `mcrs_minecraft_entity_visual`. Write it in
   `mcrs_minecraft_client_entity` after the tick, for each entity whose eye enters another block.
   Read it in `write_ticks` of the renderer, and remove `mcrs_minecraft_client_world` from the
   renderer. Check:
   `cargo nextest run -p mcrs_minecraft_client_entity -p mcrs_minecraft_entity_render`.
4. **Make `PlayerModel` an enum.** Change `player_skin.rs` of the renderer and
   `player_skins.rs` of the client. Remove `mcrs_minecraft_profile`,
   `mcrs_minecraft_client_entity` and `mcrs_minecraft_protocol_game` from the manifest of the
   renderer. Check: `cargo nextest run -p mcrs_minecraft_entity_render -p mcrs_minecraft_client`.
5. **Hold the layers.** Add a rule to the `layers` check of `xtask`:
   `mcrs_minecraft_entity_render` and `mcrs_minecraft_entity_visual` link no
   `mcrs_minecraft_network`, `mcrs_minecraft_protocol_game`, `mcrs_minecraft_client`,
   `mcrs_minecraft_client_entity`, `mcrs_minecraft_client_world`, `mcrs_minecraft_server` or
   `mcrs_minecraft_engine_server`. Check: `cargo nextest run -p xtask`, and the `layers` check
   passes.
6. **Give the player its bones.** In `crates/mcrs_minecraft_entity_model/tests/it/mob_assets.rs`,
   generate the player geometry with the bones and the parents of CPR-MODEL-002, for the two
   model types. Write `assets/mcrs/entity/geometry/player.json` again. The animation file does
   not change. Add one case to the player test of the renderer: a turn of `waist` moves the
   head and leaves a leg. Check:
   `cargo nextest run -p mcrs_minecraft_entity_model -p mcrs_minecraft_entity_render`.
7. **Review and the full run.** One review of the branch in a new agent, then
   `cargo nextest run --workspace` before the push.

A step stops and reports when its check fails for a cause outside its files, or when the
specification does not answer a question.

### 5.1 Figure in the inventory screen

Section 6 is the design. Each step is one commit of one agent, and the steps run in sequence.
Each step leaves the workspace green. The steps 1 to 4 need no window.

1. **Let a host own the light of an entity.** Add `HostLight` to
   `mcrs_minecraft_entity_visual`. In `entity_light.rs` of `mcrs_minecraft_client_entity`, the
   sampler skips an entity with `HostLight`. Add one test: with a column store, an entity with
   `HostLight` keeps the light that its host gives. Check:
   `cargo nextest run -p mcrs_minecraft_entity_visual -p mcrs_minecraft_client_entity`.
2. **Add the view mask and the frame values.** In `mcrs_minecraft_entity_render`, add
   `EntityViews` and `FramePosed`. Put the view mask into the cull flags (`records.rs`,
   `main_world.rs`) and the view bit into `FrameUniform`. The cull of `compute.wgsl` drops a
   slot whose mask does not hold the view bit. `write_ticks` writes a fresh tick record for an
   entity with `FramePosed`. Correct the table of `docs/entity-render.md`, section 4.3. Add one
   case to the GPU tests of the crate: a slot with the mask `FIGURE` is not in a visible list of
   the world view. Check: `cargo nextest run -p mcrs_minecraft_entity_render`.
3. **Add the figure view.** In `mcrs_minecraft_entity_render`, add `FigureView`, `FigureFrame`,
   `FigureTargets`, `FigureDraw` and `FigureViewSystems`, the second set of frame buffers, the
   figure run of the compute passes and the two forward pipelines (sections 6.1 and 6.4). Add
   one GPU test: a player in the figure view, drawn into a test target with a parallel
   projection. The test reads the face factors of CPR-FIG-005 from the pixels, and it finds no
   face record of the player in the world view. Check:
   `cargo nextest run -p mcrs_minecraft_entity_render`.
4. **Compute the figure layout.** In `mcrs_minecraft_client_gui`, add `inventory_figure.rs` with
   the pure functions of section 6.3: the figure rectangle, the feet point, the eye point, the
   angles `a` and `t`, the projection and the two light directions. Add one table test with the
   mouse positions and the angles of CPR-TEST-003, and the points of the table of CPR-FIG-002.
   Check: `cargo nextest run -p mcrs_minecraft_client_gui`.
5. **Draw the figure.** In `mcrs_minecraft_client_gui`, add `InventoryFigure`,
   `inventory_figure`, the system that writes the inputs, `GuiQuad::Figure` and
   `GuiDrawKind::Figure`. `draw_gui` draws the figure in its band with the scissor of the figure
   rectangle (section 6.1). In `mcrs_minecraft_client`, spawn the figure with the local player
   in `native.rs` and `web.rs`. Add the order of the figure quad to the present test of
   `inventory_screen.rs`. Check:
   `cargo nextest run -p mcrs_minecraft_client_gui -p mcrs_minecraft_client`. This step needs a
   window: section 5.2.
6. **Review and the full run.** One review of the branch in a new agent, then
   `cargo nextest run --workspace` before the push.

### 5.2 Check of the figure in a window

No test of the default tier opens a window. The scene frames of
`crates/mcrs_minecraft_client/tests/scene_frames` (feature `scene`) have no step that opens a
screen, and the tool that drives the game cannot open a screen or take a screenshot
(CPR-TEST-003). Thus the agent of step 5 checks one frame of mcrs against the numbers of the
specification.

1. Start the client with the knobs of `crates/mcrs_minecraft_client/src/config.rs`:
   `RESOLUTION` for a window of 854 by 480 window pixels, `SCREEN=inventory`, `CURSOR=175,60`
   and `CAPTURE=<file>.png`.
2. Read the screenshot. The lowest row of the feet is at 217.75 window pixels (CPR-TEST-003).
   No window pixel outside the figure rectangle, (302, 90) to (400, 230), differs from a
   screenshot with no figure.
3. Do the capture again with each mouse position of the table of CPR-TEST-003. The head and the
   body turn to the side that the table gives.
4. Hold a stack over the figure rectangle. The stack covers the figure (CPR-FIG-006).

The agent reports each screenshot path and each number that it read.

## 6. Figure in the inventory screen

The behaviour is in `docs/spec/client-player-render.md`, section 5. This section states how the
client builds it. The figure is a player entity that only the GUI draws.

Three terms apply in this section:

- **World view**: the run of the entity passes for the camera of the window.
- **Figure view**: a second run of the entity passes, with a projection that its host gives and
  with no terrain.
- **Figure space**: the space of the entity positions of the figure view. The unit is one block.
  The figure stands at the origin.

### 6.1 How the figure gets on the screen

The cull, the pose and the expand run a second time for the figure view. They write a second,
small set of frame buffers. The GUI pass then draws the face records of the figure view straight
into the window target, with a forward pipeline of the renderer.

```
WorldPass::Entities:  scatter ─► cull ─► pose ─► expand ─► gbuffer draw      (world view)
                                 cull ─► pose ─► expand                      (figure view)
GUI pass:             quads ─► figure draw ─► quads, cursor stack, tooltip
```

- **The compute passes are those of the world view.** The slot records, the static tables, the
  skin layers and the pose pipelines are data of an entity, so the two views share them. The
  figure view owns only its frame uniform, its frame buffers and its bind groups. The frame
  buffers hold 16 full rigs.
- **The figure view has no terrain.** Its frame uniform has no depth pyramid level, frustum
  planes that reject nothing and a height threshold of 0. Thus each slot of the view is a full
  rig, and the view has no impostor. Its impostor lists hold one entry.
- **The projection is a value of the host.** The frame uniform takes the matrix that maps figure
  space to clip space. The eye of the figure view is a point 1024 blocks from the origin on the
  axis of the view. The face selection of the expand and the flip of a back face read the eye,
  so they need no parallel variant.
- **The shading is forward.** The forward pipelines use the vertex stage and the face records of
  the gbuffer draw. Their fragment stage computes the face factor with the two light directions
  of the host, then the overlay, then the lightmap at the light levels of the slot. It writes
  one colour. The world view and its shaders do not change.
- **The draw is one element of the GUI pass.** The GUI batch holds a draw of the kind `Figure` at
  the position of the figure in the draw order. For that draw, `draw_gui` sets the scissor to
  the figure rectangle in window pixels and takes one depth band, as for an item. Then it calls
  the renderer, which sets its pipeline and its bind groups and draws. The quads that follow
  draw over the figure.
- **A closed screen costs nothing.** With no figure frame, the figure run of the passes and the
  figure draw do not run. The renderer creates the buffers and the pipelines of the figure view
  in its prepare step, when the host inserts `FigureTargets`. Thus the first open of the screen
  compiles no shader.
- **The renderer binds the lightmap.** The prepare step binds the lightmap of the terrain and
  its sampler. It makes the bind group again only when the terrain gives another lightmap. The
  host gives only the render pass.

`draw_gui` keeps the tracked render pass of Bevy. For the figure draw, it gives the `wgpu` pass
of that tracked pass with `wgpu_pass()`. This call resets the tracked state of the pass. Thus the
GUI sets its pipeline, its bind group and its vertex buffer again after the figure draw.

The options that this design rejects:

| Option | Reason |
|---|---|
| Draw the figure into the gbuffer | The lighting pass applies the light tint, the fog and the light directions of the dimension. CPR-FIG-005 forbids each of them. |
| Draw into an offscreen target that the GUI pass samples | It adds a target of the size of the figure rectangle, a new size at each change of the GUI scale and one more pass. The GUI pass already gives the clip rectangle, the window pixels and the draw order. |
| Pose on the CPU and draw with the mesh path of the GUI items | It is a second implementation of the pose, of the box UV, of the skin layers and of the overlay. The two implementations then differ. |

The device limit of `EntityRenderPlugin` applies: a device with no entity passes draws no
figure, and the figure rectangle stays empty. The Minimal path of `docs/entity-render.md`,
section 6, lifts that limit for the world view and for the figure view together.

The entity GPU state starts only when the terrain has a depth pyramid. The inventory screen
opens only in a world, so the figure does not see this limit. A viewer with no terrain needs a
start with no pyramid, which is later work.

One figure view runs in a frame, and each entity with the bit `FIGURE` draws in it. A second
consumer in the same frame needs an index of the view in place of the one bit.

### 6.2 View mask

- **The cull flags hold a view mask.** The high byte of the `u16` flags is a set of views: bit 0
  is the world view, bit 1 is the figure view. The frame uniform of a view holds its bit. A cull
  lane drops a slot whose mask does not hold the bit of the view.
- **The mask is a value of the entity.** The component `EntityViews` holds the mask. An entity
  with no `EntityViews` is in the world view only. `write_ticks` writes the mask into the cull
  record when the component changes.
- **A change of the mask is no structural change.** The figure gets its mask when it spawns. The
  local player in third person changes the value of its mask when the camera changes.

Rejected: a visible list of the figure view that the CPU writes, with a cull record that is not
valid for the world view. It gives the flag `valid` a second meaning, and the local player in
third person still needs a mask.

### 6.3 The figure entity and its inputs

The figure is one entity with the marker `InventoryFigure`. The client spawns it with the local
player, with the bundle `inventory_figure` of the GUI crate and the UUID of the local profile.
The entity exists for the whole session, and no system despawns it. It exists while the screen
is closed. Thus the skin loads one time, and to open the screen changes no archetype. A closed
screen costs one slot and one skin layer.

The bundle holds the components of `host_living_entity`, and `InventoryFigure`,
`EntityViews::FIGURE`, `FramePosed`, `HostLight` and an `EntityLight` with the levels 15 and 15.

| Input | Writer | Value |
|---|---|---|
| `PlayerModel`, `PlayerSkin` | `dress_players` of the client | the profile that the player list has for the UUID of the figure, as for each other player (CPR-SKIN-005). `dress_players` skips each `HostEntity` except an entity with `InventoryFigure` |
| `Motion` | the bundle | the origin of figure space; no system moves the figure |
| body yaw, head yaw, pitch | the figure system | the angles `a` and `t` of CPR-FIG-003, items 3 to 5 |
| walk, hurt time, pose, flags | the figure system | the frame values of the local player (CPR-FIG-004) |
| scale | the bundle | 1 |
| `EntityLight` | the bundle | block light 15 and sky light 15 (CPR-FIG-005, item 1) |
| `FigureView` | the figure system | the figure frame while the screen is open, else none |

- **One system writes the figure.** The figure system of the GUI crate runs in `PostUpdate`,
  before `EntitySlotSystems`. It runs while the inventory screen is open, and one more time when
  the screen changes. It writes a component only when the value changes.
- **The figure holds frame values.** The values of the figure are those of the frame, not of a
  tick. With `FramePosed`, `write_ticks` writes each tick record as a fresh record, so the GPU
  does not interpolate from the record of the frame before.
- **The tick does no harm.** The tick of `EntityTickPlugin` can run on the figure. The figure
  system runs after it and replaces each value that the tick wrote.
- **The light stays.** The sampler of `mcrs_minecraft_client_entity` skips an entity with
  `HostLight`. Section 2 changes in one row: the client writes the light of each entity with no
  `HostLight`.
- **The state of the local player comes from its visual components.** The figure system reads
  `Living` and `SyncedData` of the entity with the marker `Player`, when that entity has them.
  The local player has none of them today. Thus the figure stands, does not walk and has no
  overlay, until the work of section 4.1 gives the local player these components. The figure
  system does not change then.
- **The tilt is a part of the projection.** The figure frame holds the matrix from figure space
  to clip space: the turn by `t` about the X axis through the origin, the scale of 30 GUI units
  for one block, the feet point of CPR-FIG-002 and the size of the window. The figure frame also
  holds the two light directions of CPR-FIG-005 in figure space. The GUI turns them against the
  tilt, so they stay fixed on the screen.
- **An invisible local player has no figure draw.** While `SyncedData` of the local player has
  the flag `invisible`, the figure system gives no figure frame.

Rejected: an entity that exists only while the screen is open. Each open then adds an entity,
takes a skin layer and starts the download of the skin again.

Rejected: a sampler that skips each `HostEntity`. A player of a host that stands in a world with
columns then loses the light of the world.

### 6.4 Crate map

No crate is new. `mcrs_minecraft_client_gui` gets the dependencies
`mcrs_minecraft_entity_render` and `mcrs_minecraft_entity_visual`. The renderer and the visual
crate get no dependency, so the `layers` check of `xtask` holds.

| Crate | New public name | Contract |
|---|---|---|
| `mcrs_minecraft_entity_visual` | `HostLight` | a marker: the host of the entity writes its `EntityLight` |
| `mcrs_minecraft_entity_render` | `EntityViews` | a component with the view mask; the constants `WORLD` and `FIGURE` |
| | `FramePosed` | a marker: the tick values of the entity are values of the frame |
| | `FigureFrame` | the matrix from figure space to clip space, and two light directions in figure space |
| | `FigureView` | a resource of the main world with the figure frame of this frame, or none; the host writes it |
| | `FigureTargets` | a resource of the render world: the colour format, the depth format, the depth compare and the sample count of the pass that takes the figure draw; the host inserts it |
| | `FigureDraw` | a system parameter of the render world; `encode` draws the figure view into a `wgpu` render pass, and draws nothing when the view is not ready |
| | `FigureViewSystems` | the set in `Core3d` that runs the compute passes of the figure view; a host pass runs after it |
| | `FrameUniform::view` | the view bit; the struct grows by one field |
| `mcrs_minecraft_client_gui` | `InventoryFigure` | the marker of the figure entity |
| | `inventory_figure` | the bundle of the figure for a UUID |
| | `GuiQuad::Figure`, `GuiDrawKind::Figure` | the figure rectangle in the scene, and its draw in the batch |

The host of the figure view gives the lightmap to the forward pipelines: `FigureDraw::encode`
takes the view and the sampler of the lightmap of the terrain. The pure layout functions of the
figure stay private to the GUI crate.

A skin viewer uses `FigureView`, `FigureTargets` and `FigureDraw` with its own render pass. The
local player in third person uses `EntityViews`.

### 6.5 What the figure work does not build

- Flames on the figure (CPR-DEV-005), a shadow, a name tag, armour, items in hands, a cape and an
  elytra (`client-player-render.md`, section 0.4).
- The translucent figure of an invisible player in a team that sees its invisible members
  (CPR-FIG-004). The renderer has no translucent pass. The figure does not draw then.
- The crouch, the walk and the hurt overlay of the figure, until the local player has its visual
  components (section 6.3).
- The scale of 15/16 and the lower model of a crouch (`client-player-render.md`, section 8,
  item 1). They are a change of the model for each player, not of the figure.
- A screenshot comparison against the game (CPR-TEST-003 to CPR-TEST-005).
- The figure on a device with no entity passes (section 6.1).
