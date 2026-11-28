# Player death: the death screen, the drops, the respawn and the equipment sync

`assets/mcrs/reports/packets.json`.

This document states what occurs after the health of a player reaches 0. It states the death
screen, the death message, the drops, and the respawn request with its result. It also states the
packets that tell other players which items an entity wears. `entity-damage.md`, section 8, states the death of
a living entity. This document does not repeat that section. It states only what is different or
new for a player. `item-entity.md` states the item entities of the drops. `dimension-change.md`
states the packets of a change of dimension. This document refers to both.

The rules come from the reference implementation and from public documentation.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **tracking players**, **entity**, **network id**: as
  `entity-damage.md`, section 0.1, and `entity-tracking.md`, section 0.1, define them.
- **Fatal hit**: the hit that takes the health of an entity to 0.
- **Dying player**: a player whose health reached 0 in the current tick.
- **Dead player**: a player whose health is 0 and whose respawn request the server did not yet
  accept.
- **Death position**: the position of the player at the moment of the death.
- **Respawn point**: the data that a player stores to select the place of its next respawn. It has
  a dimension, a block position, a yaw, a pitch and a flag `forced`. A player can have no respawn
  point. How a player gets a respawn point is out of scope.
- **World spawn**: the dimension, the block position, the yaw and the pitch of the spawn of the
  world. The game rule `respawn_radius` and the world border limit the use of the world spawn.
- **Bottom centre** of a block position `(x, y, z)`: the point `(x + 0.5, y, z + 0.5)`. The feet
  of a player at that point are on the lower face of the block.
- **Minimum height**, **maximum height**: the y of the lowest and of the highest block of a
  dimension. In the overworld they are -64 and 319.
- **Clear position**: a block position that PLD-RSP-006 accepts for the box of a player.
- **Height fix**: the vertical adjustment of PLD-RSP-006.
- **Spawn placement**: the procedure of PLD-RSP-005. It gives a position from a dimension and a
  start block `S`. The respawn with no valid respawn point and the join of a player with no
  position both use it.
- **Respawn request**: the packet `client_command` with the action `perform_respawn`
  (PLD-RSP-001).
- **Respawned player**: the player after the server accepts a respawn request. It has the same
  network id, the same UUID and the same connection as the dead player. It is a new object for
  all data that PLD-RST-001, PLD-RST-003 and PLD-RST-005 list as kept.
- **Death message**: a text component that states how the player died (PLD-MSG-001).
- **Combat record**: the list of the last hits on a player, kept for the death message
  (PLD-MSG-005).
- **Death screen**: the screen of the vanilla client after its player dies.
- **Equipment slot**: one of `mainhand`, `offhand`, `feet`, `legs`, `chest`, `head`, `body` and
  `saddle`.
- **Last sent stack**: the stack that the server last told the tracking players for one equipment
  slot of one entity (PLD-EQP-004).
- **Viewer**: a player in whose tracking range an entity is, as `entity-tracking.md` defines it.
- **Game rule**: as `command-gamerule.md` defines it.

### 0.2 Provenance

The labels are those of `registry.md`, section 0.3. Each rule states its kind and its
confidence, as `docs/spec/README.md` defines them.

### 0.3 ID prefix

Each rule has an ID with the prefix `PLD`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `PLD-DTH` | the death of a player: what differs from a living entity |
| `PLD-SCR` | the death screen and the game rules that control it |
| `PLD-MSG` | the death message and its delivery |
| `PLD-DRP` | the drops of a death |
| `PLD-RSP` | the respawn request and the respawn position |
| `PLD-RST` | the state of the respawned player |
| `PLD-NET` | the packets of a respawn |
| `PLD-EQP` | the equipment sync |
| `PLD-DEV` | intentional deviations |

### 0.4 Artefacts cited

- The game rule table of `crates/mcrs_minecraft_game_rule`: `immediate_respawn` (default false),
  `show_death_messages` (default true), `keep_inventory` (default false), `respawn_radius`
  (default 10, minimum 0), `forgive_dead_players` (default true).
- `assets/mcrs/reports/packets.json`: the packet ids.
- The environment attributes `gameplay/bed_rule`, `gameplay/straw_bed_rule` and
  `gameplay/respawn_anchor_works` of `environment-attributes.md`.
- The language file of the game: the keys `death.attack.*`, `death.fell.*`, `deathScreen.*` and
  `block.minecraft.spawn.not_valid`. Each key goes to the client as a translation key. The client
  holds the text.

## 1. Death of a player

**PLD-DTH-001** (`source_suggested`, semantic, confidence: medium). The death of a player follows
DMG-DTH-001, with these differences. For a player, the steps run in this order:

1. The server sends the kill packet to the dying player (PLD-SCR-001), and the death message to
   the chat (PLD-MSG-006).
2. The server drops the items and the experience, unless the player is in spectator mode
   (section 4).
3. The server sends the entity event 3 to the tracking players (DMG-NET-003).
4. The server clears the fire of the player and sets its last death location to the dimension
   and the block position of the death.

The viewers receive the spawn of the item entities (step 2) before the entity event 3. This is the
order of DMG-NET-003. The kill packet and the chat message go only to the connection of the
dying player and to the players of PLD-MSG-006.

**PLD-DTH-002** (`source_suggested`, semantic, confidence: medium). A dead player stays in the
player list and keeps its connection. In each tick of the dead state, the death time of the
player increases by 1 (DMG-DTH-003). When it reaches 20, the server sends the entity event 60 to
the tracking players and removes the entity from the world, as DMG-DTH-003 states. The viewers
receive `remove_entities` (TRK-REM-001). The connection is not closed. The dead player can still
send a respawn request, also after this removal.

