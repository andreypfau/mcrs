# The command protocol: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states what a server and a client exchange about commands in the PLAY state:
the command tree that a player can see, the suggestions that the client asks for, the command
line that the client sends, and the feedback that the server returns. It does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Command tree**, **node**, **root**, **literal**, **argument**, **child**, **redirect**,
  **requirement**, **source**, **executable**, **command line**, **position**, **suggestion**,
  **error**: as `command-parser.md`, section 0.1, defines them.
- **Full tree**: the command tree that the server holds, with every node.
- **Player tree**: the projection of the full tree for one player (CMDP-TREE-001).
- **Wire node**: one entry of the `commands` packet.
- **Parser id**: the numeric id of an argument type in the static registry
  `minecraft:command_argument_type`.
- **Properties**: the bytes after the parser id that configure an argument type.
- **Suggestion type**: an identifier on an argument that tells the client where its suggestions
  come from (CMDP-ARG-002).
- **No-permission source**: a source that holds no permission and passes only the requirement
  `always_pass` (`command-permissions.md`, PERM-CHECK-001).
- **Restricted**: a node whose requirement fails for the no-permission source.
- **Operator**: a player in the operator list of the server (`command-permissions.md`).
- **System message**: a `system_chat` packet with `overlay` false.

### 0.2 Wire types

VarInt, String (n), Identifier and Boolean have the forms of `login.md`, section 0.2. These
types are also used:

| Type | Form |
|------|------|
| Byte | 1 byte |
| Int | 4 bytes, two's complement, big-endian |
| Long | 8 bytes, two's complement, big-endian |
| Float | 4 bytes, IEEE 754 binary32, big-endian |
| Double | 8 bytes, IEEE 754 binary64, big-endian |
| Array of X | a VarInt count, then that number of X |
| Text component | the network form of a text component: one NBT tag with no name |

### 0.3 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. `library_source` has the meaning of
`command-parser.md`, section 0.2.

### 0.4 Artefacts cited

- `assets/mcrs/reports/packets.json`: packet ids.
- `assets/mcrs/reports/registries.json`: the parser ids, in the registry
  `minecraft:command_argument_type`.
- `commands.json`: the command report that the game writes with `--reports`. The task
  `dumpReports` of `tools/vanilla-oracle` writes it to `reports/commands.json` in its output
  directory. The repository does not store it yet (section 9).

### 0.5 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CMDP-TREE` | the player tree and when it is sent |
| `CMDP-PKT` | the layout of the `commands` packet |
| `CMDP-ARG` | parser ids, properties and suggestion types |
| `CMDP-SUG` | the suggestion exchange |
| `CMDP-EXEC` | the command line from the client, and the feedback |

### 0.6 Packets

| State, direction | Name | Id |
|------------------|------|----|
| PLAY, clientbound | `commands` | 16 |
| PLAY, clientbound | `command_suggestions` | 15 |
| PLAY, serverbound | `command_suggestion` | 15 |
| PLAY, serverbound | `chat_command` | 7 |
| PLAY, serverbound | `chat_command_signed` | 8 |

Provenance: `black_box_confirmed` (`assets/mcrs/reports/packets.json`).

---

## 1. The player tree

### CMDP-TREE-001: Projection for one player

- **Inputs:** the full tree, and the source of one player.
- **Behavior:** the player tree has a new root. Visit the full tree depth first, from the root.
  For each child `C` of a visited node, in order:
  1. If the requirement of `C` fails for the source, skip `C` and all nodes below it.
  2. Make a copy of `C`: the same kind, name, argument type, suggestion type and command. The
     copy has no children yet.
  3. If `C` has a redirect, and the target already has a copy, the copy redirects to the copy
     of the target. Otherwise the copy has no redirect.
  4. Record the copy as the copy of `C`, and add it as a child of the copy of the visited node.
  5. Visit `C`.
- **Postconditions:** the root of the full tree has the new root as its copy before the visit
  starts. So a redirect to the root always survives.
- **Edge cases:**
  - A redirect to a node that the player cannot use is dropped. The copy becomes a leaf. It is
    executable only if the node itself has a command.
  - A redirect to a node that the visit reaches later is dropped, even if the player can use
    that node. An example is a redirect to a later sibling.

An implementation can reject a full tree in which a redirect target comes later in the visit
than the redirect, when the tree is built. No vanilla command has such a redirect.

