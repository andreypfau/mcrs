# The `list` command

Version: see `assets/minecraft/version.json`.

This document states the forms `list` and `list uuids`: the tree, the reply and the result. It
does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Online player**: a player in the play state, from the end of its join until its disconnect.
  A connection in the login state or in the configuration state is not an online player.
- **Player list**: the online players of the server in all dimensions, in the order of their
  joins. The first player that joined is first.
- **Player limit**: the maximum count of online players that the configuration of the server
  allows. The default is 20.
- **Profile name**, **profile UUID**: the name and the UUID of the profile that the login gives
  the player (`login.md`).
- **UUID text**: a UUID as 32 lowercase hexadecimal digits in the groups 8, 4, 4, 4 and 12, with
  `-` between the groups, for example `0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0`.
- **Name list**: one text for each player of the player list, in that order, joined with a
  separator (CMD-LIST-003).
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-LIST`.

## 1. Scope

The team decoration of a player name is out of scope. The `team` command has no specification yet.
Each rule of this document applies to a player that is not a member of a team.

---

### CMD-LIST-001: Tree

```
list                                         always_pass; executable
└── uuids                                    executable
```

The check of the root literal is `always_pass` (`command-permissions.md`, PERM-CMD-001). Each
source can run both forms, a player who is not an operator included.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `list` in `commands.json` with the tree above.

### CMD-LIST-002: The player name of `list`

- **Behavior:** in the form `list`, the text of each player is a text component with these parts:
  - the text: the profile name;
  - the click event `suggest_command`, with the command `/tell <profile name> `, with one space at
    the end;
  - the hover event `show_entity`, with the type `minecraft:player`, the profile UUID and the
    profile name as a plain text name;
  - the insertion: the profile name.
- **Postconditions:** the text has no colour and no other style.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: join one player,
run `list` as that player, and capture the `system_chat` packet. Compare the third argument
with the parts above.

### CMD-LIST-003: `list`

- **Behavior:** the command sends one success message with the broadcast flag clear
  (`commands-packet.md`, CMDP-EXEC-004). The message is the translation `commands.list.players`,
  with these arguments in this order:
  1. the count of the player list, as an int primitive;
  2. the player limit, as an int primitive;
  3. the name list of the texts of CMD-LIST-002.
- **Name list:**
  - with no player: an empty text, `{"text":""}`;
  - with one player: the text of that player, with no wrapper;
  - with two or more players: an empty text whose children are the texts of the players. Between
    two texts, a separator `, ` in the colour `gray` is a child.
- **Postconditions:** the result is the count of the player list. The reply contains each online
  player, also a player in spectator mode and the source player.
- **Edge cases:** with no player, the English text ends with `online: ` and one space.

Provenance: `source_suggested`. Kind: semantic; the key and the argument order are wire.
Confidence: medium. Experiment: start a server with no player and the player limit 20. Run `list`
from the console. The reply is `There are 0 of a max of 20 players online: `. Run
`execute store result storage t:t n int 1 run list`, then `data get storage t:t n`. The stored
value is 0.

### CMD-LIST-004: `list uuids`

- **Behavior:** the command is the same as `list` (CMD-LIST-003), with one difference. The text of
  each player is the translation `commands.list.nameAndId`, with two arguments:
  1. the profile name, as a plain text with no event and no insertion;
  2. the UUID text of the profile UUID, as a plain text.
- **Postconditions:** the result is the count of the player list. With no player, the reply is
  the same as the reply of `list`.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: medium. Experiment: start a server with no player. Run `list uuids` from the console.
The reply is `There are 0 of a max of 20 players online: `. Join one player, and run `list uuids`. The
second argument of `commands.list.nameAndId` is the UUID text of that player.

### CMD-LIST-005: The order of the player list

- **Behavior:** the name list has the players in the order of their joins, the first join first.
  A player that changes dimension or respawns keeps its position. A player that disconnects
  leaves the list. When it joins again, it goes to the end.

Provenance: `design`. Kind: deviation (CMD-LIST-DEV-001). Confidence: high.

---

## Intentional deviations

### CMD-LIST-DEV-001: A respawn does not move the player

- **Rule:** CMD-LIST-005.
- **Reference:** the game removes a player from its player list at a respawn, and adds the player
  again at the end. A death, and the return from the End through the exit portal, thus change
  the order of the names. The same accident changes the order of selectors
  (`argument-entity.md`, ARG-ENT-DEV-007).
- **Reason:** the order of joins is stable and has a meaning for a player. A respawn is not a new
  join.
- **Visible to the client:** yes. After a respawn, the name list of the game shows that player
  last.
- **Bug reference:** none.
- **Test:** a differential test compares the name list as a set when a player respawned after
  another player joined. Otherwise it compares the order.

## Unresolved

None.
