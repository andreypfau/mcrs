# Container loot: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states when a container with a saved loot table gets its contents. It states which
block entities and entities can hold a table and a seed, and their saved fields. It states each
event that rolls the table, and the events that do not. It states the context and the generator of
the roll, and when the table and the seed are cleared. It also states how world generation, a
template, a command and an item give a table and a seed to a container. It does not prescribe an
architecture.

Scope: the container source of a loot table. `loot.md` states the evaluation of a table
(LOOT-EVL), the key sets (LOOT-CTX-011) and the luck (LOOT-CTX-015). It also states the spread over
the slots (LOOT-CTR-001 to LOOT-CTR-005) and the random draws (LOOT-RND-002 and LOOT-RND-004 to
LOOT-RND-009). This document refers to these rules by id and does not state them again.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**: as `login.md`, section 0.1, defines them.
- **Stack**, **empty stack**: as `item-stack.md`, section 0.1, defines them.
- **Loot table**, **table**, **context**, **parameter**, **key set**, **luck**, **generator**,
  **draw**, **drop list**, **free slot**, **keyed stream**: as `loot.md`, section 0.1, defines them.
- **Sequence stream**: as LOOT-RND-005 states it. **Exact mode** and **keyed mode**: as LOOT-RND-002
  states them.
- **Dimension random**: as `block-simulation.md`, section 0.1, defines it (SIM-RNG-001).
- **Spread**: the steps of LOOT-CTR-001 to LOOT-CTR-005.
- **Loot container**: a block entity or an entity of a type that CTL-HLD-001 lists.
- **Table reference**: the key of a loot table that a loot container holds. The save field is
  `LootTable`.
- **Seed**: the signed 64-bit integer that a loot container holds with its table reference. The
  save field is `LootTableSeed`. The seed 0 means "no seed" (LOOT-RND-004).
- **Unrolled container**: a loot container that holds a table reference.
- **Roll**: the steps of CTL-ROL-001 for one unrolled container.
- **Slot access**: an operation that reads the stack of a slot, or changes the stack of a slot, of
  a container. The save of a container is not a slot access.
- **Spectator**: a player in the game mode `spectator`.
- **Lock**: the item predicate in the save field `lock` of a block entity (component
  `minecraft:lock`). A player passes the lock when the main-hand item matches the predicate. A
  block entity without the field has no lock.
- **Double chest**: two blocks of the same chest type that the block state property `type`
  connects, with the values `left` and `right`. Each half is a separate loot container. The view of
  54 slots of a double chest has the 27 slots of the half `right` as the slots 0 to 26, and the 27
  slots of the half `left` as the slots 27 to 53.
- **64-bit draw**: one signed 64-bit value from the generator of a placement. For the 48-bit
  generator it is `next_long()` of BMR-PICK-003, step 4. For a Xoroshiro128++ generator it is the
  value `x` of LOOT-RND-006. A 64-bit draw is not a draw of an evaluation (LOOT-RND-006).

### 0.2 Provenance

The labels are those of `registry.md`, section 0.3. The author read the reference of the game
version above. Each rule states its kind, as `docs/spec/README.md` defines it.

### 0.3 Artefacts cited

- The data pack of the client jar: `data/minecraft/loot_table/chests/` (58 tables). Each of these
  tables has the type `chest` and a `random_sequence`.
- The structure templates of the client jar: `data/minecraft/structure/`. In these templates, 569
  block entity compounds have the field `LootTable`: 350 `chest`, 209 `barrel`, 6 `dispenser` and
  4 `decorated_pot`. No compound has the field `LootTableSeed`. No entity of a template has a
  table. 198 of these compounds also have the field `Items` with an empty list. They are all in the
  templates under `data/minecraft/structure/abandoned_camp/`. No compound with `LootTable` has an
  `Items` list with a stack, or the field `item`.
- The dispensers and the decorated pots with a table are all in the templates under
  `data/minecraft/structure/trial_chambers/`. The dispensers name the tables
  `dispensers/trial_chambers/corridor` and `dispensers/trial_chambers/chamber`. The pots name the
  table `pots/trial_chambers/corridor`. These tables have the type `chest`. Thus a template gives a
  table to a dispenser and to a decorated pot, and not only to a chest or a barrel.