Provenance: `source_suggested`. Confidence: medium. Experiment: on a vanilla server, give a
player level 0 and capture `commands`. The capture has no `tp` and no `teleport`. Give the
player level 2 and capture again. The node `tp` has the redirect flag, and its target is
`teleport`.

### CMDP-TREE-002: When the server sends the tree

The server sends a `commands` packet with the player tree:

1. when the player enters PLAY, after the packets that announce the recipes;
2. after the player respawns;
3. after the player moves to another dimension;
4. after the permission level of the player changes;
5. after a data pack reload, to every player.

In each case the server first sends the permission level of the player
(`command-permissions.md`, PERM-LEVEL-003).

Provenance: `source_suggested`. Confidence: medium. Experiment: capture the PLAY packets of a
join, of a death and respawn, of a Nether portal, of `op` and `deop`, and of `reload`. Each
sequence has an `entity_event` with an id from 24 to 28 for the player, then `commands`.

### CMDP-TREE-003: Node numbering

- **Inputs:** the player tree.
- **Behavior:** number the nodes by a breadth-first walk:
  1. Put the root in a queue.
  2. Take the first node from the queue. If it already has a number, skip it. Otherwise give
     it the next number, from 0, and put its children in the queue, in order, and then its
     redirect target, if any.
  3. Repeat step 2 until the queue is empty.
- **Postconditions:** the root has number 0. Each node has one number. The list of wire nodes
  is in the order of the numbers.

Provenance: `source_suggested`. Confidence: medium. Experiment: capture `commands` on a vanilla
server, and check that the root index is 0 and that the children of the root are 1 to `n`.

### CMDP-TREE-004: What the client does with the restricted flag

- **Behavior:** the client gives each restricted node a requirement that only a source with
  the client permission atom `minecraft:client/commands/restricted` passes. The client source
  for typed commands holds that atom. The client source for the second test below holds
  nothing.
- **Postconditions:** before the client sends a command that the player did not type, for
  example from a click event or a dialog, it parses the command line two times:
  1. with its own source. If the command line is not valid (`command-parser.md`,
     CMD-PARSE-007), it asks the player with the text `multiplayer.confirm_command.parse_errors`;
  2. with a source with no permissions. If the command line is then not valid, it asks the
     player with the text `multiplayer.confirm_command.permissions_required`.

  When the command has a signable argument, the client asks with
  `multiplayer.confirm_command.signature_required` between the two tests.

Provenance: `source_suggested`. Confidence: medium. Experiment: send a system message with a
`run_command` click event for `/gamemode creative` to an operator. A click opens the
confirmation screen with the text of `multiplayer.confirm_command.permissions_required`.

---

## 2. The `commands` packet

### CMDP-PKT-001: Layout

| Field | Type | Notes |
|-------|------|-------|
| nodes | Array of wire node | in the order of CMDP-TREE-003 |
| root index | VarInt | the number of the root, always 0 from a vanilla server |

A wire node:

| Field | Type | Present when |
|-------|------|--------------|
| flags | Byte | always |
| children | Array of VarInt | always; the numbers of the children, in order |
| redirect | VarInt | flag `0x08` is set; the number of the target |
| name | String (32767) | the node is a literal or an argument |
| parser id | VarInt | the node is an argument |
| properties | depends on the parser id (CMDP-ARG-001) | the node is an argument |
| suggestion type | Identifier | flag `0x10` is set |

Flags:

| Bits | Meaning |
|------|---------|
| `0x03` | node kind: 0 root, 1 literal, 2 argument |
| `0x04` | executable |
| `0x08` | has a redirect |
| `0x10` | has a suggestion type; only on an argument |
| `0x20` | restricted (CMDP-TREE-004) |
| `0x40`, `0x80` | always 0 |

Provenance: `source_suggested`. Confidence: medium. Experiment: decode a captured `commands`
packet with this layout, and check that the decode ends at the last byte.

### CMDP-PKT-002: Trees that the client rejects

The client rejects the packet, and the connection ends, when one of these is true:

- When only the redirect links are followed, a chain has no end at a node without a redirect.
  An example is a node that redirects to itself.
- When only the child links are followed, they form a cycle.
- A parser id is not in the registry. The client then cannot find the end of the properties.

A redirect to an ancestor, or to the root, is not a cycle in this sense.

Provenance: `source_suggested`. Confidence: medium. Experiment: send a tree whose node 1
redirects to node 1, with a modified server. The client disconnects.