**PLD-DTH-003** (`source_suggested`, semantic, confidence: medium). From the tick of the death to
the end of the load window after the respawn, the server treats the client as "not loaded". The
load window ends when the client sends `player_loaded`, or 60 ticks after the server accepts the
respawn request. In this state the server ignores the requests that BRK-REQ-005 lists, and it
ignores the move and the use requests of the player. A click on a container menu of a dead
player changes nothing. The server sends the content of that menu again to the client.

**PLD-DTH-004** (`source_suggested`, semantic, confidence: medium). The death makes these changes
to the dying player, in addition to DMG-DTH-001:

- an entity that sits on the shoulder of the player leaves the shoulder;
- a neutral mob within 32 blocks horizontally and 10 blocks vertically of the death position
  is told that the player died. This occurs when the game rule `forgive_dead_players` is true.

Neutral mobs are out of scope. This rule is a hook.

## 2. The death screen

**PLD-SCR-001** (`source_suggested`, wire, confidence: high). The server sends `player_combat_kill`
(play clientbound 69) one time to the dying player. The fields are:

| Field | Type | Value |
|-------|------|-------|
| player id | VarInt | the network id of the dying player |
| message | text component | the death message when `show_death_messages` is true; otherwise the empty text component |

The server sends the packet also when `show_death_messages` is false. In that case the message
is empty. The server does not send the packet to other players.

**PLD-SCR-002** (`source_suggested`, client agreement, confidence: high). When the client reads
the packet and the id is the id of its own player, it acts in one of two ways:

- When the client shows the death screen (PLD-SCR-003), it opens the screen with the message of
  the packet.
- Otherwise it sends the respawn request at once, and it opens no screen.

The client ignores the packet when the id is not its own player.

**PLD-SCR-003** (`source_suggested`, wire, confidence: high). The game rule `immediate_respawn`
controls the death screen. The server tells the client in two ways:

- The `login` packet has the Boolean "enable respawn screen". Its value is the negation of the
  rule at the moment of the login.
- When the rule changes, the server sends `game_event` (play clientbound 39) to each player. The
  event is 11 (immediate respawn). The value is 1.0 when the rule is now true, and 0.0 when it
  is now false.

The client shows the death screen when the value is 0.0 or when the Boolean is true. The server
does not send the event at a respawn. A change of the rule while a player is dead takes effect
for the next death.

**PLD-SCR-004** (`documentation`, client agreement, confidence: medium). The death screen of the
vanilla client shows these items:

- The title: the translation `deathScreen.title`, or `deathScreen.title.hardcore` when the world
  is hardcore.
- The death message of the packet, below the title.
- The score: the translation `deathScreen.score.value` with the experience score of the player.
- A button that sends the respawn request. Its text is `deathScreen.respawn`, or
  `deathScreen.spectate` in a hardcore world.
- A button `deathScreen.titleScreen` that leaves the world.

The buttons are not active in the first 20 ticks of the screen. The server does not check the
time between the death and the respawn request (PLD-RSP-001). Layout is out of scope.

## 3. The death message

**PLD-MSG-001** (`source_suggested`, semantic, confidence: medium). The death message is a
translatable text component. Let `V` be the display name of the victim. Let `msg_id` be the field
`message_id` of the damage type of the fatal hit. The server selects the component in this
order:

1. When the combat record is empty: the key `death.attack.generic` with `V`.
2. When the damage type has the death message type `intentional_game_design`: the key
   `death.attack.<msg_id>.message` with two arguments. The first is `V`. The second is the text of
   the key `death.attack.<msg_id>.link` in square brackets. That second text has the click event
   "open the URL of the bug report MCPE-28723" and the hover text `MCPE-28723`.
3. When the damage type has the death message type `fall_variants` and the combat record has a
   significant fall (PLD-MSG-003): the fall message of PLD-MSG-003.
4. Otherwise: the message of the damage source (PLD-MSG-002).

The death message type is a field of the data-pack registry `minecraft:damage_type`. The values
are `default`, `fall_variants` and `intentional_game_design`.

**PLD-MSG-002** (`source_suggested`, semantic, confidence: medium). The message of a damage source
has the base key `death.attack.<msg_id>`. It is selected in this order:

1. The source has no causing entity and no direct entity. Let `K` be the kill credit of the
   player (`player-attack.md`). When `K` exists, the key is the base key plus `.player`, with the
   arguments `V` and the display name of `K`. Otherwise the key is the base key, with `V`.
2. The source has a causing entity or a direct entity. Let `N` be the display name of the causing
   entity, or of the direct entity when there is no causing entity. Let `H` be the main-hand stack
   of the causing entity, when it is a living entity. When `H` is not empty, it can have a custom name. Then
   the key is the base key plus `.item`, with `V`, `N` and the display name of `H`. Otherwise the
   key is the base key, with `V` and `N`.

**PLD-MSG-003** (`source_suggested`, semantic, confidence: medium). The combat record has one
entry for each hit. An entry has four values at the time of the hit. These are the damage source, the damage, the
fall distance of the victim and the fall location of the victim. The fall location is, in this order:

- when the victim last touched a climbable block: the kind of that block, as the table below
  states;
- otherwise, when the victim is in water: `water`;
- otherwise none.

| Climbable block | Fall location |
|-----------------|---------------|
| `ladder`, or a block with the tag `trapdoors` | `ladder` |
| `vine` | `vines` |
| `weeping_vines`, `weeping_vines_plant` | `weeping_vines` |
| `twisting_vines`, `twisting_vines_plant` | `twisting_vines` |
| `scaffolding` | `scaffolding` |
| any other climbable block | `other_climbable` |

The server selects the significant entry in these steps:

1. For each entry, let the fall value be the fall distance. When the damage type of the entry has
   the tag `always_most_significant_fall`, the fall value is the largest f32 value.
