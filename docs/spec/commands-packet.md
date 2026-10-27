# The command protocol: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states what a server and a client exchange about commands in the PLAY state.
The subjects are the command tree that a player can see and the suggestions that the client
asks for. The subjects are also the command line that the client sends and the feedback that
the server returns. The document does not prescribe an architecture.

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
- **Restricted**: a node whose permission check is `require`, with any permission
  (`command-permissions.md`, PERM-SET-002).
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
`command-parser.md`, section 0.2. The kinds of rule are those of `README.md`, section
"Semantics, not accidents".

### 0.4 Artefacts cited

- `assets/mcrs/reports/packets.json`: packet ids.
- `assets/mcrs/reports/registries.json`: the parser ids, in the registry
  `minecraft:command_argument_type`.
- `commands.json`: the command report that the game writes with `--reports`. The task
  `dumpReports` of `tools/vanilla-oracle` writes it to `reports/commands.json` in its output
  directory. The repository does not store it yet (section 7).

### 0.5 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CMDP-TREE` | the player tree and when it is sent |
| `CMDP-PKT` | the layout of the `commands` packet |
| `CMDP-ARG` | parser ids, properties and suggestion types |
| `CMDP-SUG` | the suggestion exchange |
| `CMDP-EXEC` | the command line from the client, and the feedback |
| `CMDP-DEV` | intentional deviations from the reference |

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
- **Behavior:**
  1. The player tree has a new root. The root of the full tree maps to that root.
  2. A node of the full tree is in the player tree when two conditions are true. Its
     requirement passes for the source, and its parent is in the player tree.
  3. A node of the player tree has the fields of its node in the full tree. These fields are
     the kind, the name, the argument type, the suggestion type, the command and the
     requirement.
  4. The children of a node of the player tree keep the order of the full tree.
  5. A node keeps its redirect when the target is usable by the source (`command-parser.md`,
     CMD-TREE-004). A usable target is in the player tree. The order of a visit of the tree has
     no effect on this result (CMDP-DEV-002).
- **Postconditions:** a redirect to the root always stays.
- **Edge cases:** when the target of a redirect is not usable by the source, the node has no
  redirect in the player tree. The node is executable only if it has a command. The server
  parse must not follow that redirect for that player (`command-parser.md`, CMD-TREE-004).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a vanilla
server, give a player level 0 and capture `commands`. The capture has no `tp` and no `teleport`.
Give the player level 2 and capture again. The node `tp` has the redirect flag, and its target
is `teleport`.

### CMDP-TREE-002: When the server sends the tree

- **Behavior:**
  - The server sends a `commands` packet with the player tree when the player enters PLAY.
  - The server sends the player tree again when the player tree of that player changes. An
    example is a change of the permission level by `op` or `deop`.
  - The server sends the permission level of the player (`command-permissions.md`,
    PERM-LEVEL-003) at four times. These are the entry into PLAY, each respawn, each change of
    dimension and each change of the level.
- **Edge cases:** the client makes a new player object at a respawn and at a change of
  dimension. The new player object has no permissions until the client receives the level.
  Without the level, the client rejects entity selectors that start with `@`.
- **Postconditions:** the server can send the tree at other times. The order of the level and
  the tree is unspecified.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.
Experiment: on a modified server that does not send the level after a respawn, give a player
level 2, kill the player, and respawn. Type `/tell @a hi`. The client underlines `@a` with the
key `argument.entity.selector.not_allowed`.

### CMDP-TREE-003: Node numbers

- **Inputs:** the player tree.
- **Behavior:** give each node of the player tree one number, from 0 to the count of nodes
  minus 1. Each number belongs to one node.
- **Postconditions:** the list of wire nodes is in the order of the numbers. The root index is
  the number of the root. The choice of numbers is unspecified apart from these conditions.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: with a modified
server, send the vanilla player tree with the numbers in reverse order. The client accepts the
packet, and its suggestions do not change.

### CMDP-TREE-004: The restricted flag

- **Behavior:** the server sets the flag `0x20` on each restricted node of the player tree.
- **Postconditions:** the client gives each node with the flag a requirement. Only a source
  with the client atom `minecraft:client/commands/restricted` passes that requirement. The
  source of the client for typed commands holds that atom.
- **Edge cases:** the client tests a command line that the player did not type, for example
  from a click event or a dialog:
  1. The client parses the line with its own source. When the line is not valid
     (`command-parser.md`, CMD-PARSE-007), the client asks the player to confirm it.
  2. When the line has a signable argument, the client asks the player to confirm it. The
     next test does not run.
  3. The client parses the line with a source that has no permissions. When the line is not
     valid, the client asks the player to confirm it.