---

## 3. Argument types on the wire

### CMDP-ARG-001: Parser ids and properties

The parser id of a type is its id in `assets/mcrs/reports/registries.json`, registry
`minecraft:command_argument_type`. The table lists the types that the parser and the first
commands use.

| Type | Parser id | Properties |
|------|-----------|------------|
| `brigadier:bool` | 0 | none |
| `brigadier:float` | 1 | number range, Float |
| `brigadier:double` | 2 | number range, Double |
| `brigadier:integer` | 3 | number range, Int |
| `brigadier:long` | 4 | number range, Long |
| `brigadier:string` | 5 | VarInt mode: 0 `word`, 1 `phrase`, 2 `greedy` |
| `minecraft:entity` | 6 | Byte: `0x01` one target only, `0x02` players only |
| `minecraft:vec3` | 10 | none |
| `minecraft:rotation` | 29 | none |
| `minecraft:resource_location` | 36 | none |
| `minecraft:entity_anchor` | 38 | none |
| `minecraft:dimension` | 41 | none |
| `minecraft:gamemode` | 42 | none |
| `minecraft:resource_or_tag` | 44 | Identifier: the registry |
| `minecraft:resource_or_tag_key` | 45 | Identifier: the registry |

A number range is a Byte of flags, then the minimum when flag `0x01` is set, then the maximum
when flag `0x02` is set, each in the type of the table. A bound is present when it differs from
the default of `command-parser.md`, CMD-ARG-002.

