# Frames of the vanilla client as a reference: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of the game values: `assets/minecraft/version.json`.

The vanilla client is an external judge of the mob rendering of mcrs. A scene file states a
small world, the commands that a server runs in it, and the camera poses of the frames. A
capture task runs each scene in the vanilla client and writes each frame as an image, with a
record of the render inputs of the frame. The mcrs client renders the same scenes. A comparison
measures the difference of the two images. This document states the scene format, how a client
runs a scene, the content of a record, and how a comparison works.

## 0. Conventions

### 0.1 Terms

- **Scene**: one file `tools/scenes/<name>.json`.
- **Script**: the ordered list of steps of a scene.
- **Capture**: a step that makes one frame.
- **Frame**: one image of a scene, at one camera pose, after a part of the script.
- **Background frame**: a frame at the camera pose of a frame, with no entity other than the
  player in the world.
- **Record**: the file `<scene>.records.json`. It holds the render inputs of each frame.
- **Frame artefact**: the images and the records in
  `crates/mcrs_minecraft_client/tests/fixtures/vanilla/frames/`. The task `captureClientFrames`
  of `tools/vanilla-oracle` writes them.
- **Capturing client**: the client that runs a scene and writes the frames. The vanilla client
  is the reference capturing client.
- **Frozen entity**: an entity that does not tick, because the server stopped the ticks of the
  world with the command `tick freeze`. A player is never frozen.
- **Eye position**: the position of the camera. It is the position of the player plus the eye
  height of the player.