The confirmation screens are internal to the client. They are not part of the behaviour of the
server.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: send a system
message with a `run_command` click event for `/gamemode creative` to an operator. A click opens
the confirmation screen with the text of `multiplayer.confirm_command.permissions_required`.

---

## 2. The `commands` packet

### CMDP-PKT-001: Layout

| Field | Type | Notes |
|-------|------|-------|
| nodes | Array of wire node | in the order of the numbers (CMDP-TREE-003) |
| root index | VarInt | the number of an entry of the kind root |

A wire node:

| Field | Type | Present when |
|-------|------|--------------|
| flags | Byte | always |
| children | Array of VarInt | always; the numbers of the children, in the order of the player tree |
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
| `0x40`, `0x80` | the server must write 0 |

The order of the children is part of the meaning. The client tries the argument children of a
node in that order.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: decode a captured
`commands` packet with this layout, and check that the decode ends at the last byte.

### CMDP-PKT-002: Trees that the client rejects

The client rejects the packet when one of these is true:

- When only the redirect links are followed, a chain has no end at a node without a redirect.
  An example is a node that redirects to itself.
- When only the child links are followed, they form a cycle.
- A child index or a redirect index is not less than the number of nodes.
- The root index does not point to an entry of the kind root.

A redirect to an ancestor, or to the root, is not a cycle in this sense.

A server must not send a parser id that is not in the registry (CMDP-DEV-004).

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: send a tree whose
node 1 redirects to node 1, with a modified server. The client disconnects.

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
| `minecraft:block_pos` | 8 | none |
| `minecraft:vec3` | 10 | none |
| `minecraft:block_predicate` | 13 | none |
| `minecraft:rotation` | 29 | none |
| `minecraft:swizzle` | 32 | none |
| `minecraft:resource_location` | 36 | none |
| `minecraft:entity_anchor` | 38 | none |
| `minecraft:dimension` | 41 | none |
| `minecraft:gamemode` | 42 | none |
| `minecraft:time` | 43 | Int: the minimum, in ticks |
| `minecraft:resource_or_tag` | 44 | Identifier: the registry |
| `minecraft:resource_or_tag_key` | 45 | Identifier: the registry |
| `minecraft:resource` | 46 | Identifier: the registry |
| `minecraft:heightmap` | 51 | none |

A number range is a Byte of flags, then the minimum when flag `0x01` is set. The maximum follows
when flag `0x02` is set. Each bound has the type of the table. A bound is present when it
differs from the default of `command-parser.md`, CMD-ARG-002.

The Int of `minecraft:time` is 4 bytes, big-endian, and is always present.
`command-time.md`, CMD-TIME-001, lists the values that the `time` command sends for `time` and
`resource`. The registry of `resource`, `resource_or_tag` and `resource_or_tag_key` is the
identifier of the registry, for example `minecraft:timeline`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`) for the ids.
`source_suggested` for the properties. Kind: wire. Confidence: medium. Experiment: decode the
`gamemode` node of a captured `commands` packet. Its `target` child has parser id 6 and the
property byte `0x02`. Then decode the node `time set <time>`. Its parser id is 43, and its
property is the Int `0`.

### CMDP-ARG-002: Suggestion types

| Suggestion type | Who makes the suggestions |
|-----------------|---------------------------|
| `minecraft:ask_server` | the server, through CMDP-SUG |
| `minecraft:available_sounds` | the client, from the sound events that it knows |
| `minecraft:summonable_entities` | the client, from the entity types that `summon` can make |
| `minecraft:post_effects` | the client, from the post effects that it knows |

The server writes `minecraft:ask_server` on an argument whose suggestions the client cannot
make from its own data. The client treats an unknown suggestion type as
`minecraft:ask_server`. An argument without a suggestion type gets the suggestions of its type,
which the client makes itself.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment:
find an argument with flag `0x10` in a captured `commands` packet, and read its identifier.

### CMDP-ARG-003: A type that the client parses differently

The client parses each argument with the type of its parser id. The server can parse an argument
with a grammar that accepts a different set of command lines. Then the client underlines command
lines that the server runs, or the reverse. A server that has an argument type with no parser id
must send a type whose grammar the client applies in the same way. `brigadier:string` in the
mode `greedy`, with the suggestion type `minecraft:ask_server`, accepts every rest of a line.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: with a
modified server, send a root literal `x` with a child `brigadier:string` in the mode `greedy`
and the suggestion type `minecraft:ask_server`. Type `/x a "b @c`. The client underlines
nothing.

