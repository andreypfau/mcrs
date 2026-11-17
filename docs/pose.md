# Pose and eye height: design

Design note for the pose and the eye height of a player. Behaviour comes from
`docs/spec/player-pose.md` (POSE), `docs/spec/entity-physics.md` (PHY), `docs/spec/client-camera.md`
(CCAM) and `docs/spec/client-player-render.md` (CPR).

## 1. Classification of the state

| Value | Kind | Owner |
|-------|------|-------|
| pose of a player | authoritative truth on the server; derived on a client from entity data | the selection system of the server |
| shift key state | authoritative truth, from `player_input` and `interact` | the packet ingest |
| swim state, fall flying, spin attack, sleep | authoritative truth, each in its own domain | fluids, elytra flight, riptide, beds |
| box, eye height | derived from the position, the pose and the `scale` attribute; never stored | none |
| entity data entries 0 and 6 | a projection of the pose and the flags | the entity data encoder, on `Changed` |
| eye filter of the camera | derived state with history (CCAM-EYE-001) | the camera tick of the client |

## 2. Crate map

| Crate | Layer | Content |
|-------|-------|---------|
| `mcrs_minecraft_pose` (`no_std`) | 3, data | `Pose` with the wire codec; `Dimensions`, `Scaling`; `Pose::player_dimensions` |
| `mcrs_minecraft_physics` | 3, rules | `select_pose(view, &PoseInputs) -> Pose`; `tick_player` calls it |
| `mcrs_minecraft_engine` | 4 | the components `Pose` and `ShiftKey` on a player; the ingest of `player_input`; the set `PoseSystems::{Select, Publish}`; the entity data of a player for trackers and for the player |
| `mcrs_minecraft_vanilla_*` (the plugin that holds player movement) | 5 | the system that calls `select_pose` in `PoseSystems::Select` |
| `mcrs_minecraft_client` | 7 | the eye filter and the reach ray read the pose of the local player |
| `mcrs_minecraft_entity_visual`, `mcrs_minecraft_entity_render` | 6 | POSE-REN-001 to POSE-REN-009; the eye height of a remote player from its pose |

Reason for the new crate: a proxy, a viewer and a world map read the pose and its box (POSE-API-003).
They must not take the physics kernel or the entity registries for that.

## 3. Use cases

| User | Takes | Does not take |
|------|-------|---------------|
| proxy | `mcrs_minecraft_pose` to decode entry 6 | physics, Bevy |
| world map, model viewer | `mcrs_minecraft_pose`, the drawing of POSE-REN | physics, network |
| headless bot | the client without rendering: `select_pose` through `tick_player`, the eye height for its target ray | rendering |
| minigame server | the engine; a system in `PoseSystems::Select` after the vanilla one replaces the pose (for example, no crawl) | — |
| server with other rules | removes the vanilla plugin and writes `Pose` itself | the vanilla selection |

## 4. Server tick

1. Ingest: `player_input` sets `ShiftKey` and bit 1 of the shared flags at once (POSE-SRV-002).
2. Movement.
3. `PoseSystems::Select`: the vanilla system calls `select_pose` for every player each tick
   (PHY-BOX-006). It writes `Pose` with `set_if_neq`. User systems run after it.
4. `PoseSystems::Publish`: on `Changed<Pose>` or a changed flag, encode the entries 0 and 6 once
   and send them to the trackers and to the player (POSE-NET-003).

The box and the eye height are functions of the pose (POSE-SRV-010). Each reader computes them.
Nothing pushes a new box into another state.