- **Client tick**: one tick of the capturing client, 50 ms of game time.
- **Mask**: the set of pixels where a frame and its background frame are different.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule has a Milestone line. `1` means the first release
of these files. `later` means a rule that a later release implements.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CFC-SCENE` | the scene file |
| `CFC-RUN` | how a capturing client runs a scene |
| `CFC-REC` | the record of a frame |
| `CFC-CMP` | the comparison of two frames |
| `CFC-OPT` | the options that the mcrs client must expose |
| `CFC-TEST` | evidence and experiments |

### 0.4 Scope

This document covers the six scenes of milestone 1: `pig_idle`, `pig_walk`, `pig_lineup`,
`pig_hurt_death`, `frog_states` and `pig_grid`. Sounds, particles and the GUI are out of
scope.

## 1. The scene file

### CFC-SCENE-001: Fields of a scene

- **Inputs:** a scene file. It is a JSON object.
- **Behavior:** the object has these fields. Each field is necessary.
  1. `name`: the name of the scene. It is the stem of the file name.
  2. `description`: one sentence that states the content of the scene.
  3. `resolution`: the width and the height of each image, in pixels. All six scenes use
     `[854, 480]`.
  4. `world`: the world that the server creates. CFC-SCENE-002 states it.
  5. `options`: the client options. CFC-SCENE-003 states them.
  6. `script`: the steps. CFC-SCENE-004 states them.
- **Postconditions:** a capturing client can run the scene with no other input.

Provenance: `design` (the format is an mcrs format; nothing in the game defines it).
Kind: layout. Confidence: high. Milestone: 1.

### CFC-SCENE-002: The world

- **Inputs:** the `world` object.
- **Behavior:**
  1. `type` is `flat_default`. The world is the default flat world: one layer of bedrock, two
     layers of dirt and one layer of grass block, in the plains biome. The top face of the grass
     is at `y = −60`.
  2. `seed` is the text `1`.
  3. `generate_structures` is `false`.
  4. `game_rules` gives these values: `advance_time` `false`, `advance_weather` `false`,
     `spawn_mobs` `false` and `mob_drops` `false`.
  5. The world has one player. The name of the player is `Capture`. The player is in the game
     mode `spectator`, so the client draws no hand and the player has no body in the frame.
- **Postconditions:** the terrain, the sky and the light are the same in each run.

Provenance: `design`. The value `mob_drops = false` prevents an experience orb at a random
position (CFC-TEST-001). Kind: semantic. Confidence: high. Milestone: 1.

### CFC-SCENE-003: The options

- **Inputs:** the `options` object.
- **Behavior:**
  1. Each key is a key of the options file of the game, `options.txt`. Each value is a text. It
     is the value as the options file writes it.
  2. The capturing client sets each option before it creates the world. An option that the
     object does not name keeps its default value.
  3. The six scenes set these values:

     | Key | Value | Meaning |
     |-----|-------|---------|
     | `fov` | `0.0` | field of view 70°; the file writes `(degrees − 70) / 40` |
     | `fovEffectScale` | `0.0` | no change of the field of view from the state of the player |
     | `bobView` | `false` | no view bobbing |
     | `ao` | `true` | smooth lighting on |
     | `entityShadows` | `false` | no shadow under an entity |
     | `renderClouds` | `"false"` | no clouds |
     | `renderDistance` | `5` | 5 chunks |
     | `gamma` | `0.5` | the default brightness |
     | `vignette` | `false` | no dark border |
     | `entityDistanceScaling` | `1.0` | the default distance at which entities are drawn |
     | `chunkSectionFadeInTime` | `0.0` | a new chunk section shows at once |
     | `improvedTransparency` | `false` | the default transparency |
     | `mipmapLevels` | `4` | the default mipmap levels |

  4. The capturing client hides the HUD.
- **Postconditions:** the frame shows only the world and the entities.

Provenance: `design` (the values); `documentation` (the keys are those of `options.txt`).
Kind: layout. Confidence: high. Milestone: 1.

### CFC-SCENE-004: Steps of the script

- **Inputs:** the `script` array. Each element is an object with one of these fields.
- **Behavior:**

  | Field | Effect |
  |-------|--------|
  | `command` | The server runs the command, with the permissions of the server console. |
  | `commands` | The server runs each command of the array, in order. |
  | `wait` | The client waits the given number of client ticks. |
  | `step` | The server runs `tick step <n>`. The client waits until the server ends the step. Then the client waits 10 client ticks. |
  | `pose` | The server sets the pose of the entity with the UUID `entity` to `pose`, for example `croaking` or `using_tongue`. The server sends the pose to the clients as synchronised entity data. The client waits 10 client ticks. |
  | `capture` | The client makes a frame with the name `capture` at the camera pose `at` (CFC-RUN-002). `partial_tick` is the partial tick of the frame. It is `1.0` when it is absent. |

  Each scene starts with the commands `gamerule minecraft:mob_drops false`, `time set 6000`,
  `weather clear` and `tick freeze`, and then a wait of 10 client ticks.
- **Postconditions:** the script gives the same server state in each run.

Provenance: `design`. A vanilla server has no command that sets a pose. A conforming
reproduction of the `pose` step needs a server that can set it, for example the capture server
of the reference. Kind: layout. Confidence: high. Milestone: 1.

## 2. Running a scene

### CFC-RUN-001: Order of a run

- **Inputs:** a scene.
- **Behavior:**
  1. The client sets the options (CFC-SCENE-003) and hides the HUD.
  2. The client creates the world (CFC-SCENE-002) and joins it.
  3. The client waits until it has all chunks in the render distance. Then it waits 40 client
     ticks, so that the client builds the meshes of the chunks.
  4. The server puts the player in the game mode `spectator`.
  5. The client runs the steps of the script, in order.
  6. The server teleports each entity that is not a player to `(0, −60, 5000)`. The entities go
     out of the tracking range of the client. The client waits 20 client ticks.
  7. For each frame of step 5, in order, the client makes a background frame at the same camera
     pose and with the same partial tick. The name of the background frame is
     `<frame>.background`.
  8. The client writes the record of the scene (CFC-REC-001).
- **Postconditions:** each frame has a background frame.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

### CFC-RUN-002: Camera pose of a frame

- **Inputs:** the object `at` of a capture: `position` `[x, y, z]` in blocks, `yaw` and
  `pitch` in degrees.
- **Behavior:**
  1. `position` is the eye position.
  2. The server teleports the player to `(x, y − e, z)` with the rotation `(yaw, pitch)`. `e` is
     the eye height of the player. For a standing player in the game mode `spectator`, `e` is
     `1.62`.
  3. The client waits 10 client ticks. Then it waits until it has all chunks in the render
     distance, and then 40 client ticks more.
  4. The client draws the frame at the size `resolution`, with the partial tick of the capture.
  5. The yaw is 0° to +Z (south) and 90° to −X (west). A positive pitch looks down.
- **Postconditions:** the camera is at rest. The eye position and the rotation do not change
  between the previous tick and the current tick, so the partial tick does not move the camera.

Provenance: `source_suggested_black_box_confirmed` (each record shows the eye position, the
yaw and the pitch of the scene). Kind: semantic. Confidence: high. Milestone: 1.

### CFC-RUN-003: Entities are frozen

- **Inputs:** a frame.
- **Behavior:**
  1. Each entity that is not a player is a frozen entity. The client draws a frozen entity with
     the partial tick `1.0`, whatever the partial tick of the frame. Thus the client draws the
     state of the current tick.
  2. A change of the state of an entity occurs only in a `step`, or from a command or a `pose`
     step.
  3. In `pig_walk`, the server moves the pig by `0.2` blocks to +Z with
     `execute as <uuid> at @s run tp @s ~ ~ ~0.2`. The client waits 5 client ticks, so that it
     receives the new position before the `step`. Then a `step` of 1 tick lets the client move
     the pig toward the new position and advance the walk animation.
- **Postconditions:** the render inputs of each frame are the same in each run.

Provenance: `source_suggested_black_box_confirmed` (CFC-TEST-001: three runs give the same
bytes). Kind: semantic. Confidence: high. Milestone: 1.

## 3. The record

### CFC-REC-001: Content of a record

- **Inputs:** the frames of a scene.
- **Behavior:** the record is a JSON object with these fields.
  1. `scene`: the name of the scene.
  2. `version`: the game version.
  3. `frames`: one object for each frame and each background frame, in the order of the files.
     Each object has these fields:
     - `image`: the file name of the image.
     - `camera`: `position` (the eye position, blocks), `yaw` and `pitch` (degrees), `fov`
       (the vertical field of view that the client used, degrees) and `partial_tick`.
     - `entities`: one object for each entity that the client draws, except the player.
  4. An entity object has these fields:

     | Field | Content |
     |-------|---------|
     | `id` | the UUID of the entity |
     | `type` | the entity type id |
     | `frozen` | `true` when the entity is a frozen entity |
     | `partial_tick` | the partial tick of the entity: `1.0` for a frozen entity |
     | `position` | the position at the partial tick, in blocks |
     | `age` | the ticks that the client counted for the entity |
     | `pose` | the pose that the client has for the entity |
     | `invisible` | `true` when the entity is invisible |
     | `head_pitch` | the pitch of the head at the partial tick, degrees |
     | `body_yaw` | the yaw of the body at the partial tick, degrees |
     | `head_yaw` | the yaw of the head at the partial tick, degrees |
     | `walk_position` | the position of the walk animation at the partial tick |
     | `walk_speed` | the speed of the walk animation at the partial tick |
     | `hurt_time` | the remaining ticks of the hurt flash, `0` to `10` |
     | `death_time` | the ticks since the death, `0` to `20` |
     | `baby` | `true` for a baby |
     | `saddled` | `true` when the entity wears a saddle |
     | `variant` | the variant id, for a pig and a frog |
     | `croak`, `tongue` | for a frog: `started` and `elapsed_ticks` of the animation |

     The fields from `body_yaw` to `baby` are only on a living entity.
- **Postconditions:** a client can draw a frame from the record without a simulation of the
  entities (CFC-CMP-002).

Provenance: `source_suggested`. The values come from the state of the vanilla client at the
frame. `client-entities.md` defines the walk position, the walk speed, the body yaw, the hurt
time and the death time. Kind: layout. Confidence: high. Milestone: 1.

### CFC-REC-002: Packets and entity updates of a scene

- **Inputs:** a run of a scene.
- **Behavior:**
  1. From the first step of the script to the last frame, the harness writes
     `<scene>.packets.json` and `<scene>.entity_ticks.json` next to the record.
  2. The harness stops the two files before it moves the mobs away for the background frames.
  3. `client-entities.md`, section 0.2, states the keys of the two files.
- **Postconditions:** a replay of the packets in a client gives the entity rows of the scene.

Provenance: `source_suggested_black_box_confirmed`. Three runs of `pig_walk`,
`pig_hurt_death` and `pig_idle` gave the same files, except the arrival count of a packet that
arrives while no entity ticks. Kind: layout. Confidence: high. Milestone: 1.

## 4. Comparison

### CFC-CMP-001: Image difference

- **Inputs:** a frame of the frame artefact, its background frame, and the frame that the mcrs
  client draws for the same scene and the same capture.
- **Behavior:**
  1. All images are 8-bit sRGB PNG files, with no alpha.
  2. The mask is the set of pixels where the vanilla frame and the vanilla background frame are
     different in a channel. The comparison expands the mask by 2 pixels in each direction.
  3. The comparison computes the LDR FLIP error of the mcrs frame against the vanilla frame,
     with a viewing distance of 0.7 m on a 0.7 m wide monitor at 3840 pixels.
  4. The comparison reports the mean FLIP error in the mask and the mean FLIP error out of the
     mask. The mean in the mask measures the entities. The mean out of the mask measures the
     terrain and the sky.
  5. A frame passes when each mean is not more than its threshold:

     | Scene | In the mask | Out of the mask |
     |-------|-------------|-----------------|
     | `pig_idle`, `pig_lineup` | 0.05 | 0.10 |
     | `pig_walk`, `pig_hurt_death`, `frog_states` | 0.08 | 0.10 |
     | `pig_grid` | 0.10 | 0.10 |

  6. The comparison also writes the FLIP error image of each frame, for a person to look at.
- **Postconditions:** a failure names the scene, the frame, the two means and the thresholds.

Provenance: `design`. The thresholds are a proposal. The first run of the mcrs client confirms
or changes them. Kind: deviation. Confidence: medium. Milestone: 1.

### CFC-CMP-002: Two modes of input

- **Inputs:** a scene.
- **Behavior:**
  1. **Full mode.** The mcrs client joins a server that runs the scene. The mcrs client computes
     the state of each entity from the data that the server sends. The comparison tests the
     client state and the rendering together.
  2. **Render mode.** The mcrs client takes the render inputs of each entity from the record.
     The comparison tests only the rendering.
  3. When the full mode fails and the render mode passes, the defect is in the client state.
     When both fail, the defect is in the rendering.
- **Postconditions:** each failure points at the client state or at the rendering.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1 (render mode); later (full
mode, which needs a server that can run the `pose` step).

## 5. Options of the mcrs client

### CFC-OPT-001: Options that a comparison needs

- **Inputs:** none.
- **Behavior:** the mcrs client lets a test set each of these, with the meaning of
  CFC-SCENE-003: the field of view, the effect scale of the field of view, view bobbing, smooth
  lighting, entity shadows, clouds, the render distance, the brightness, the vignette, the
  distance scale of entities, the fade time of a chunk section, the mipmap levels, the hidden
  HUD and the size of the image.
- **Postconditions:** the mcrs client can draw a frame with the options of a scene.

Provenance: `design`. Kind: layout. Confidence: high. Milestone: 1.

## 6. Evidence and experiments

### CFC-TEST-001: The frames are the same in each run

- **Procedure:**
  1. Run `captureClientFrames` three times.
  2. Compare the bytes of each image and each record of the runs.
- **Expected result:** each file is the same in each run.
- **Result on 2026-11-07:** each file was the same in three runs, after the scenes set
  `mob_drops` to `false`. Before that, the dying pig of `pig_hurt_death` dropped an experience
  orb at a random position.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high.
Milestone: 1.

### CFC-TEST-002: The machine of the frame artefact

- **Procedure:** read the log of the capture task.
- **Expected result:** the log names the graphics backend and the device.
- **Result on 2026-11-07:** the vanilla client used the Vulkan backend through MoltenVK
  1.4.2 on an Apple M4 Max. Another device can give other pixels at the edges of textures.
  A new capture on another device must not replace the frame artefact without a review.

Provenance: `black_box_confirmed`. Kind: deviation. Confidence: high. Milestone: 1.

## 7. Open questions

1. With the render distance `8`, the wait for the chunks of CFC-RUN-001 does not end. The scenes
   use the render distance `5`. The cause is not known.
2. In `frog_states`, the record gives the pose `standing` for the frogs after the `pose` step,
   but the croak and the tongue animations are `started`. The croak at tick 6 shows no visible
   change of the throat. A capture at more ticks can show when the croak shape is visible.
3. A server that runs the full mode (CFC-CMP-002) is not yet available: a vanilla server cannot
   run the `pose` step.