---

## 4. Suggestions from the server

### CMDP-SUG-001: Request

`command_suggestion`:

| Field | Type | Notes |
|-------|------|-------|
| id | VarInt | chosen by the client |
| text | String (32500) | the command line up to the cursor of the client |

- **Behavior:**
  - The server must accept a text of 256 code units from every player.
  - The server must accept a text of 32500 code units from a player who can edit command
    blocks. That player is in creative mode and has the level `gamemasters` or higher.
  - The server can drop a request with a longer text. It then sends no reply.
  - The server answers each other request with the suggestions of its own parse at the end of
    the text (CMDP-SUG-002). CMDP-SUG-003 states when the server can drop a request.
  - The kind of the argument at the end of the text has no effect on the answer.
- **Postconditions:** the text starts with `/` when the input of the chat screen starts with
  `/`. The client keeps one request open. A new request replaces it. The client takes only a
  reply whose id equals the id of the open request.

The limits agree with the client. The chat screen holds at most 256 code units, and the editor
of a command block holds at most 32500. The client sends a request when its parse reaches one
of these arguments:

- an argument with the suggestion type `minecraft:ask_server`, or an unknown one;
- an argument of a registry that the client does not have;
- an argument of type `minecraft:objective`.

`client-commands.md`, section 4, states when the client sends a request, its ids, and how the
client treats a late reply.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment:
with a tree that has one argument with the suggestion type `minecraft:ask_server`, type `/x a`
and capture the request. Its text is `/x a`.

### CMDP-SUG-002: Reply

- **Behavior:**
  1. If the text starts with `/`, the parse starts at position 1. Otherwise it starts at 0.
  2. Parse the text with the source of the player (`command-parser.md`, section 4).
  3. Collect the suggestions at the end of the text (`command-parser.md`, section 6).
  4. Send `command_suggestions`.
- **Postconditions:**
  - All positions count from the start of the text, the `/` included.
  - Let `n` be the number of suggestions of step 3. The reply holds at least the first
    `min(n, 1000)` suggestions, in the order of `command-parser.md`, section 6.
  - The server must limit the number of suggestions in one reply. The limit is 1000 or more.

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

When there is no suggestion, the range is unspecified and the array is empty.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send a request
for `/gamem` as an operator on a vanilla server. The reply has the start 1 and the length 5, and
one suggestion `gamemode`. A request for `/gamemode` gets no suggestion, because the only match
equals the text (`command-parser.md`, CMD-SUG-005).

### CMDP-SUG-003: Pace

- **Behavior:**
  - The server sends a reply to the latest request of a player.
  - The server can drop a request that a newer request of the same player replaces.
  - The server must limit the work on requests for each player in each tick.