2. Consider the entries whose damage type has the tag `is_fall` or `always_most_significant_fall`
   and whose fall value is more than 0. Take the entry with the largest fall value. The first
   of equal entries wins. When that entry is not the first entry of the record, the significant
   entry is the entry before it.
3. Let `alternative` be the entry with a fall location and the largest damage. The first of equal
   entries wins.
4. When the largest fall value is more than 5.0, the significant entry is the result of step 2.
   Otherwise, when `alternative` exists and its damage is more than 5.0, it is `alternative`.
   Otherwise there is no significant fall.

The fall message for a significant entry `E` and the fatal hit `S` is:

- When the damage type of `E` has the tag `is_fall` or `always_most_significant_fall`: the key
  `death.fell.accident.<fall location of E>` with `V`. The fall location `generic` replaces a
  fall location that is missing.
- Otherwise, let `A` be the display name of the entity of `E`, and `C` the display name of the
  entity of `S`. When `A` exists and is not equal to `C`: the key `death.fell.assist`, or
  `death.fell.assist.item` as PLD-MSG-002 selects the item form. The arguments are `V` and `A`,
  and for the item form also the display name of the main-hand stack of the entity of `E`.
- Otherwise, when `C` exists: the key `death.fell.finish`, or `death.fell.finish.item`. The
  arguments are `V` and `C`, and for the item form also the display name of the main-hand stack of
  the entity of `S`.
- Otherwise: the key `death.fell.killer` with `V`.

**PLD-MSG-004** (`source_suggested`, semantic, confidence: medium). The tags of the damage types
that PLD-MSG-003 reads are `is_fall` and `always_most_significant_fall`. `entity-damage.md`,
section 0.6, lists `is_fall` for `fall`. The corpus of this repository does not hold the tag
`always_most_significant_fall`. Section 12 (U6) lists this as unresolved.

**PLD-MSG-005** (`source_suggested`, semantic, confidence: medium). The combat record of a player
starts empty. The server adds an entry for each hit that the hit procedure of `entity-damage.md` applies. The
server clears the record in the first tick in which the player is dead. It also clears it in the
first tick in which this much

time passed since the last entry:

- 300 ticks, when the player is in combat;
- 100 ticks, otherwise.

A player is in combat from a hit whose source has a living causing entity or direct entity. The
combat state ends when the record clears. The packets `player_combat_enter` (play clientbound 68)
and `player_combat_end` (play clientbound 67) are out of scope. The server composes the death
message before it clears the record for the death.

**PLD-MSG-006** (`source_suggested`, semantic, confidence: medium). When `show_death_messages` is
true, the server sends the death message as a chat system message (not an overlay) to these
players. The visibility rule of the team of the dying player selects them:

| The team of the dying player | Receivers |
|------------------------------|-----------|
| none, or visibility `always` | all players |
| visibility `hideForOtherTeams` | the players of that team |
| visibility `hideForOwnTeam` | all players that are not in that team |
| visibility `never` | none |

The dying player also receives the message in the kill packet (PLD-SCR-001). When
`show_death_messages` is false, the server sends no chat message.

## 4. The drops

