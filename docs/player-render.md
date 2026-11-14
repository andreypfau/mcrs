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

The crate adds three public names:

- `TickingArea`: a trait of a resource that tells whether the entities of a column tick.
- `EntityTickPlugin<A: TickingArea>`: the plugin that ticks each entity in `ClientEntityTick`.
- `EntityLight`: a component with the block light and the sky light at the eye of an entity.

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
| light at the eye | `EntityLight` | `entity_visual` | the client, from its columns; a host or a viewer sets its value |
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
entity and view, which goes into the spare bits of the cull flags. No present type changes.

### 4.2 Player doll in the inventory GUI

The doll is an entity with the same components, an `EntityLight` that the GUI sets, and a
placement in the space of its view. It needs a second view with no terrain. The slots, the skin
layers and the pose are data of an entity, not of a view. Only the cull, the draw targets and
the frame uniform are bound to one view, and they are private to the render world. The second
view adds a view mask for each entity and one set of these private resources for each view. No
public type of the player work changes.

### 4.3 Skin viewer or editor

The viewer takes `mcrs_minecraft_entity_render` and `mcrs_minecraft_entity_visual`. It adds the
entity with `host_living_entity`, sets `PlayerModel` and `PlayerSkin`, and adds
`EntityTickPlugin` with a ticking area that holds each column. An editor writes `PlayerSkin`
again after each change, and the next frame draws the new texels. The viewer needs the second
view of section 4.2 when it draws no terrain.

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