- `assets/mcrs/reports/registries.json`: the entries of `minecraft:block_entity_type`,
  `minecraft:entity_type` and `minecraft:trigger_type`.
- The loot fixture `crates/mcrs_minecraft_loot_table/tests/fixtures/vanilla/loot.bin` of the loot
  dumper of `tools/vanilla-oracle`, with `loot_capture_procedure.md` beside it.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CTL-HLD` | the loot containers, their saved fields and the item component |
| `CTL-TRG` | the events that roll the table, and the events that do not |
| `CTL-ROL` | the roll: its steps, its context, its generator and the clear of the reference |
| `CTL-GEN` | how world generation, a template, a command and an item assign a table and a seed |
| `CTL-DEV` | the intentional deviations of section 5.1 |

## 1. Loot containers

**CTL-HLD-001** (`source_suggested`, semantic, confidence medium). These block entity types and
entity types are loot containers. The type names come from `assets/mcrs/reports/registries.json`.
The slot count is the number of slots of the container for the spread (LOOT-CTR-002).

| Kind | Type | Slots |
|------|------|-------|
| block entity | `minecraft:chest` (also each copper chest), `minecraft:trapped_chest`, `minecraft:barrel`, `minecraft:shulker_box` | 27 |
| block entity | `minecraft:dispenser`, `minecraft:dropper`, `minecraft:crafter` | 9 |
| block entity | `minecraft:hopper` | 5 |
| block entity | `minecraft:decorated_pot` | 1 |
| entity | `minecraft:chest_minecart`, each `*_chest_boat`, `minecraft:bamboo_chest_raft` | 27 |
| entity | `minecraft:hopper_minecart` | 5 |

No other type is a loot container. The block entity `minecraft:brushable_block` has fields with the
same names, but it is not a loot container.

**CTL-HLD-002** (`source_suggested`, semantic, confidence medium). A loot container saves its table
reference and its seed in these fields of its compound:

| Field | Value | Written when |
|-------|-------|--------------|
| `LootTable` | an identifier: the key of a loot table | the container is an unrolled container |
| `LootTableSeed` | a long | the container is an unrolled container, and its seed is not 0 |

An unrolled container does not write its stacks: no `Items` field, and no `item` field for a
decorated pot. A loot container without a table reference writes no `LootTable` and no
`LootTableSeed`. It writes its stacks as a container without a table does. At the load:

- an absent `LootTableSeed` gives the seed 0;
- a compound with `LootTable` gives an unrolled container, and its slots are all empty;
- a compound with `LootTable` can also have an empty `Items` list, or no stack in `item`. The load
  accepts it, because it holds no stack. 198 compounds of the vanilla templates have this form
  (section 0.3);
- a compound with `LootTable` and an `Items` list with a stack, or a stack in `item`, fails the
  decode;
- a compound with `LootTableSeed` and without `LootTable` fails the decode;
- a compound without `LootTable` and without `LootTableSeed` gives a container without a table
  reference.

Each of the two failures is a separate error of the decode. Reason for the first failure: a
container holds a table reference with a seed, or the stacks, and never both. A load that accepts
both drops saved stacks with no report. Reason for the second failure: no event
reads a seed without a table reference. A load that accepts it drops saved data with no report,
because the next save does not write it. No vanilla template and no vanilla data pack file has
either form. CTL-DEV-002 records the reference.

A failure of the decode has this result for each carrier of the compound:

| Carrier | Result |
|---------|--------|
| a block entity of a saved chunk | the column cannot be decoded. Its region is quarantined, and the column reads as missing (SAVE-QUARANTINE-001) |
| a block entity of a template | the template does not load. `place template` that names it fails with `commands.place.template.invalid`. The load mode of a structure block that names it shows `structure_block.load_not_found`. No block changes |
| the compound of a command, for example `data merge`, `setblock`, `fill` or `summon` | the command fails with an error. No block and no entity changes |
| `minecraft:block_entity_data` of a stack (CTL-HLD-003, CTL-GEN-003) | the player places the block, and the block entity does not get the data of the component. The other components of the stack apply as PLC-INV-008 of `block-placing.md` states |

The key in `LootTable` does not need to name a table that exists (CTL-ROL-004). For a block
entity, the typed fields and the decode errors of this rule belong to `persistence.md`.

**CTL-HLD-003** (`source_suggested`, semantic, confidence medium). The item component
`minecraft:container_loot` holds a table reference and a seed. Its fields are `loot_table`, an
identifier, and `seed`, a long with the default 0. These events use it:

- The server places a block from a stack with the component. The block entity becomes an unrolled
  container with that table reference and seed. This does not apply to a decorated pot.
- The server makes a stack from an unrolled container of a block, and the stack gets the component.
  Two events do this. The first is a pick of the block with its data by a player in creative mode.
  The second is the drop of a shulker box that CTL-TRG-006 states. The 17 block tables of the
  shulker boxes copy the component with `copy_components` (`data/minecraft/loot_table/blocks/`).

The pick of an unrolled decorated pot with its data gives no `minecraft:container_loot`. The stack
gets `minecraft:block_entity_data` with `LootTable` and `LootTableSeed`. Thus a pot that a player
places from this stack is an unrolled container with the same table reference and seed.

A shulker box that an explosion breaks keeps its table reference and its seed in the item. A
shulker box that a player places from that item rolls the same table with the same seed. When the
seed is not 0, CTL-ROL-003 states when this roll gives the same stacks as the first box.

## 2. The events that roll the table

**CTL-TRG-001** (`source_suggested`, semantic, confidence medium). The server rolls the table of an
unrolled container at the first event of this section. The roll occurs in the same tick as the
event, before the event reads or changes a slot. Thus the event sees the stacks of the roll. Each
row of section 2 states whether the context has a player. A later event does not roll again,
because the roll cleared the table reference (CTL-ROL-005).

Exception (`design`, semantic): a roll can reach the search of an `exploration_map`
(LOOT-MOD-057). Then the roll is complete some ticks after the event, and the number of ticks is
unspecified (LOOT-MOD-060). Until the roll is complete, the container keeps its table reference and
its seed, and its slots hold no stack of the roll. Each event of this section in that time has no
effect on the slots, sees no stack and starts no second roll. For example:

- An open by a player (CTL-TRG-002) shows the container to the player when the roll is complete.
- A hopper, a dropper or a dispenser (CTL-TRG-008) moves no stack from or into the container.
- A comparator reads the signal of an empty container. It gets an update when the roll is complete.
- A removal of the block (CTL-TRG-005, CTL-TRG-006) removes the block at the event. The drop of
  that rule occurs at the block position when the roll is complete.
- A removal of an entity loot container (CTL-TRG-007) removes the entity at the event. The stacks
  drop at the position of the entity at the event when the roll is complete.

When the roll is complete, the server clears the table reference and the seed, and writes the
slots, in one step. Reason: the tick never waits for a search. CTL-DEV-004 records the reference.

**CTL-TRG-002** (`source_suggested`, semantic, confidence medium). A player opens an unrolled
container. The server rolls the table with that player (CTL-ROL-002), before it sends the contents
of the container to the client. Thus the first contents that the player sees are the stacks of the
roll. Exception: a roll that reaches a search shows the container when the roll is complete
(CTL-TRG-001). These actions open a container:

- a player uses a block loot container, and the block opens its menu;
- a player uses a chest minecart or a hopper minecart;
- a player uses a chest boat with the sneak key held, or a chest boat without a free seat;
- a passenger of a chest boat opens the inventory of the boat with the inventory key.

**CTL-TRG-003** (`source_suggested`, semantic, confidence medium). These conditions prevent the open
of CTL-TRG-002. Then the server does not roll the table, and the container stays an unrolled
container.

- The player is a spectator. A spectator can open a container without a table reference. A
  spectator gets no message and hears no sound for an unrolled container.
- The block loot container has a lock, and the player does not pass it. The player gets the action
  bar message `container.isLocked` with the name of the container, and the sound
  `minecraft:block.chest.locked`. A spectator passes each lock.
- The block does not open. For example, a block above a chest blocks its lid, a cat sits on the
  chest, or a block blocks the lid of a shulker box.

**CTL-TRG-004** (`source_suggested`, semantic, confidence medium). A player opens a double chest.
The server tests the conditions of CTL-TRG-003 for both halves before it rolls a table. When one
half fails, the server rolls no table. Otherwise it rolls the table of each unrolled half, with the
player. The half `right`, which holds the slots 0 to 26 of the view of 54 slots, rolls first. The
order changes a result only in exact mode, when both halves continue one sequence stream
(CTL-DEV-001).

**CTL-TRG-005** (`source_suggested`, semantic, confidence medium). The block of a block loot
container is removed, and the removal drops the contents of the container (BRK-DRP-002). The server
rolls the table without a player, also when a player breaks the block. Then the stacks of the roll
drop as the contents. This applies to each block loot container except a shulker box (CTL-TRG-006).
These removals drop the contents:

- a break by a player, in survival mode and in creative mode;
- an explosion;
- the command `setblock` with the mode `destroy`, and `fill` with the mode `destroy`;
- each other removal of the block that is not in CTL-TRG-010.

The game rule `block_drops` does not change the roll (BRK-LOOT-005). A decorated pot that a
projectile breaks also rolls its table.

**CTL-TRG-006** (`source_suggested`, semantic, confidence medium). The break of an unrolled shulker
box has these results:

| Break | Roll | The shulker box item |
|-------|------|----------------------|
| a player in survival or adventure mode | with that player | holds the stacks of the roll in `minecraft:container`, and has no `minecraft:container_loot` |
| a player in creative mode | without a player | drops only when the roll gave a stack. It holds the stacks of the roll |
| an explosion, a piston, or another break without a player | no roll | holds `minecraft:container_loot` with the table reference and the seed (CTL-HLD-003) |

**CTL-TRG-007** (`source_suggested`, semantic, confidence medium). An entity loot container is
removed for one of these reasons. The server rolls the table without a player, and the stacks of the
roll drop as the contents of the entity.

- Damage destroys the entity. The game rule `entity_drops` does not change the roll, and the
  contents drop also when the game rule is false.
- A player in creative mode hits the entity.
- The command `kill` kills the entity.

The unload of the entity with its chunk, and a change of dimension, do not roll the table. The
entity keeps its table reference and its seed.

**CTL-TRG-008** (`source_suggested`, semantic, confidence medium). These mechanisms make a slot
access without a player. Each one rolls the table of an unrolled container without a player. The
column "Moment" states the event in the tick at which the mechanism reads the slots.

| Mechanism | When it rolls | Moment |
|-----------|---------------|--------|
| a hopper or a hopper minecart under the container | the hopper has room for a stack, and it tries to take a stack from the container | the transfer of the hopper |
| a hopper that points into the container | the hopper holds a stack, and it tries to put the stack into the container | the transfer of the hopper |
| a dropper that points into the container | the dropper puts a stack into the container | the dispense of the dropper |
| an unrolled hopper | its next transfer attempt: the hopper is enabled, and it has no cooldown | the transfer of the hopper |
| a dispenser or a dropper | redstone activates it. It rolls before it selects a slot | the scheduled tick of the dispense, 4 ticks after the activation |
| a crafter | redstone activates it. It rolls before it reads its slots | the scheduled tick of the craft, 4 ticks after the activation |
| a comparator | the comparator reads the container. It reads a minecart container through a detector rail that the minecart is on. A comparator reads 0 from a chest with a blocked lid (CTL-TRG-003), and does not roll it | each time the comparator calculates its input: at a neighbour update of the comparator, in its scheduled tick, when an entity places it, and when a player changes its mode |
| a copper golem | the golem tests the slots of a chest, or changes them | the tick of the golem in which it comes within its interaction range of the chest |
| a decorated pot | a player uses the pot, with an item or with an empty hand. The pot rolls before it tests the item. A use with the sneak key held and an item in a hand does not roll | the use of the block by the player |

The transfer of a hopper occurs in the tick of its block entity. A hopper, a comparator and a
copper golem see a double chest as the view of 54 slots. An access to a slot rolls only the half that
holds that slot. A comparator reads each slot in ascending order, so it rolls the half `right`
first. A copper golem tests the half `right` first, and it tests the half `left` only when the half
`right` is empty after its roll.

The row of a crafter is `design`, semantic. Reason: without the roll, a crafter that holds a table
crafts from its empty slots, and a later open replaces these slots with the stacks of the roll.
CTL-DEV-003 records the reference for a crafter.

**CTL-TRG-009** (`source_suggested`, semantic, confidence medium). These commands make a slot access
without a player. Each one rolls the table of an unrolled container without a player, also when the
command source is a player.

- `loot insert`, `loot replace block` and `loot replace entity` into the container;
- `item replace block`, `item replace entity` and `item modify` on a slot of the container;
- `execute if items` and `execute unless items` on a slot of the container.

When the roll of the container reaches a search, each of these commands sees the container as the
exception of CTL-TRG-001 states. A table that `loot` names, and a modifier that `item modify` names,
are separate evaluations. LOOT-MOD-060 states which of them waits for its own search.

**CTL-TRG-010** (`source_suggested`, semantic, confidence medium). These events do not roll the
table. The container keeps its table reference and its seed.

- The save of a chunk or of an entity, and the unload of a chunk.
- `data get block` and `data get entity`. The result shows `LootTable` and no `Items`.
- `clone`. The copy is an unrolled container with the same table reference and seed. The block of
  the mode `move` is removed without a drop.
- `setblock` with the mode `replace` or `strict`, and `fill` with each mode except `destroy`. These
  remove the block without a drop of the contents.
- The pick of a block with its data (CTL-HLD-003).
- A refused open (CTL-TRG-003).

## 3. The roll

**CTL-ROL-001** (`source_suggested`, semantic, confidence medium). A roll of an unrolled container
does these steps:

1. Clear the table reference (CTL-ROL-005).
2. Make the context of CTL-ROL-002 with the key set `minecraft:chest` (LOOT-CTX-011).
3. Evaluate the table of the reference with that context (LOOT-EVL-001) and the generator of
   CTL-ROL-003 or CTL-ROL-007.
4. Spread the drop list over the slots of this loot container (LOOT-CTR-001), with the same
   generator.

The spread puts stacks only in the free slots (LOOT-CTR-001). A stack that does not fit is lost
(LOOT-CTR-004). Each half of a double chest is a container of 27 slots for the spread. A slot
access of the steps 3 and 4 does not roll again, because step 1 cleared the reference. Step 1 also
clears the reference when the table does not exist (CTL-ROL-004) or gives an empty drop list.

Exception: a roll that reaches a search (LOOT-MOD-057) clears the table reference and the seed when
the roll is complete. The server writes the slots of step 4 in the same step (CTL-TRG-001). The
result of steps 3 and 4 is the result that LOOT-MOD-058 states for the generator at the event.

**CTL-ROL-002** (`source_suggested`, semantic, confidence medium). The context of a roll holds
these values and no other parameter:

| Value | Block loot container | Entity loot container |
|-------|----------------------|-----------------------|
| `origin` | the centre of the block position: `(x + 0.5, y + 0.5, z + 0.5)` | the position of the entity at the event of the roll |
| `this_entity` | the player of the event, when the event has one | the same |
| luck | the luck of that player (LOOT-CTX-015); 0 without a player | the same |

The origin decides a condition `location_check` and the search of an `exploration_map`, for example
in `chests/abandoned_mineshaft` and `chests/shipwreck_map`. No vanilla table of the type `chest`
reads `this_entity`, and no such table uses `quality` or `bonus_rolls`.

**CTL-ROL-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). A roll of a
container with a seed that is not 0 takes its draws from the generator that LOOT-RND-004 selects
for that seed. The evaluation and the spread take all their draws from this one generator
(LOOT-RND-008). Two containers in the same world can have the same table, the same seed, the same
origin, the same luck and the same free slots. Then they get the same stacks in the same slots.

**CTL-ROL-004** (`source_suggested`, semantic, confidence medium). The table reference can name a
key that the registry `minecraft:loot_table` does not hold. Then the roll clears the reference and
puts no stack in a slot. The server does not report an error. A table of a type other than `chest`
uses the context of CTL-ROL-002 all the same (LOOT-CTX-012).

**CTL-ROL-005** (`source_suggested`, semantic, confidence medium). After a roll, the container has no
table reference and the seed 0. Thus:

- the next save writes the stacks, and writes no `LootTable` and no `LootTableSeed`;
- a container rolls its table at most one time for each table reference that it gets;
- a later assignment of a table without a seed (CTL-GEN-003) gives the seed 0, and not the seed of
  the earlier table.

**CTL-ROL-006** (`design`, semantic). A roll with a seed that is not 0 is in the exact mode of
LOOT-RND-002. A parity test makes a loot container with the slot count of a fill case of the
loot fixture, all slots empty. It gives the container the table reference and the seed of
the case, and the origin of the case. Then it rolls with no player. It compares the stack of each
slot and the state of the generator after the spread with the case.

**CTL-ROL-007** (`design`, semantic). A roll of a container with the seed 0 takes its draws from a
keyed stream (LOOT-RND-009). This is the keyed mode of LOOT-RND-002. The value of the key that
tells the source apart (LOOT-RND-009, fourth item) is:

- for a block loot container: the key of the dimension and the block position;
- for an entity loot container: the UUID of the entity.

Thus the two halves of a double chest roll from two separate streams, and the order of their rolls
does not change a result. Two containers with the seed 0 and the same table get independent stacks.
The evaluation and the spread take their draws from the same keyed stream. A parity test compares
the distribution of the stacks over many rolls. CTL-DEV-001 records the reference behaviour.

## 4. The assignment of a table and a seed

**CTL-GEN-001** (`source_suggested`, semantic, confidence medium). A world generation feature or a
structure piece can put a loot container with a table. It then takes one 64-bit draw from its own
generator, and the draw becomes the seed. The container stays an unrolled container until an event
of section 2. These vanilla sources do this:

| Source | Container | Table |
|--------|-----------|-------|
| the feature `monster_room` | chest | `chests/simple_dungeon` |
| the bonus chest of a new world | chest | `chests/spawn_bonus_chest` |
| a structure piece that places a chest, for example a desert pyramid, a jungle temple, a nether fortress, a stronghold, a woodland mansion, a buried treasure, an igloo, an ocean ruin, a shipwreck or an end city | chest | the table of the piece |
| a jungle temple | dispenser | `chests/jungle_temple_dispenser` |
| a mineshaft corridor | chest minecart | `chests/abandoned_mineshaft` |

The generator of each source, the order of its draws and the positions belong to the documents of
the structures and the features. Thus the seed of each container depends on the world
seed and on the position of the structure.

**CTL-GEN-002** (`source_suggested`, semantic, confidence medium). The placement of a template puts
its blocks in this order:

1. the blocks without a compound that have a full collision shape;
2. the other blocks without a compound;
3. the blocks with a compound.

In each group, the order is ascending by `y`, then by `x`, then by `z`, in the coordinates of the
template before the rotation. The block entity of a block with a compound can be a loot container.
For each such block that the placement puts, the placement takes one 64-bit draw from its
generator. The draw replaces the field `LootTableSeed` of the compound. Then the block entity loads
the compound (CTL-HLD-002). Thus:

- a compound with `LootTable` gives an unrolled container with the table of the compound and a new
  seed. The vanilla templates give tables to chests, barrels, dispensers and decorated pots
  (section 0.3);
- a compound without `LootTable` gives no table, but it still takes its draw. That draw changes
  the seeds of the later containers of the placement;
- a block that the placement does not put takes no draw. Such a block is outside the bounding box
  of the placement, or outside the build height, or a processor removes it.

The generator of the placement is as follows:

| Placement | Generator |
|-----------|-----------|
| the load mode of a structure block, with the field `seed` not 0 | a new 48-bit generator (BMR-PICK-003), set to `seed` |
| `place template`, with the argument `seed` not 0 | a new 48-bit generator, set to `seed` |
| a structure block or `place template` with the seed 0 | a new 48-bit generator, set to a seed that the server selects for each placement |
| a structure in world generation | the document of the structures states it |

The method that selects the seed for the seed 0 is unspecified, and a parity test does not compare
it. The vanilla server uses the wall-clock time in milliseconds. The argument `seed` of
`place template` is a 32-bit int with the default 0. With an integrity below
1, the structure block and `place template` remove blocks with a second generator of the same seed.
That generator takes no 64-bit draw of this rule. A block that the integrity removes takes no draw.
A template keeps the fields `LootTable` and `LootTableSeed` of an unrolled container when a
structure block saves it.

**CTL-GEN-003** (`source_suggested`, semantic, confidence medium). These commands and items give a
table reference and a seed directly, with the fields of CTL-HLD-002:

- `setblock`, `fill` and `summon` with a compound that has `LootTable`;
- `data merge` and `data modify` on a loot container;
- a stack with `minecraft:container_loot` (CTL-HLD-003), or with `minecraft:block_entity_data` that
  has `LootTable`.

A compound with `LootTable` and without `LootTableSeed` gives the seed 0. Thus each chest that
`setblock` places with only `LootTable` rolls from a keyed stream (CTL-ROL-007).

## 5. Intentional deviations, unresolved items

### 5.1 Intentional deviations

**CTL-DEV-001.**

- **Rule**: CTL-ROL-007.
- **Reference**: a roll with the seed 0 continues the sequence stream of the `random_sequence` of
  the table (LOOT-RND-005). Each vanilla table of the type `chest` has a `random_sequence`. A table
  without one takes the dimension random (SIM-RNG-001). Thus the stacks depend on
  the order of all earlier rolls of the same sequence. Both halves of a double chest continue one
  stream, and the half `right` rolls first. LOOT-DEV-101 records the same difference for each
  source.
- **Visible to the client**: no, as a distribution. A player cannot repeat the stacks of the
  vanilla server for a container with the seed 0. The containers of world generation have a seed,
  and this deviation does not change them.
- **Test**: a differential test of a container with the seed 0 compares the distribution of the
  stacks over many rolls. A test in exact mode gives the state of the sequence stream as the
  generator.

**CTL-DEV-002.**

- **Rule**: CTL-HLD-002.
- **Reference**: the load ignores the stacks of a compound with `LootTable` and a non-empty `Items`
  list or a stack in `item`. The container rolls its table, and the stacks are lost. The load also
  ignores `LootTableSeed` without `LootTable`, and the next save does not write it. The vanilla
  server never writes either form.
- **Visible to the client**: yes, only through a command, a custom template or a custom stack. A
  `data merge`, `setblock`, `fill`, `summon` or `place template` with such a compound gives an
  error in place of a container (CTL-HLD-002).
- **Test**: a differential test does not use such a compound.

**CTL-DEV-003.**

- **Rule**: CTL-TRG-008, the row of a crafter.
- **Reference**: the craft of a crafter reads its slots, but it does not roll the table first. An
  unrolled crafter thus crafts nothing, and it keeps its table reference and its seed.
- **Visible to the client**: yes, only for a crafter that gets a table from a command or an item.
  No vanilla source gives a table to a crafter. The player sees a craft from the stacks of the roll.
- **Test**: a differential test does not activate an unrolled crafter.

**CTL-DEV-004.**

- **Rule**: CTL-TRG-001, the exception, and CTL-ROL-001.
- **Reference**: a roll that reaches a search runs the search in the tick of the event. The event
  sees the stacks of the roll in that tick, and the tick takes as long as the search.
- **Visible to the client**: yes. A container whose table has `exploration_map`, for example
  `chests/shipwreck_map`, shows its contents some ticks after the open. A hopper, a comparator or a
  drop of the break gets the stacks some ticks after the event.
- **Test**: a differential test compares the slots and the stacks after the roll is complete. It
  does not compare the tick of the stacks.

### 5.2 Unresolved

None.

Item U1 is closed: CTL-ROL-007 applies the decision of LOOT-RND-009 to a container with the seed 0.
Item U2 is closed: CTL-TRG-005 and CTL-TRG-006 state the context of a break. A roll from a saved
seed must give the stacks of the vanilla server, also for a player with a luck other than 0. Item
U4 is closed: CTL-TRG-008 rolls the table of a crafter at its activation (CTL-DEV-003). Item U3 is
closed: CTL-HLD-002 states the load of each form, with CTL-DEV-002.
