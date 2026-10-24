# Keyword arguments: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `entity_anchor` 38, `gamemode` 42 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, through `commands-packet.md`, CMDP-ARG-001.

This document states two argument types that read one word from a fixed set:
`minecraft:gamemode` and `minecraft:entity_anchor`. Both have no properties on the wire. It does
not prescribe an architecture.

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
- **Suggest:** each keyword that matches the remaining input in lower case: the keyword starts
  with it, or has it right after a `.`, `_` or `/`.
- **Examples:** `survival`, `creative`.
- **Edge cases:** `Creative` fails: the match is case-sensitive. `0` fails: the id is not a
  keyword.

Provenance: `source_suggested`. Confidence: high.

### ARG-KW-002: `entity_anchor`

- **Keywords:** `feet` and `eyes`.
- **Read:** note the start. Read an unquoted string. If it is a keyword, the result is that
  anchor. Otherwise set the cursor back to the start, and fail with `argument.anchor.invalid`,
  with the string.
- **Suggest:** each keyword that matches the remaining input as in ARG-KW-001.
- **Examples:** `eyes`, `feet`.
- **Value:** for an entity, `feet` is its position, and `eyes` is its position plus its eye
  height on the vertical axis. For a source without an entity, both are the source position.

Provenance: `source_suggested`. Confidence: high.

---

## Errors

| Translation key | Arguments | Position |
|-----------------|-----------|----------|
| `argument.gamemode.invalid` | the string | after the string |
| `argument.anchor.invalid` | the string | the start |

Provenance: `source_suggested`. Confidence: high.
