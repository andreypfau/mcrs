# Keyword arguments: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `entity_anchor` 38, `gamemode` 42, `heightmap` 51 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, through `commands-packet.md`, CMDP-ARG-001.

This document states three argument types that read one word from a fixed set:
`minecraft:gamemode`, `minecraft:entity_anchor` and `minecraft:heightmap`. Each has no properties
on the wire. It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Reader**, **cursor**, **error**, **suggestion**, **remaining input**, **unquoted string**:
  as `command-parser.md` defines them.
- **Keyword**: one word of the fixed set of a type.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-KW` | keyword arguments |

---

### ARG-KW-001: `gamemode`

- **Keywords:** `survival`, `creative`, `adventure`, `spectator`, in this order. They are the
  game modes with the ids 0, 1, 2 and 3.
- **Read:** read an unquoted string. If it is a keyword, the result is that game mode.
  Otherwise fail with `argument.gamemode.invalid`, with the string, at the cursor after the
  string. The cursor is not set back.
- **Suggest:** each keyword that passes the filter of `command-parser.md`, CMD-SUG-008.
  CMD-SUG-006 sets the final order.
- **Examples:** `survival`, `creative`.
- **Edge cases:** `Creative` fails: the match is case-sensitive. `0` fails: the id is not a
  keyword.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `gamemode creative` and `gamemode Creative`. The first sets the creative mode.
The second reply has `argument.gamemode.invalid` with `Creative`, and its context ends after
`Creative`.

### ARG-KW-002: `entity_anchor`

- **Keywords:** `feet` and `eyes`.
- **Read:** note the start. Read an unquoted string. If it is a keyword, the result is that
  anchor. Otherwise set the cursor back to the start, and fail with `argument.anchor.invalid`,
  with the string.
- **Suggest:** each keyword that passes the filter of `command-parser.md`, CMD-SUG-008.
  CMD-SUG-006 sets the final order.
- **Examples:** `eyes`, `feet`.
- **Value:** for a source with an entity, `feet` is the source position. `eyes` is the source
  position plus the eye height of that entity on the vertical axis. The source position
  can differ from the position of the entity. For a source without an entity, both are the
  source position.
- **Eye height:** the eye height of the entity in its current pose. A player that crouches has
  a lower eye height than a player that stands.
- **Lifetime:** each source starts at `feet`. `execute anchored` sets the anchor.
  `execute positioned <pos>` sets the position and sets the anchor back to `feet`.
  `execute as`, `execute at`, `execute positioned as` and `execute positioned over` keep the
  anchor. After `execute as`, `eyes` uses the eye height of the new entity.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, stand still and send `execute anchored eyes run teleport ^ ^ ^`. The player moves up
by the eye height. Then send `execute anchored eyes positioned ~ ~ ~ run teleport ^ ^ ^`. The
player does not move, because `positioned` sets the anchor back to `feet`.

### ARG-KW-003: `heightmap`

- **Keywords:** `world_surface`, `ocean_floor`, `motion_blocking`, `motion_blocking_no_leaves`, in
  this order. They are the kept heightmap types of `heightmap.md`, HM-001, in lower case. The
  generation types `world_surface_wg` and `ocean_floor_wg` are not keywords.
- **Read:** read an unquoted string. If it is a keyword, the result is that heightmap type.
  Otherwise fail with `argument.enum.invalid`, with the string, at the cursor after the string.
  The cursor is not set back.
- **Suggest:** each keyword that passes the filter of `command-parser.md`, CMD-SUG-008.
  CMD-SUG-006 sets the final order.
- **Examples:** `world_surface`, `ocean_floor`.
- **Edge cases:** `WORLD_SURFACE` fails: the match is case-sensitive, and the save uses the upper
  case name (`heightmap.md`, HM-001). `world_surface_wg` fails. `1` fails: the id is not a
  keyword. An empty string fails with the empty string.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute positioned over ocean_floor run say a`. The command runs. Then send
`execute positioned over WORLD_SURFACE run say a` and
`execute positioned over world_surface_wg run say a`. Each reply has `argument.enum.invalid`
with the string, and its context ends after the string. Type `/execute positioned over ` and
read the suggestions: the four keywords.

---

## Errors

| Translation key | Arguments | Position |
|-----------------|-----------|----------|
| `argument.gamemode.invalid` | the string | after the string |
| `argument.anchor.invalid` | the string | the start |
| `argument.enum.invalid` | the string | after the string |

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as an operator, send
`gamemode foo`, `execute anchored foo run say x` and `execute positioned over foo run say x`. The
replies have `argument.gamemode.invalid`, `argument.anchor.invalid` and `argument.enum.invalid`,
each with `foo`.
