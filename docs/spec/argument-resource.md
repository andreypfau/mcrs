# Identifier arguments: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `resource_location` 36, `dimension` 41, `resource_or_tag` 44, `resource_or_tag_key` 45 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, through `commands-packet.md`, CMDP-ARG-001.

This document states four argument types that read an identifier. The first two are a plain
identifier and a dimension. The third reads an entry or a tag of a registry and checks it at
parse time. The fourth reads an entry key or a tag key and does not check it. The document does
not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Reader**, **cursor**, **source**, **error**, **suggestion**, **remaining input**: as
  `command-parser.md` defines them.
- **Identifier**: as `registry.md`, REG-ID-001, defines it.
- **Identifier character**: `0` to `9`, `a` to `z`, `_`, `:`, `/`, `.` or `-`.
- **Registry**: the registry that the argument names in its properties.
- **Tag key**: an identifier with the prefix `#`, which names a tag of the registry.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ARG-RES` | the four identifier arguments |

---

## 1. Reading an identifier

### ARG-RES-001: Read an identifier

- **Behavior:** note the start.
  1. Read the longest run of identifier characters.
  2. If the run is a valid identifier (`registry.md`, REG-ID-001), the result is that
     identifier. The cursor stays after the run.
  3. Otherwise set the cursor back to the start, and fail with `argument.id.invalid`.
- **Edge cases:**

| Text at cursor | Result |
|----------------|--------|
| `stone` | `minecraft:stone` |
| `Stone` | `minecraft:` with an empty path: `S` is not an identifier character, so the run is empty |
| `a:b:c` | `argument.id.invalid` at the start |
| `:x` | `minecraft:x` |
| `stone[x]` | `minecraft:stone`, cursor before `[` |

  An empty run is the valid identifier `minecraft:` with an empty path. The next step of the
  parser then decides: `Stone` fails with `dispatcher_expected_argument_separator`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `bossbar add a:b:c "X"`. The reply has `argument.id.invalid`, with the context at
`a`. Send `bossbar add :x "X"`, then `bossbar list`: the list has `minecraft:x`.

### ARG-RES-002: Suggest identifiers

- **Inputs:** a list of identifiers and an optional prefix `x`, for example `#`.
- **Behavior:** suggest each identifier that passes the filter of `command-parser.md`,
  CMD-SUG-008, for identifier candidates. With a prefix `x`, use the form of CMD-SUG-008 for
  identifiers with a prefix. The text of each suggestion is `x`, if given, and the full
  identifier `namespace:path`. CMD-SUG-006 sets the final order.
- **Edge cases:** `r` = `oak` suggests `minecraft:oak_log` and `minecraft:stripped_oak_log`.
  `r` = `#min` with the prefix `#` keeps `min`, and suggests every tag of the namespace
  `minecraft`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a vanilla
server, as an operator, send a suggestion request with `/locate structure vill`. The reply
includes every `minecraft:village_*` structure. The structure registry is not synchronized, so
the client gets this list from the server (ARG-RES-007).

---

## 2. The four types

### ARG-RES-003: `resource_location`

- **Read:** read an identifier (ARG-RES-001).
- **Suggest:** nothing, unless the node has a suggestion type.
- **Examples:** `foo`, `foo:bar`, `012`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: in the
vanilla client, type `/bossbar add `. The client shows no suggestion for the identifier.

### ARG-RES-004: `dimension`

- **Read:** read an identifier.
- **Suggest:** the identifiers of the dimensions that the source knows, with ARG-RES-002. For
  the client, these are the dimensions that the server announced at login.
- **When a command runs:** the command looks up the dimension with that identifier. If none
  exists, it fails with `argument.dimension.invalid`, with the identifier.
- **Examples:** `minecraft:overworld`, `minecraft:the_nether`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `execute in minecraft:nope run say x`. The reply has `argument.dimension.invalid`
with `minecraft:nope`. Then send `execute in the_nether run say x`: the command runs.

### ARG-RES-005: `resource_or_tag_key`

- **Properties:** the registry, an Identifier on the wire.
- **Read:**
  - If next is `#`: note the position of `#`. Skip it and read an identifier. The result is a
    tag key of the registry. If the identifier read fails, the error has the position after
    `#` (ARG-RES-001 sets the cursor back to there).
  - Otherwise read an identifier. The result is an entry key of the registry.
- **Postconditions:** the read does not check that the entry or the tag exists. The command
  checks it when it runs.
- **Suggest:** the tags of the registry with the prefix `#`, then the entries of the registry,
  with ARG-RES-002. The source finds the registry as ARG-RES-007 states.
- **Examples:** `foo`, `foo:bar`, `012`, `#skeletons`, `#minecraft:skeletons`.
- **Text of the value:** an entry prints as its identifier. A tag prints as `#` and its
  identifier.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `locate structure #minecraft:village` and `locate structure minecraft:nope`. The
first finds a village. The second reply is an error of the `locate` command, not
`argument.resource.not_found`, because the read does not check the entry.

### ARG-RES-006: `resource_or_tag`

- **Properties:** the registry, an Identifier on the wire.
- **Read:**
  - If next is `#`: skip it and read an identifier. If the registry has no tag with that
    identifier, fail with `argument.resource_tag.not_found`, with the identifier and the
    registry identifier, at the cursor after the identifier. The result is the tag.
  - Otherwise read an identifier. If the registry has no entry with that identifier, fail with
    `argument.resource.not_found`, with the identifier and the registry identifier, at the
    cursor after the identifier. The result is the entry.
- **Postconditions:** the read needs the registry contents of the source. The client uses the
  registries that the server synchronized. The server uses its own registries.
- **Suggest:** as ARG-RES-005, with the registry lookup of ARG-RES-007.
- **Text of the value:** an entry prints as its identifier. A tag prints as `#` and its
  identifier.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `locate biome minecraft:nope`. The reply has `argument.resource.not_found` with
`minecraft:nope` and `minecraft:worldgen/biome`, and its context ends after `nope`.

### ARG-RES-007: The registry of the suggestions

- **Behavior:** the source looks up the registry that the argument names.
  - The client looks in its registries: the built-in registries and the registries that the
    server synchronized. When it finds the registry, it suggests from it with ARG-RES-002.
  - When the client does not find the registry, it suggests nothing itself. It sends a
    suggestion request with the command line up to its cursor to the server
    (`commands-packet.md`, CMDP-SUG-001). The reply of the server gives the suggestions
    (CMDP-SUG-002).
  - The server looks in its registries. When it does not find the registry there, it looks in
    its reloadable registries, for example loot tables and predicates. When it does not find the
    registry in either, it suggests nothing.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the client,
type `/locate structure vill`. The client has no structure registry, so it sends a suggestion
request. Capture the serverbound packet, and read the suggestions of the reply. They include
every `minecraft:village_*` structure.

---

## 3. Errors

| Translation key | Arguments |
|-----------------|-----------|
| `argument.id.invalid` | none |
| `argument.dimension.invalid` | identifier |
| `argument.resource.not_found` | identifier, registry |
| `argument.resource_tag.not_found` | identifier, registry |

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as an operator, send
`bossbar add a:b:c "X"`, `execute in minecraft:nope run say x`, `locate biome minecraft:nope`
and `locate biome #minecraft:nope`. The replies have the four keys of the table, in this order.