`item-entity.md` states the item entities of a player death. IEN-SPN-001 (row "death of a
player") states the start values. IEN-SPN-002 states the scatter velocity. IEN-SPN-009 states the
order of the stacks. This section states the conditions and the cases that those rules do not state.

**PLD-DRP-001** (`source_suggested`, semantic, confidence: medium). The server drops the items of
a dying player in this case only. The player is not in spectator mode and the game rule
`keep_inventory` is false. In each other case the server drops no item of the inventory, and it
deletes no item. The items stay in the inventory, and PLD-RST-003 states what the respawned player
gets.

**PLD-DRP-002** (`source_suggested`, semantic, confidence: medium). In the case of PLD-DRP-001, the
server first deletes each stack that has an enchantment with the effect
`minecraft:prevent_equipment_drop`. The vanilla enchantment with that effect is
`vanishing_curse`. The server checks each slot of the inventory. These are the 36 main slots, the off-hand
slot, the four armor slots, the body slot and the saddle slot. A deleted stack makes no item
entity. Then the server drops the stacks in the order of IEN-SPN-009. A player who has
`keep_inventory` keeps a stack with this enchantment.

**PLD-DRP-003** (`source_suggested`, semantic, confidence: medium). Each non-empty stack makes one
item entity. The count of the stack does not change, and the server does not split or merge
stacks. The item entity gets the start values of IEN-SPN-001 for the death of a player. It starts
at the x and z of the death position and at the y of the death position plus `eye height - 0.3`. Its velocity is the scatter
velocity of IEN-SPN-002. Its pickup delay is 40 ticks. It has no thrower and no owner. A viewer
receives its spawn as IEN-NET-001 states.

**PLD-DRP-004** (`source_suggested`, semantic, confidence: medium). The server also drops
experience for the death of a player in this case only. The player is not in spectator mode and
`keep_inventory` is false. The amount is `min(7 × level, 100)`, where `level` is the experience
level of the player at the death. The server drops the amount as experience orbs at the death
position, after the items. `experience.md` states the split of the amount
into orbs. The drop does not depend on the last hit by a player and does not depend on the game
rule `mob_drops`. An enchantment of the weapon of the causing entity can change the amount. That change
is out of scope.

**PLD-DRP-005** (`source_suggested`, semantic, confidence: medium). The stack on the cursor of
the player and the stacks of the 2 × 2 crafting grid are not part of the inventory. The server
does not drop them at the death. It drops them when the entity of the dead player is removed
from the world. This is at the end of PLD-DTH-002, or at the respawn, whichever comes first. The drop uses the values of the row "the cursor stack and the craft grid when the player leaves" of IEN-SPN-001. It occurs at the position of the removal. The server drops them also when `keep_inventory` is
true, and also in spectator mode. The respawned player does not keep them.

**PLD-DRP-006** (`source_suggested`, semantic, confidence: medium). A dying player that is in
spectator mode drops no loot and no experience. The player keeps its inventory and experience at
the respawn (PLD-RST-003).

## 5. The respawn request

**PLD-RSP-001** (`source_suggested`, wire, confidence: high). The packet `client_command` (play
serverbound 12) has one field: the action, a VarInt enum:

| Value | Action |
|-------|--------|
| 0 | `perform_respawn` |
| 1 | `request_stats` |
| 2 | `request_game_rule_values` |

For the action `perform_respawn`, the server acts in these cases:

- The health of the player is more than 0: the server ignores the request and sends nothing.
- The health is 0: the server respawns the player (PLD-RSP-002 to PLD-NET-001). The respawn
  completes in the tick of the packet or in a later tick (PLD-RSP-007). The server does not
  check the time since the death.

The actions 1 and 2 are out of scope. The request of the player that won the game (the end
credits) is out of scope.

**PLD-RSP-002** (`source_suggested`, semantic, confidence: medium). The server selects the
respawn position in this order:

1. The player has no respawn point, or the dimension of the respawn point is not loaded. The
   respawn position is the adjusted world spawn (PLD-RSP-004). The server sends no message. The
   player keeps its respawn point.
2. The block at the respawn point is a respawn anchor, and all of these are true:
   - the flag `forced` is true, or the charge of the anchor is more than 0;
   - the environment attribute `gameplay/respawn_anchor_works` is true at the anchor;
   - a stand-up position exists around the anchor (PLD-RSP-003).

   The respawn position is the first stand-up position. The yaw faces the anchor. The pitch is 0.
   When the flag `forced` is false, the charge of the anchor decreases by 1.
3. The block at the respawn point is a bed, and its bed rule allows to set the spawn. The bed
   rule is the value of `gameplay/bed_rule` or `gameplay/straw_bed_rule` at the block. Its field
   `can_set_spawn` allows it. A stand-up position must exist around the bed (PLD-RSP-003). The
   respawn position is the first stand-up position. The yaw faces the bed. The pitch is 0.
4. The flag `forced` is true, and neither the block at the point nor the block above it is solid
   or liquid. The respawn position is the centre of the block at the point, 0.1 blocks above
   its lower face. The yaw and the pitch are those of the respawn point.
5. In each other case the respawn point is not valid. The respawn position is the adjusted world
   spawn. The respawned player has no respawn point. Before the packet `respawn`, the server
   sends `game_event` with the event 0 (no respawn block available) and the value 0.0. The client
   shows the message `block.minecraft.spawn.not_valid`.

When step 2 or step 3 finds a block but no stand-up position, the result is step 5. The
server does not try step 4. When the player respawns in a dimension other than the death
dimension, the respawn point can lead to that other dimension. The respawn dimension is the
dimension of the respawn point.

**PLD-RSP-003** (`source_suggested`, semantic, confidence: medium). A stand-up position is a
block position for which all of these are true:

- the position has a floor: the floor height above the lower face of the block is less than 1.0.
  A block with no collision uses the top of the block below it;
- the box of a player at the floor height has no block collision;
- when the first pass applies, neither the block nor the block below it is dangerous;
- the block and the block above it do not have the tag `invalid_spawn_inside`;
- the box is inside the world border.

The server searches in two passes over the same ordered candidates. In the first pass, the
dangerous blocks are not allowed. In the second pass, they are allowed. A dangerous block has the tag `fire`,
or is `lava`, `magma_block`, a lit `campfire`, `lava_cauldron`, `wither_rose`,
`sweet_berry_bush`, `cactus` or `powder_snow`. The server tries the candidates in this order and takes the first
that is valid.

For an anchor at `(x, y, z)`, the offsets `(dx, dy, dz)` are, in this order:

- the eight horizontal offsets `(0,0,-1)`, `(-1,0,0)`, `(0,0,1)`, `(1,0,0)`, `(-1,0,-1)`,
  `(1,0,-1)`, `(-1,0,1)` and `(1,0,1)`;
- the same eight with `dy = -1`;
- the same eight with `dy = +1`;
- `(0, 1, 0)`.

For a bed, let `forward` be the direction in which the block faces. Let `right` be the direction
that is clockwise from `forward`. Let `side` be the opposite of `right` when the yaw of the
respawn point faces `right`, and `right` otherwise. For a bed with no bed block directly below
it, use these horizontal offsets in `(side, forward)` coordinates:
`(1,0)`, `(1,-1)`, `(1,-2)`, `(0,-2)`, `(-1,-2)`, `(-1,-1)`, `(-1,0)`, `(-1,1)`, `(0,1)`, `(1,1)`,
then `(0,0)` and `(0,-1)`. All offsets have `dy = 0`. The server stops at the first valid
position. For a bed with a bed block directly below it (a bunk), the order is different. Section
12 (U7) lists the bunk order as unresolved.

**PLD-RSP-004** (`source_suggested`, semantic, confidence: medium). The adjusted world spawn is the
respawn position when the player has no valid respawn point (PLD-RSP-002, steps 1 and 5). It has
these parts:

- The respawn dimension is the dimension of the world spawn. When the server does not have that
  dimension, the respawn dimension is the overworld.
- The position is the result of the spawn placement (PLD-RSP-005) in the respawn dimension, with
  the start block `S` of this rule.
- The yaw and the pitch are those of the world spawn.

`S` is the block position of the world spawn when that position is inside the world border of the
respawn dimension. A block position is inside the border when its x and z are not less than the
low edges and are less than the high edges. In the other case, `S` is the block that contains the
centre of the border. Its y is one block above the highest block of the heightmap
`MOTION_BLOCKING` of its column. When that column is not loaded, the game uses the minimum height
as the y. The y in that case has the kind unspecified.

The join of a player with no position uses the same spawn placement and the same `S`
(`persistence.md`, SAVE-PLAYER-001, paragraph "Default position"). Experiment PLD-E6 (section 13)
confirms that a respawn with no respawn point gets the result of the spawn placement, as a join
does. The border adjustment of `S` has no experiment.

**PLD-RSP-005** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The spawn
placement gives a position from a dimension and a start block `S`. The default game mode of the
world (`GameType` of `level.dat`) selects the procedure. The game mode of the player has no
effect.

- Spectator: the bottom centre of `S`.
- Adventure: the height fix of `S` (PLD-RSP-006).
- Survival or creative: the bottom centre of the first valid candidate of the search below. When
  no candidate is valid, the height fix of `S`.

Each result is a bottom centre. Its x and z are those of a block plus 0.5. Its y is a whole
number, and the feet of the player are on the lower face of that block.

Let `r` be the value of `respawn_radius`, or 0 when the value is negative. Let `d` be the distance
from the x and z of `S` to the nearest edge of the world border of the dimension, rounded down.
`d` is negative when `S` is outside the border. The search radius is `min(r, d)`. When `d` is 1 or
less, the search radius is 1, also when `r` is 0.

The candidates are the columns of the square of the search radius around `S`. A search radius of
0 gives one candidate, the column of `S`. For a search radius up to 15, the square has at most 961
columns, and the server tests each column one time. The order of the columns is pseudo-random. The
order has the kind unspecified. For a search radius of 8, the game tests only a part of the square
(PLD-DEV-001). The behaviour for a search radius of 16 or more is unresolved (section 12, U4).

A column `(x, z)` is a valid candidate when all of these are true:

1. The top height of the column is not below the minimum height. The top height is the y of the
   highest block of the heightmap `MOTION_BLOCKING`. A column with no such block has the top
   height of the minimum height minus 1. In a dimension with a ceiling, the top height is the
   spawn height of the generator instead. That height is the minimum height plus the number of
   layers for a flat generator, with the height of the dimension as a limit. It is 64 for each
   other generator.
2. The column has no fluid surface at its top. A column has a fluid surface when its world-surface
   height is not above the top height and is above its ocean-floor height. These heights are the y
   of the highest block of the heightmaps `WORLD_SURFACE` and `OCEAN_FLOOR`.
3. The server scans the blocks of the column downward, from the top height plus 1 to the minimum
   height. It stops with no result at the first block that holds a fluid. It stops with a result
   at the first block whose collision shape covers the full top face of the block. The candidate
   position is the block above that block. A scan that finds neither block gives no result.
4. The candidate position is a clear position (PLD-RSP-006).

The server tests a candidate only on a column that is fully generated (PLD-RSP-007). Experiments
PLD-E1, PLD-E4, PLD-E5, PLD-E6 and PLD-E7 (section 13) confirm the three procedures, the search
radius 0 and 1, item 2 and the bottom centre. The order of the candidates and a search radius
from 2 to 15 have no experiment.

**PLD-RSP-006** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The box of a
player at a block position is 0.6 blocks wide and 1.8 blocks high. The centre of its lower face is
the bottom centre of the position. The position is a clear position when the box overlaps none of
these:

- the collision shape of a block, for a collision with no entity;
- a `water` or `lava` block, of each level. Such a block counts as a full cube. Another block
  that holds a fluid, for example a waterlogged slab or `seagrass`, counts only with its
  collision shape;
- the box of an entity that another entity can collide with (`entity-physics.md`,
  PHY-SHP-009). The conditions are those for a colliding entity that is not a happy ghast.

The world border does not count. A box that only touches a shape at a face does not overlap it.

The height fix of the block `S = (sx, sy, sz)` has these steps:

1. Let `y` be `sy`. While the position `(sx, y, sz)` is not clear and `y` is less than the
   maximum height, add 1 to `y`.
2. Subtract 1 from `y`.
3. While the position `(sx, y, sz)` is clear and `y` is more than the minimum height, subtract 1
   from `y`.
4. Add 1 to `y`.

The result is the bottom centre of `(sx, y, sz)`. These results follow from the steps, and the
game gives each of them:

- `S` is clear at the minimum height, and a block 2 blocks above `S` closes the box. Step 1 stops
  at once, and step 3 does not run. The result is `S`, the last clear position (PLD-E2).
- A clear `S` above a column that is clear down to the minimum height, for example a column of
  the void, gives the minimum height plus 1 (PLD-E7).
- No position from `sy` up to the maximum height is clear, and the position one block below the
  maximum height is not clear. The result is the maximum height, and it is not a clear position
  (PLD-E3). The server must give the same result.

A result that is not clear occurs only in one case. Step 1 stops at a position that is not clear,
and the position one block below it is not clear too. Step 1 can stop at a position that is not
clear only at the maximum height or above it. The entity part of a clear position has no
experiment.

**PLD-RSP-007** (`source_suggested`, semantic, confidence: medium). The spawn placement and the
checks of the respawn point in PLD-RSP-002 read columns that can be absent from memory. The server
tests a candidate, a respawn point or the column of `S` only when that column is fully generated.
It loads such a column from the save. When the save does not hold the column, the server generates
it. The result thus does not depend on the columns that were in memory before.

The game completes the respawn in the tick of the request, and that tick waits for the columns. A
server can instead complete the respawn in a later tick, after the columns are ready. The tick of
the completion has the kind unspecified. Until the respawn completes, these are true:

- The player stays a dead player. The server sends it no packet of PLD-NET-001.
- The server sends no position before the final respawn position.
- The server ignores each other respawn request of the player.
- When the player disconnects, its player file holds a dead player.

The world spawn and the respawn point can change while the respawn waits. The server can use the
values of the request tick or of a later tick. This choice has the kind unspecified. When the
respawn completes, the server sends the packets of PLD-NET-001 with the final position. The vanilla
client keeps its death screen until it receives `respawn`. After the first press, the respawn
button of that screen stays inactive.

## 6. The state of the respawned player

**PLD-RST-001** (`source_suggested`, semantic, confidence: medium). The respawned player keeps its
network id, its UUID, its connection, its game profile, its chat session and its entity tags. The
server removes the dead player from the world and from the player list. It adds the respawned
player at the end of the list of the player list of the respawn dimension (ARG-ENT-DEV-007).
The player list shown in the tab list does not change, and the respawn sends no `player_info_update` to any
player. The hardcore change of PLD-RST-005 is a later change.

**PLD-RST-002** (`source_suggested`, semantic, confidence: medium). The respawned player has these
values, whatever the game rules:

| Value | Result |
|-------|--------|
| health | the max health of the new attributes (PLD-RST-004) |
| food level, saturation, exhaustion | 20, 5.0, 0 |
| active effects | none |
| absorption | 0 |
| fire, freeze, air, fall distance | the start values of a new player |
| pose, sprint, sneak, the use of an item, the ride | the start values of a new player |
| item cooldowns | none |
| the combat record | empty |
| the portal cooldown | 0 |
| the position and the rotation | PLD-RSP-002 |
| the game mode and the previous game mode | those of the dead player (PLD-RST-005 for hardcore) |
| the abilities | those that the game mode gives |

**PLD-RST-003** (`source_suggested`, semantic, confidence: medium). The respawned player has the
inventory, the experience level, the experience progress, the total experience and the score of
the dead player in two cases. In the first case the game rule `keep_inventory` is true when the
server accepts the respawn request. In the second case the dead player is in spectator mode. The inventory is the 36 main
slots and all other equipment slots. The selected hotbar slot is also kept. In each other case,
the inventory is empty, the selected hotbar slot is 0, and the experience level, progress, total
experience and score are 0. The check of the game rule is at the respawn, not at the death. A
change of the rule between the two events changes the result.

**PLD-RST-004** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The
respawned player has the base value of each attribute of the dead player. It has no attribute
modifier of the dead player. This includes the modifiers that a command added and the modifiers
that an item gave. The modifiers of the worn items apply again in the first tick in which the
server finds the worn items as changed slots (PLD-EQP-006). The health of
the respawned player is its max health, which the base value of `max_health` gives.

The recording `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/a4_respawn.txt` (experiment A4
of `attributes.md`) shows the attributes. After the respawn, `max_health` has the base value 30 of
the dead player. `luck` has no modifier of `/attribute`, and `movement_speed` has no modifier of
the effect `speed`. The recording shows no health and no item modifier. The statements of this rule
about the health and about the modifiers of items are `source_suggested`.

**PLD-RST-005** (`source_suggested`, semantic, confidence: medium). The respawned player also has
these values from the dead player:

- the ender chest, the recipe book and the enchantment seed;
- the player model customisation and the last death location;
- the entered-Nether position and the post effects;
- the data of the warden spawn tracker.

In a hardcore server, the respawned player is
in spectator mode, as if `/gamemode spectator` ran at once after the respawn
(`command-gamemode.md`). The check is the hardcore setting of the server. The change comes after
the respawn and sends its own packets of CMD-GM-003, including `player_info_update` with the game
mode action. The respawn itself sends no `player_info_update`.

## 7. The packets of a respawn

**PLD-NET-001** (`source_suggested`, wire, confidence: medium). When the server accepts a respawn
request, the connection of the player receives these packets in this order. Section 8 lists
the packets that other players receive.

| Step | Packet | Content |
|------|--------|---------|
| 0 | `game_event` | event 0, value 0.0. Only when the respawn point is not valid (PLD-RSP-002 step 5) |
| 1 | `respawn` | PLD-NET-002 |
| 2 | `player_position` | a new teleport id, the respawn position, the rotation and an empty relative set. The player must confirm it (`teleport-confirmation.md`) |
| 3 | `set_default_spawn_position` | the world spawn of the respawn dimension: the dimension (Identifier), the block position, the yaw (f32) and the pitch (f32) |
| 4 | `change_difficulty` | as DIM-CHG-003 step 2 |
| 5 | `set_experience` | the experience progress, the total experience and the level, as PLD-RST-003 states them |
| 6 | `update_mob_effect` | none, because the respawned player has no effect |
| 7 | `post_effects` | the post effects of the player |
| 8 | the packets of DIM-CHG-003 steps 8 to 14 | the border, the time, the world spawn again, the rain events, event 13 of `game_event`, the ticking state and the ticking step |
| 9 | `entity_event` and `commands` | the permission level of the player, as DIM-CHG-003 steps 3 and 4 |
| 10 | `container_set_content` | the inventory menu with the carried item, as PLD-RST-003 states it |
| 11 | `sound` | `block.respawn_anchor.deplete` (PLD-NET-003), only in that case |

The chunks and the entities of the respawn position (PLD-NET-004) go between step 2 and step 10 and
after step 10. Their position against steps 3 to 10 is free. The server sends `set_health` and
a second `set_experience` in the next tick. This is the rule of DIM-CHG-003: the respawn
marks the sent values as unknown (DMG-NET-005). The server does not send `set_held_slot` in
this sequence (unresolved U2).

**PLD-NET-002** (`source_suggested`, wire, confidence: high). The fields of `respawn` (play
clientbound 84) in this game version are, in this order:

| Field | Type | Value |
|-------|------|-------|
| dimension type | VarInt | the network id of the dimension type of the respawn dimension (REG-NET-006) |
| dimension | Identifier | the respawn dimension |
| game mode | VarInt | the id of the game mode of the player |
| previous game mode | VarInt | 0 for none, otherwise the id of that mode plus 1 |
| flat | Boolean | true when the respawn dimension is a flat world |
| last death location | optional | a Boolean; when true, the Identifier of the dimension and the block position of the death |
| portal cooldown | VarInt | 0 |
| sea level | VarInt | the sea level of the respawn dimension |
| data to keep | Byte | 0 |

The last death location is the death that the server just stored (PLD-DTH-001 step 4). With the
data to keep 0, the client keeps no attribute modifier and no entity data of its player. A
dimension change sends 3 (DIM-CHG-004). DIM-CHG-004 states the fields of the packet.

**PLD-NET-003** (`source_suggested`, wire, confidence: medium). The respawned player can keep a
respawn point. If the block of that point is a respawn anchor, the server sends one `sound`
packet (play clientbound 119). The packet goes to that player. The sound is `block.respawn_anchor.deplete`. The
source is `blocks`. The position is the block position of the anchor, as whole block
coordinates. The volume and the pitch are 1.0. The server sends it in the same case when the
flag `forced` is true and the anchor has no charge.

**PLD-NET-004** (`source_suggested`, semantic, confidence: low). The chunks around the respawn
position reach the client. For a respawn into another dimension, DIM-CHG-007 states the load of
the chunks. For a respawn in the same dimension, section 12 (U3) lists this as unresolved.

## 8. What other players see at a respawn

**PLD-NET-005** (`source_suggested`, semantic, confidence: medium). Each viewer of the dead player
that still tracks it receives `remove_entities` with its network id. This is the packet of
PLD-DTH-002 or of the respawn, whichever comes first. A viewer that tracks the respawned player
receives the spawn bundle of TRK-SPN-001, with the same network id. The respawned player leaves the viewers that it did not track in the respawn dimension. The order of the packets is the removal first, then the spawn. The respawn sends no `player_info_update` to a viewer. The hardcore change of PLD-RST-005 can send one afterwards.

## 10. Equipment sync

**PLD-EQP-001** (`source_suggested`, wire, confidence: high). The packet `set_equipment` (play
clientbound 104) has these fields:

| Field | Type | Value |
|-------|------|-------|
| entity | VarInt | the network id of the entity |
| entries | list | one entry for each changed slot |

Each entry has a slot byte and a wire stack (STK-NET-001). Bits 0 to 6 of the slot byte are the
slot id. Bit 7 is set when another entry follows. The slot ids are those of CENTS-EQUIP-001:
`mainhand` 0, `offhand` 1, `feet` 2, `legs` 3, `chest` 4, `head` 5, `body` 6, `saddle` 7. An
empty stack in an entry empties the slot on the client. The entries are in the order of the slot
ids. A packet has at least one entry.

**PLD-EQP-002** (`source_suggested`, semantic, confidence: medium). For a player, the equipment
slots read these places of the inventory:

| Equipment slot | Inventory slot |
|----------------|----------------|
| `mainhand` | the selected hotbar slot, 0 to 8 |
| `feet`, `legs`, `chest`, `head` | 36, 37, 38, 39 |
| `offhand` | 40 |
| `body` | 41 |
| `saddle` | 42 |

**PLD-EQP-003** (`source_suggested`, wire, confidence: high). When a viewer
starts to track an entity, the viewer receives one `set_equipment` in the spawn bundle of
TRK-SPN-001, step 4. The packet has one entry for each slot that is not empty, in the order of
the slot ids. When all slots are empty, the bundle has no `set_equipment`. This rule applies to a player in three cases. The viewer enters the range of the player. The player joins the server inside the range of the viewer. The player respawns (PLD-NET-005). TRK-E1 shows the packet of
a mob.

**PLD-EQP-004** (`source_suggested`, semantic, confidence: medium). Each tick, the server compares
the stack of each equipment slot of a living entity with the last sent stack of that slot. A
slot is changed when the item, the count or any component differs. The server then does these
steps:

1. It stores a copy of the new stack as the last sent stack of each changed slot.
2. It sends one `set_equipment` to the tracking players. It has one entry for each changed slot.
   An emptied slot has an empty stack.

The server does not send this packet to the entity itself. A player gets the changes of its own
inventory from the packets of `inventory-transactions.md`. The comparison includes the count and
the components. Thus a change of the count of the held stack, and a change of the damage of a
tool, each make a packet. The packet goes in the tick in which the entity ticks. In that tick,
it goes before the update packets of the tracker (TRK-UPD-002). The same tick can change a slot
more than one time. The viewers see only the last value (kind: semantic).

**PLD-EQP-005** (`source_suggested`, wire, confidence: medium). When the main hand
and the off hand exchange their stacks in one tick, the server sends the entity event 55 to the
tracking players. It sends no `set_equipment` for these two slots. The exchange has three
conditions. Both slots changed. The new main-hand stack equals the last sent off-hand stack. The new off-hand
stack equals the last sent main-hand stack. The server updates the last sent stacks of both
slots. A change of another slot in that tick still makes a `set_equipment`. The client swaps its
two stacks when it reads the event.

**PLD-EQP-006** (`source_suggested`, semantic, confidence: medium). The last sent stacks of a new
player object are empty. In the first tick of a player after it joins the server, each slot that is not empty is a changed
slot (PLD-EQP-004). This is true also in the first tick after a respawn. The
tracking players can then receive a `set_equipment` that repeats the entries of the spawn bundle
(PLD-EQP-003). This repeat is of the kind unspecified: an implementation can send it or not,
and a parity test must not compare it.

**PLD-EQP-007** (`source_suggested`, semantic, confidence: medium). The drops of a death empty
the equipment slots of a dying player (section 4). In the next tick of the dead player, the
tracking players receive one `set_equipment` with an empty stack for each slot that was not
empty. A player that keeps its inventory (PLD-DRP-001) sends no such packet.

## 11. Intentional deviations

### PLD-DEV-001: A search radius of 8 tests each column

- **Rule:** PLD-RSP-005.
- **Reference:** for a search radius of 8, the game tests only 17 of the 289 columns of the
  square. The 17 columns have the same x, at a random offset, and each z of the square. The game
  tests each of them 17 times. For each other search radius up to 15, it tests each column one
  time.
- **Reason:** a player expects the same search for each search radius. The reference result is
  an accident of its pseudo-random order.
- **Visible to the client:** yes, with `respawn_radius` 8. The server finds a valid candidate in
  more cases. The game falls back to the height fix more often.
- **Bug reference:** `none`.
- **Test:** a differential test does not compare a spawn placement with a search radius of 8.

## 12. Unresolved


- **U2.** After a respawn with `keep_inventory`, the server keeps the selected hotbar slot of the
  dead player. The reference sends no `set_held_slot` in the respawn sequence. A new client player
  starts with slot 0. The client and the server can then disagree on the selected slot until the
  player changes it. Is this a defect? A player expects the same slot.
- **U3.** The reference copies the chunk tracking view of the dead player to the respawned player.
  The behaviour of the chunk packets for a respawn in the same dimension is not stated.
- **U4.** For a search radius of 16 or more, the square has more than 1024 columns. The game then
  tests only 1024 columns. These are the first 1024 columns of the square, in rows of the same z,
  from the row of the lowest z. They form a strip along the side of the lowest z. A player does not
  expect this. PLD-RSP-005 states the behaviour only for a search radius up to 15. The owner must
  decide the behaviour for a larger radius. The default `respawn_radius` 10 does not depend on this
  decision.
- **U5.** The pose of a dying player. DMG-NET-002 states that the death sends the pose `dying`. It is not
  known whether a viewer receives that pose for a player. The client can animate the death from
  the health 0 alone.
- **U6.** The tag `always_most_significant_fall` is not in the corpus of this repository. The set of
  damage types that have it is unknown. The owner must supply the tag file.
- **U7.** The order of the stand-up offsets of a bunk bed (a bed with a bed block below it). It
  has a third search, below the bed and above the bed. A later bed specification states it.
- **U8.** What the server does with an open container menu of a dying player. PLD-DRP-005 states
  the stacks of the inventory menu only. A stack on the cursor of an open chest menu is not
  covered.
- **U9.** The exact floor-height test of PLD-RSP-003 for a block with a partial collision shape,
  for example a slab or a carpet. 

## 13. Experiments

Each experiment runs on the vanilla server of the game version of this document, in a new world.
Run `forceload add 0 0` and `whitelist off` first. Each join uses a new player name, so the
player has no player file. Read the position of the join in the log line of the login. The
maximum height of the overworld is 319.

| Id | Steps | Observed result | Rules |
|----|-------|-----------------|-------|
| PLD-E1 | Set `respawn_radius` to 0. Place `stone` at `0 100 0`. Run `setworldspawn 0 0 0`. Join a player in survival. | The player joins at `(0.5, 101.0, 0.5)`. The y comes from the column, not from `S`. | PLD-RSP-005; `persistence.md` SAVE-PLAYER-001 |
| PLD-E2 | Run `defaultgamemode adventure`. Place `air` at `5 -64 5` and `5 -63 5`, and `stone` at `5 -62 5`. Run `setworldspawn 5 -64 5`. Join a player. | The player joins at `(5.5, -64.0, 5.5)`. | PLD-RSP-006 |
| PLD-E3 | In adventure, fill `8 -64 8` to `8 319 8` with `stone`. Run `setworldspawn 8 0 8`. Join a player. | The player joins at `(8.5, 319.0, 8.5)`, inside the stone. | PLD-RSP-006 |
| PLD-E4 | Run `defaultgamemode spectator`. Run `setworldspawn 3 -10 4`. Join a player. | The player joins at `(3.5, -10.0, 4.5)`. | PLD-RSP-005 |
| PLD-E5 | Run `defaultgamemode survival`, with `respawn_radius` 0. Place `barrier` at `12 119 12` and at the four sides of `12 120 12`. Place `water` at `12 120 12`. Run `setworldspawn 12 130 12`. Join a player. | The player joins at `(12.5, 121.0, 12.5)`, on the water. The column has a fluid surface, and the height fix stops above the water. | PLD-RSP-005, PLD-RSP-006 |
| PLD-E6 | In survival, with `respawn_radius` 0 and `immediate_respawn` true, run `execute in minecraft:the_end run setworldspawn 20 0 20`. Join a player. Teleport it to `-10 80 -10`, then kill it. After the respawn, run `data get entity <name> Pos`. | The player joins at `(20.5, 64.0, 20.5)` in the End. After the respawn, `Pos` is `[20.5d, 64.0d, 20.5d]`. | PLD-RSP-004, PLD-RSP-005 |
| PLD-E7 | In the End, run `worldborder set 50`, with its centre at `0 0`. Run `setworldspawn 100 70 100` in the End, and join a player. Then run `setworldspawn 30 70 30` in the End, and join another player. | The first player joins at `(100.5, 1.0, 100.5)`. The column is void, and the minimum height of the End is 0. The second player joins at `(31.5, 63.0, 29.5)`, a column of the square of radius 1 around `30 30`. | PLD-RSP-005, PLD-RSP-006; `persistence.md` SAVE-DEV-006 |