- **Postconditions:** a reply to an older request is allowed. The client ignores it
  (CMDP-SUG-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send three
requests in one network write, before the next tick of a vanilla server. The third request
gets a reply.

---

## 5. Command lines from the client

### CMDP-EXEC-001: `chat_command`

| Field | Type |
|-------|------|
| command | String (32767) |

The command line has no `/` at the start. The client sends `chat_command_signed` instead when the
command has a signable argument. The first commands have none. `chat_command_signed` is out of
scope. `client-chat.md`, CHAT-SEND-004, states when the client picks each packet, and the
layout of `chat_command_signed` in an unsigned session.

When the server enforces secure profiles and the parse has a signable argument, the server
does not run a `chat_command`. It sends the player the translation
`chat.disabled.invalid_command_signature` as a system message. The message argument of `ban`,
`ban-ip`, `kick`, `me`, `msg`, `say` and `teammsg` is signable.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: type `/gamemode
creative` and capture the packet. It is `chat_command` with the text `gamemode creative`. On a
server that enforces secure profiles, send `chat_command` with the text `say hi`. The reply has
the key `chat.disabled.invalid_command_signature`, and nothing is broadcast.

### CMDP-EXEC-002: Checks before the parse

1. If a code unit of the command line is U+00A7, U+007F or less than U+0020, the server
   disconnects the player with `multiplayer.disconnect.illegal_characters`.
2. Otherwise the server runs the command line. The command counts against the command spam
   limit of the player. The spam limit is part of the chat specification, which does not exist
   yet.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with a modified
client, send `chat_command` with the text `say a§b`. The server disconnects the client with the
key `multiplayer.disconnect.illegal_characters`.

### CMDP-EXEC-003: Feedback for an error

- **Inputs:** a command line of length `n`, and an error from `command-parser.md`,
  CMD-PARSE-007.
- **Behavior:** the server sends system messages to the player:
  1. The error text: the translation key of the error with its arguments, in red.
  2. If the error has a position, a second message with the context text of
     `command-parser.md`, CMD-ERR-003. The message is in gray, with a click event
     `suggest_command` with `/` and the command line. When the position is before the end, the
     command line from the position to the end follows, in red and underlined. The translation
     `command.context.here` follows last, in red and italic.
- **Postconditions:** the command does not run.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send `gamemode
creativ` as an operator. The first message has the key `argument.gamemode.invalid` with the
argument `creativ`. The second message has `...` and `de creativ`, then the key
`command.context.here`. The error position is 16, the end of the line, so no red part follows.

### CMDP-EXEC-004: Feedback when a command runs

- **Behavior:**
  - A command reports success with a message and a broadcast flag. The player receives the
    message as a system message when the game rule `send_command_feedback` is true.
  - When the broadcast flag is set and `send_command_feedback` is true, every other online
    operator receives a message. It is the translation `chat.type.admin` with the display name
    of the source and the message, in gray and italic. The server log receives the same text when
    `log_admin_commands` is true.
  - A command fails with an error with no position. The player receives the error text in red.
  - A command that fails in an unexpected way gives the player the translation
    `command.failed` in red. The hover text of that message is unspecified.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with two
operators online, one runs `gamemode creative`. The other receives `chat.type.admin` in gray
italic.

---

## 6. Intentional deviations

### CMDP-DEV-001: Nodes that compare equal share one number

- **Rule:** CMDP-TREE-003.
- **Reference:** the reference identifies nodes by node equality, not by identity, when it
  numbers them and when it finds the copy of a redirect target. Two nodes are equal when they
  have the same kind, literal text or name, argument type and command, and equal children of
  the same names. The order of the children, the redirect, the requirement and the suggestion
  type are not compared. Two different nodes that are equal get one number.
- **Visible to the client:** yes, when two merged nodes differ in the redirect target, the
  restricted flag or the suggestion type. The client then gets the fields of one node in both
  places. Otherwise no.

  In the vanilla tree, only the number types of Brigadier compare by value. The other argument
  types compare by instance, and each call of a factory makes a new instance. These vanilla
  nodes are equal:
  - `execute run` and `return run`: the literal `run`, with a redirect to the root.
  - Below `execute store result` and `execute store success`: the literals `value` and `max`
    after `bossbar <id>`, each with a redirect to `execute`.
  - Below the same two literals: the type literal with one name after each `<path>`, for the
    names `byte`, `double`, `float`, `int`, `long` and `short`. Their children `<scale>`, of
    the type `brigadier:double`, are also equal. Each `<scale>` redirects to `execute`.

  The nodes of each merged set have the same fields on the wire, so the client cannot see
  these merges. No visible merge in the vanilla tree is known.
- **Test:** a comparison of the decoded trees reports a difference in the count of nodes as
  this deviation. A comparison of the bytes does not apply (CMDP-TREE-003).

### CMDP-DEV-002: A redirect to a later node is dropped

- **Rule:** CMDP-TREE-001, behavior 5.
- **Reference:** the reference makes the player tree with a depth-first visit. A redirect
  stays only when the visit has already copied its target. A redirect to a node that the visit
  reaches later is dropped, even when the player can use that node.
- **Visible to the client:** yes. The client cannot parse past the node. No vanilla tree has
  such a redirect.
- **Test:** a redirect that the reference tree does not have, and whose target comes later in a
  depth-first visit, is this deviation.

### CMDP-DEV-003: The server parse follows a redirect to a hidden target

`command-parser.md`, CMD-DEV-004, records this deviation for CMDP-TREE-001. No vanilla tree has
such a redirect: each target has the check of the node that redirects to it, or is the root.

### CMDP-DEV-004: An unknown parser id is misread

- **Rule:** CMDP-PKT-002.
- **Reference:** the client reads a node with an unknown parser id as a root. It does not read
  the properties or the suggestion type of that node, and it misreads the bytes that follow.
- **Visible to the client:** no. The server never sends such an id.
- **Test:** no test sends such an id.

---

## 7. Open questions

- The repository does not store `commands.json`. The update tool can write it next to the
  other reports. The permission table of `command-permissions.md` then gets a cited path.
- The chat specification must state the command spam limit and the chat visibility rules for
  system messages.
