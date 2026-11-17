# Pose and eye height: design

Design note for the pose and the eye height of a player. Behaviour comes from
`docs/spec/player-pose.md` (POSE), `docs/spec/entity-physics.md` (PHY), `docs/spec/client-camera.md`
(CCAM) and `docs/spec/client-player-render.md` (CPR).

## 1. State today

- Two `Pose` enums exist. `mcrs_minecraft_entity::Pose` has the 18 wire values. `mcrs_minecraft_physics::Pose`
  has 7 values and the player dimensions. No conversion exists between them.
- `mcrs_minecraft_physics` selects the pose in `tick_player` (PHY-BOX-006). Only the client calls it.
  The selection does not have the inputs `fall_flying` and `spin_attack`.
- The server keeps no pose and no shift key. It does not handle `player_input`. It sends no entity
  data for players.
- The eye height `1.62` is a constant in about 15 places: the camera, the reach rays of the client
  and the server, commands, item launch, the mob view of a player and the GUI figure. The standing
  box `0.6 × 1.8` is a constant in 4 places.
- The client draws the crouch of a remote player (CPR-POSE-003). It does not draw the other poses.

## 2. Classification of the state

| Value | Kind | Owner |
|-------|------|-------|
| pose of a player | authoritative truth on the server; derived on a client from entity data | the selection system of the server |
| shift key state | authoritative truth, from `player_input` and `interact` | the packet ingest |
| swim state, fall flying, spin attack, sleep | authoritative truth, each in its own domain | #88, #91, the riptide item, #739 |
| box, eye height | derived from the position, the pose and the `scale` attribute; never stored | none |
| entity data entries 0 and 6 | a projection of the pose and the flags | the entity data encoder, on `Changed` |
| eye filter of the camera | derived state with history (CCAM-EYE-001) | the camera tick of the client |

## 3. Crate map

| Crate | Layer | Content | Change |
|-------|-------|---------|--------|
| `mcrs_minecraft_pose` (new, `no_std`) | 3, data | `Pose` with the wire codec; `Dimensions`, `Scaling`; `Pose::player_dimensions` | takes `Pose` from `mcrs_minecraft_entity` and `Dimensions` from `mcrs_minecraft_physics` |
| `mcrs_minecraft_physics` | 3, rules | `select_pose(view, &PoseInputs) -> Pose`, public | the selection leaves `tick_player`; `tick_player` calls it |
| `mcrs_minecraft_engine` | 4 | the components `Pose` and `ShiftKey` on a player; the ingest of `player_input`; the set `PoseSystems::{Select, Publish}`; the entity data of a player for trackers and for the player | new |
| `mcrs_minecraft_vanilla_*` (the plugin that holds player movement) | 5 | the system that calls `select_pose` in `PoseSystems::Select` | new |
| `mcrs_minecraft_client` | 7 | the eye filter reads the pose of the local player | constant removed |
| `mcrs_minecraft_entity_visual`, `mcrs_minecraft_entity_render` | 6 | POSE-REN-001 to POSE-REN-009; the eye height of a remote player from its pose | new |

Reason for the new crate: a proxy, a viewer and a world map read the pose and its box (POSE-API-003).
They must not take the physics kernel or the entity registries for that.

## 4. Use cases

| User | Takes | Does not take |
|------|-------|---------------|
| proxy | `mcrs_minecraft_pose` to decode entry 6 | physics, Bevy |
| world map, model viewer | `mcrs_minecraft_pose`, the drawing of POSE-REN | physics, network |
| headless bot | the client without rendering: `select_pose` through `tick_player`, the eye height for its target ray | rendering |
| minigame server | the engine; a system in `PoseSystems::Select` after the vanilla one replaces the pose (for example, no crawl) | — |
| server with other rules | removes the vanilla plugin and writes `Pose` itself | the vanilla selection |

## 5. Server tick

1. Ingest: `player_input` sets `ShiftKey` and bit 1 of the shared flags at once (POSE-SRV-002).
2. Movement (#94 when it lands).
3. `PoseSystems::Select`: the vanilla system calls `select_pose` for every player each tick
   (PHY-BOX-006). It writes `Pose` with `set_if_neq`. User systems run after it.
4. `PoseSystems::Publish`: on `Changed<Pose>` or a changed flag, encode the entries 0 and 6 once
   and send them to the trackers and to the player (POSE-NET-003).

The box and the eye height are functions of the pose (POSE-SRV-010). Each reader computes them.
Nothing pushes a new box into another state.

## 6. Out of scope of #90

The swim state (#88), fall flying (#91), the spin attack and sleep (#739) are inputs of
`PoseInputs` with the value `false` until their domains set them. The drawing of these poses on
a client comes with #90, because entity data from another server already carries them.

## 7. Steps

Each step is one pull request. The steps 1 to 3 have no dependency on the server.

1. `refactor`: the crate `mcrs_minecraft_pose`; delete the second `Pose` and move `Dimensions`; all
   callers move.
2. Physics: public `select_pose` with the full desired list of PHY-BOX-006.
3. Client: the eye filter (CCAM-EYE-001, CCAM-EYE-002) and the reach ray read the pose of the local
   player.
4. Engine: `ShiftKey`, `Pose`, the ingest of `player_input`, the vanilla selection system.
5. Engine: the entity data of a player for trackers and for the player (POSE-NET).
6. Server: the eye height and the box of each consumer read the pose (POSE-EYE-002); the constants
   are deleted.
7. Client: the drawing of remote players (POSE-REN-001 to POSE-REN-009).