The `vec3` of the vanilla commands adds 0.5 to a whole absolute horizontal value. The client
makes its own `vec3` the same way, because `vec3` has no properties (`argument-coordinates.md`,
ARG-COORD-003).

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`) for the ids.
`source_suggested` for the properties. Confidence: medium. Experiment: decode the `gamemode`
node of a captured `commands` packet. Its `target` child has parser id 6 and the property byte
`0x02`.

### CMDP-ARG-002: Suggestion types

| Suggestion type | Who makes the suggestions |
|-----------------|---------------------------|
| `minecraft:ask_server` | the server, through CMDP-SUG |
| `minecraft:available_sounds` | the client, from the sound events that it knows |
| `minecraft:summonable_entities` | the client, from the entity types that `summon` can make |
| `minecraft:post_effects` | the client, from the post effects that it knows |

A vanilla server writes `minecraft:ask_server` for every argument whose suggestion source is
not one of the other three. The client treats an unknown suggestion type as
`minecraft:ask_server`. An argument without a suggestion type gets the suggestions of its type,
which the client computes itself.

Provenance: `source_suggested`. Confidence: medium. Experiment: find an argument with flag
`0x10` in a captured `commands` packet, and read its identifier.

### CMDP-ARG-003: A type that the client parses differently

The client parses each argument with the type of its parser id. When the server parses an
argument with a grammar that accepts a different set of command lines, the client underlines
command lines that the server runs, or the reverse. A server that has an argument type with no
parser id must send a type whose grammar the client applies in the same way. `brigadier:string`
in the mode `greedy`, with the suggestion type `minecraft:ask_server`, accepts every rest of a
line.

Provenance: `source_suggested`. Confidence: high.

---

## 4. Suggestions from the server

### CMDP-SUG-001: Request

`command_suggestion`:

| Field | Type | Notes |
|-------|------|-------|
| id | VarInt | chosen by the client |
| text | String (32500) | the command line up to the cursor of the client |

The client sends the request when the parse of its input reaches an argument with the
suggestion type `minecraft:ask_server`, or an unknown one. The text starts with `/` when the
input of the chat screen starts with `/`. The client keeps one request open. A new request
replaces it. The client takes only a reply whose id equals the id of the open request.

Provenance: `source_suggested`. Confidence: medium. Experiment: with a tree that has one
argument with the suggestion type `minecraft:ask_server`, type `/x a` and capture the request.
Its text is `/x a`.

### CMDP-SUG-002: Reply

- **Behavior:**
  1. If the text starts with `/`, the parse starts at position 1. Otherwise it starts at 0.
  2. Parse the text with the source of the player (`command-parser.md`, section 4).
  3. Collect the suggestions at the end of the text (`command-parser.md`, section 6).
  4. Keep the first 1000 suggestions of the list.
  5. Send `command_suggestions`.
- **Postconditions:** all positions count from the start of the text, the `/` included.

`command_suggestions`:

| Field | Type | Notes |
|-------|------|-------|
| id | VarInt | the id of the request |
| start | VarInt | the start of the range of the list |
| length | VarInt | the end minus the start of the range |
| suggestions | Array of suggestion | |

A suggestion:

| Field | Type |
|-------|------|
| text | String (32767) |
| has tooltip | Boolean |
| tooltip | Text component, present when has tooltip is true |

When there is no suggestion, the reply has the range `[0, 0]` and an empty array.

Provenance: `source_suggested`. Confidence: medium. Experiment: send a request for `/gamem` as
an operator on a vanilla server. The reply has the start 1 and the length 5, and one
suggestion `gamemode`. A request for `/gamemode` gets no suggestion, because the only match
equals the text (`command-parser.md`, CMD-SUG-005).

### CMDP-SUG-003: Pace

- **Behavior:** the server keeps at most one open request for each player.
  1. A new request replaces an open request that the server has not started. The replaced
     request gets no reply.
  2. The server handles at most one request for each player in one tick. When the player has
     no request handled in the current tick, the server handles a new request at once.
     Otherwise it handles the request in the next tick.
  3. When a new request arrives while the server handles the open one, the open one gets no
     reply.
- **Postconditions:** the client gets a reply to its latest request. A reply to an older
  request is allowed: the client ignores it (CMDP-SUG-001).

Provenance: `source_suggested`. Confidence: medium. Experiment: send three requests in one
packet bundle on a vanilla server. The third request gets a reply. The first two can get no
reply.

---

## 5. Command lines from the client

### CMDP-EXEC-001: `chat_command`

| Field | Type |
|-------|------|
| command | String (32767) |

The command line has no leading `/`. The client sends `chat_command_signed` instead when the
command has a signable argument. The first commands have none. `chat_command_signed` is out of
scope.

Provenance: `source_suggested`. Confidence: medium.

### CMDP-EXEC-002: Checks before the parse

1. If a code unit of the command line is U+00A7, U+007F or less than U+0020, the server
   disconnects the player with `multiplayer.disconnect.illegal_characters`.
2. Otherwise the server handles the command line on the server thread. The command counts
   against the command spam limit of the player. The spam limit is part of the chat
   specification, which does not exist yet.

Provenance: `source_suggested`. Confidence: medium.

### CMDP-EXEC-003: Feedback for an error

- **Inputs:** a command line of length `n`, and an error from `command-parser.md`,
  CMD-PARSE-007.
- **Behavior:** the server sends system messages to the player:
  1. The error text: the translation key of the error with its arguments, in red.
  2. If the error has a position `q`, a second message. Let `k` = the smaller of `q` and `n`.
     The message has these parts, in gray, and a click event `suggest_command` with `/` and the
     command line:
     - the ellipsis text `...`, when `k > 10`;
     - the command line from `max(0, k − 10)` to `k`;
     - when `k < n`: the command line from `k` to the end, in red and underlined;
     - the translation `command.context.here`, in red and italic.
- **Postconditions:** the command does not run.

Provenance: `source_suggested`. Confidence: medium. Experiment: send `gamemode creativ` as an
operator. The first message has the key `argument.gamemode.invalid` with the argument
`creativ`. The second message has `...` and `de creativ`, then the key `command.context.here`. The error position is 16, the end
of the line, so no red part follows.

### CMDP-EXEC-004: Feedback when a command runs

- **Behavior:**
  - A command reports success with a message and a broadcast flag. The player receives the
    message as a system message when the game rule `send_command_feedback` is true.
  - When the broadcast flag is set, every other online operator receives the translation
    `chat.type.admin` with the display name of the source and the message, in gray and
    italic, when `send_command_feedback` is true. The server log receives the same text when
    `log_admin_commands` is true.
  - A command fails with an error with no position. The player receives the error text in red.
  - A command that fails in an unexpected way gives the player the translation
    `command.failed` in red, with the failure text as its hover text.

Provenance: `source_suggested`. Confidence: medium. Experiment: with two operators online, one
runs `gamemode creative`. The other receives `chat.type.admin` in gray italic.

---

## 6. Open questions

- The repository does not store `commands.json`. The update tool can write it next to the
  other reports. The permission table of `command-permissions.md` then gets a cited path.
- The client limits the text of a request to 32500 code units, but the server decodes a
  shorter limit in one case. The case is not known.
- The chat specification must state the command spam limit and the chat visibility rules for
  system messages.
