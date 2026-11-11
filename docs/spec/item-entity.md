# Item entity: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| World version | `5127` |

Source of these values: `assets/minecraft/version.json`.

This document states the item entity: a stack that lies in the world. It states the start values of
an item entity for each cause, its motion in a fluid, its age, its pickup by a player, the merge of
two item entities, its damage, its saved form and its packets. It does not prescribe an
architecture.

Scope: the item entity from its spawn to its removal. `item-stack.md` states the stack.
`inventory-transactions.md` states the `drop` operation that makes a dropped stack (ITX-OP-020).
`entity-physics.md` states the move, the gravity and the drag of an item on land and in air
(PHY-ORD-005). `entity-tracking.md` states the spawn bundle, the updates and the removal of an
entity. This document refers to the rules of these documents by id. Section 9 lists what this
document does not state.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**: as `login.md`,
  section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **maximum stack size**: as `item-stack.md`, section 0.1,
  defines them.
- **Dropped stack**: as `inventory-transactions.md`, section 0.1, defines it.
- **Placement**, **holder**: as `docs/item-design.md`, section 1, defines them.
- **Item entity**: an entity of the type `minecraft:item`. It holds one stack that is not empty.
- **Alike**: two stacks are alike when they have the same item and components (STK-EQ-001).
- **Owner**: the player that the field `Owner` names. Only the owner can pick up the item entity.
- **Thrower**: the entity that the field `Thrower` names. The thrower does not limit a pickup.
- **Pickup delay**: the value of the field `PickupDelay`, in ticks.
- **Age**: the value of the field `Age`, in ticks.
- **Health**: the value of the field `Health`.
- **Collector**: the player that picks up an item entity.
- **Pickup box**: the box in which a player collects item entities (IEN-PCK-001).
- **Viewer**: a player that tracks the item entity (`entity-tracking.md`, section 4).
- **Hand form**: the start values of IEN-SPN-002.
- **Default form**: the start values of IEN-SPN-008.
- **Tick count**: the number of ticks that the item entity existed, from 0 at its spawn. A load sets
  it to 0. It is not the age.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 13.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a captured result in section 13.
- `source_suggested`: taken from the reference implementation, with no captured result. The
  confidence is at most medium. Section 12 attaches an experiment.
- `documentation`: taken from public documentation of a protocol.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 11 collects these items.

Each rule states its kind, as `docs/spec/README.md` defines it. A rule with a captured result
states confidence high. A `source_suggested` rule states confidence medium. A `design` or
`unresolved` rule states no confidence.

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the registry report. It gives the wire id 74 of the
  entity type `minecraft:item`.
- `assets/mcrs/item_definition/`: the prototypes. They give the `damage_resistant` component of
  the netherite items, of `ancient_debris` and of `nether_star`.
- The public protocol documentation of Bedrock Edition, repository `Mojang/bedrock-protocol-docs`,
  for protocol version 2225. Section 7 uses its names.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `IEN-DAT` | the values of an item entity and its saved form |
| `IEN-SPN` | the start values for each cause |
| `IEN-MOV` | the motion in a fluid and the per-tick counters |
| `IEN-AGE` | the age and the removal at the end of the lifetime |
| `IEN-PCK` | the pickup by a player |
| `IEN-MRG` | the merge of two item entities |
| `IEN-DMG` | the damage and the destruction |
| `IEN-NET` | the packets |

### 0.5 Notation

`U(a, b)` is `a + r × (b - a)`, with `r` a uniform random value in `[0, 1)`. `T(m, s)` is
`m + s × (r1 - r2)`, with `r1` and `r2` two independent uniform random values in `[0, 1)`: a
triangular distribution from `m - s` to `m + s`. `θ` is `U(0, 2π)`. `yaw` and `pitch` are the look
direction of an entity in degrees, as `entity-physics.md` defines them. Numbers with the suffix `F`
are f32 values. An integer range `a..b` includes `a` and `b`.

## 1. The values of an item entity

**IEN-DAT-001** (`design`, semantic). The stack of an item entity is one stack of
`docs/item-design.md` with the placement `Dropped`. The stack and the item entity are one entity.
A drop changes the placement of the stack, and a pickup that takes all items changes it again or
merges the stack (`docs/item-design.md`, section 7.1). Reason: a stack is one entity for its life
in the model of `docs/item-design.md`. This document states the values that the placement
`Dropped` adds.

**IEN-DAT-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). An item entity has these values
in addition to the values of each entity. Results R1 to R3 show each field in the saved form.

| Value | Range | Default | Meaning |
|-------|-------|---------|---------|
| stack | a stack that is not empty | none | the items |
| age | -32768..32767 | 0 | IEN-AGE-001 |
| pickup delay | -32768..32767 | 0 | IEN-MOV-002, IEN-PCK-002 |
| health | -32768..32767 | 5 | IEN-DMG-001 |
| owner | a UUID, or no value | no value | IEN-PCK-002, IEN-MRG-004 |
| thrower | a UUID, or no value | no value | the entity that threw the stack |

**IEN-DAT-003** (`source_suggested_black_box_confirmed`, wire, confidence high). The saved form of an item entity has
the fields of each entity and these fields. Results R1 to R3 show them.

| Field | NBT type | Written when | Read when absent |
|-------|----------|--------------|------------------|
| `Item` | compound, the saved stack (STK-SAV-001) | always | the empty stack |
| `Age` | short | always | 0 |
| `PickupDelay` | short | always | 0 |
| `Health` | short | always | 5 |
| `Owner` | int array of 4, a UUID | the owner has a value | no value |
| `Thrower` | int array of 4, a UUID | the thrower has a value | no value |

**IEN-DAT-004** (`source_suggested`, semantic, confidence medium). A load of an item entity with no `Item` field, or
with the empty stack, removes the item entity at once.

**IEN-DAT-005** (`source_suggested`, semantic, confidence medium). The box of an item entity is 0.25 wide and 0.25
high (`entity-physics.md`, section 9.3). A new item entity gets a random yaw `U(0, 360)`. The yaw has
no effect that a player can see. Kind of the yaw: unspecified.

## 2. Start values for each cause

**IEN-SPN-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). The table gives the start values
for each cause. "No value" in the columns of the owner and the thrower means that the field is
absent. The age and the health start at their defaults, except where the table states a value.

| Cause | Position and velocity | Pickup delay | Thrower | Owner | Result |
|-------|-----------------------|--------------|---------|-------|--------|
| drop of the hand (`player_action` status 4 and 5) | hand form, look | 40 | the player | no value | R1, R4 |
| `drop` of a click: the throw click, a click outside the menu | hand form, look | 40 | the player | no value | none |
| drop of the creative inventory (IEN-SPN-007) | hand form, look | 40 | the player | no value | none |
| rest of the `give` command (IEN-SPN-004) | hand form, look | 0 | no value | the player | R2 |
| death of a player | hand form, scatter | 40 | no value | no value | R3 |
| the cursor stack and the craft grid when the player leaves | hand form, look | 40 | no value | no value | none |
| the rest of the cursor stack at a menu close (IEN-SPN-006) | hand form, look | 40 | no value | no value | none |
| loot of a mob death, drop of a mob | default form at the feet | 10 | no value | no value | R5 |
| drop of a block (`block_drops` true) | PHY-CMP-025 | 10 | no value | no value | R6 |
| contents of a destroyed container block | IEN-SPN-005 | 0 | no value | no value | R7 |
| contents of a destroyed item entity (IEN-DMG-005) | default form at the item entity | 0 | no value | no value | none |
| `summon item` | the command position, velocity 0 | 0 | no value | no value | R8 |

**IEN-SPN-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). The hand form of an entity at
`(x, y, z)` with the eye height `e` has the position `(x, y + e - 0.3F, z)`. It has one of two
velocities:

- look: `(-sin(yaw) × cos(pitch) × 0.3F + cos(θ) × q, -sin(pitch) × 0.3F + 0.1F + T(0, 0.1F),
  cos(yaw) × cos(pitch) × 0.3F + sin(θ) × q)`, with `q = U(0, 0.02F)`;
- scatter: `(-sin(θ) × p, 0.2F, cos(θ) × p)`, with `p = U(0, 0.5F)`.

The sine and the cosine take the angles in radians and give f32 values. Result R4 shows the look
velocity in five directions: an up part from 0.0125 to 0.1901 and a horizontal random part of 0.0198
or less. The up part of the look velocity is triangular, with the mean 0.1. Result R3 shows the
vertical start velocity 0.2 of the scatter velocity.

**IEN-SPN-003** (`black_box_confirmed`, semantic, confidence high). The server does not limit the number of drops of
the hand. Forty drops of 1 item in one burst give forty item entities. The previous inventory
experiments (result R36 of the closed inventory specification) show this.

**IEN-SPN-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). The `give` command divides the
count into parts of up to the maximum stack size. For each part, it inserts the part into the
inventory (IEN-PCK-003). Then:

- When all items of the part found room, the server makes a display item entity: the hand form with
  the look velocity, the stack with the count 1, the pickup delay 32767 and the age 5999. The age
  reaches 6000 in the next tick, and the server removes it (IEN-AGE-001). No viewer gets
  `take_item_entity` for it. Result R9 shows `add_entity`, `set_entity_data` with the count 1 and
  `remove_entities` for each viewer, also for 10 `stone`.
- When some items found no room, the rest becomes one item entity with the values of the table row
  "rest of the `give` command".

**IEN-SPN-005** (`source_suggested_black_box_confirmed`, semantic, confidence high). When a container block is
destroyed, each stack of the container becomes item entities in this way:

1. The position is `(floor(x) + U(0, 0.75) + 0.125, floor(y) + U(0, 0.75), floor(z) + U(0, 0.75) +
   0.125)`, where `(x, y, z)` is the block position. One stack has one position.
2. While the stack is not empty, take a part of `n` items, where `n` is a uniform integer in
   `10..30`, or the rest when it is smaller. Make one item entity with the part.
3. Each item entity has the velocity `(T(0, 0.11485), T(0.2, 0.11485), T(0, 0.11485))`.

Result R7 shows 64 `stone` in four parts of 12, 15, 17 and 20, each with the pickup delay 0. The
block item of the container itself is a block drop, with the pickup delay 10.

**IEN-SPN-006** (`source_suggested`, semantic, confidence medium). When a player closes a menu, the cursor stack goes
back into the inventory. The server inserts it into the slots that hold an alike stack with room,
then into the empty main slots, in the order of IEN-PCK-003. The rest that finds no room becomes an
item entity with the values of the table. When the player leaves the server, the cursor stack does
not go back: it becomes an item entity. The stacks of the craft grid of the player do the same.

**IEN-SPN-007** (`source_suggested`, semantic, confidence medium). A creative drop is a `set_creative_mode_slot`
packet with a negative slot number and a valid stack. The server keeps a counter for each
connection. Each creative drop adds 20, and each tick subtracts 1 while the counter is above 0. The
server ignores a creative drop while the counter is 1480 or more. Thus a burst can have 74 creative
drops.

**IEN-SPN-008** (`source_suggested_black_box_confirmed`, semantic, confidence high). The default form at a point
`(x, y, z)` has that position and the velocity `(U(-0.1, 0.1), 0.2, U(-0.1, 0.1))`. Result R5 shows
it for the loot of a sheep.

**IEN-SPN-009** (`source_suggested`, semantic, confidence medium). A player death drops the stacks only when the game
rule `keep_inventory` is false. The server first destroys each stack whose enchantments prevent the
drop of equipment, for example `vanishing_curse`. Then it drops each stack of the main slots, in
the order of the slots, then each equipment slot. Result R3 shows a drop of the main slots, a
helmet and the off-hand slot.

## 3. Motion and counters

**IEN-MOV-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick, an item entity
moves as the row "item" of the table of PHY-ORD-005 states, with one change. When the item entity is
in water and the water height at the item entity is more than 0.1, step `G` changes to this step:

- multiply `v.x` and `v.z` by 0.99F;
- when `v.y < 0.06F`, add 5.0E-4F to `v.y`.

The drag `D` of PHY-ORD-005 follows. Result R10 shows `v.x` 0.1, 0.09702, 0.094129, 0.091324 and
`v.y` 0, 0.00049, 0.00097, 0.00144 in the first ticks in still water.

**IEN-MOV-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick, the pickup delay
decreases by 1 when it is above 0 and is not 32767. The pickup delay 32767 never changes. Result R11
shows a pickup delay of 5 that permits a pickup some ticks later, and the value 32767 that stays.

**IEN-MOV-003** (`source_suggested`, semantic, confidence medium). The server gives an item entity the "needs sync"
mark of TRK-UPD-001 in two cases:

- the velocity at the end of the tick differs from the velocity at its start by a squared length of
  more than 0.01;
- the item entity went into a fluid or out of it in this tick.

**IEN-MOV-004** (`black_box_confirmed`, semantic, confidence high). When the `y` of an item entity is less than the
minimum `y` of the dimension minus 64, the server removes the item entity. Result R12 shows the
removal at `y` -128.1 in the overworld, and no removal at `y` -127.9.

**IEN-MOV-005** (`unresolved`, semantic). The motion of an item entity in lava. The reference
suggests the step of IEN-MOV-001 with the factor 0.95F in place of 0.99F, when the lava height is
more than 0.1. Result R10 does not agree: in lava, the first ticks show gravity and a factor of 0.75
on each axis. See U1.

## 4. Age and the end of the lifetime

**IEN-AGE-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick, the age increases
by 1, except when it is -32768. When the age is 6000 or more at the end of the tick, the server
removes the item entity. Thus an item entity with the age 0 exists for 6000 ticks, 5 minutes. The
age -32768 never changes, and the item entity stays. Result R13 shows the removal of an item entity
with the age 5990 in the tenth tick, and the age -32768 that stays.

**IEN-AGE-002** (`source_suggested`, semantic, confidence medium). Some causes set a special age: 5999 for the display
item of IEN-SPN-004, and -6000 for the `nether_star` of a wither, which then exists for 12000 ticks.

## 5. Pickup by a player

**IEN-PCK-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick of a player that is
alive and not in spectator mode, the server collects each item entity whose box intersects the
pickup box. The pickup box is one of these:

- the box of the player, larger by 1 on `x` and `z` and by 0.5 on `y`, on each side;
- when the player is a passenger of a vehicle that is not removed: the smallest box that contains
  the box of the player and the box of the vehicle, larger by 1 on `x` and `z`.

The intersection is strict: boxes that only touch do not intersect. For a player of width 0.6 and an
item entity, a horizontal distance of centres below 1.425 is in reach. Result R14 shows a pickup at
1.40 and no pickup at 1.45. The bottom of the item entity must be below the top of the player plus
0.5: result R14 shows a pickup at 2.15 above the feet of a standing player and no pickup at 2.3.

**IEN-PCK-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). A player can pick up an item
entity when each of these conditions is true:

1. The pickup delay is 0.
2. The owner has no value, or the owner is this player.
3. The insertion of IEN-PCK-003 moves 1 item or more, or the player is in creative mode
   (IEN-PCK-006).

The thrower has no effect. Result R15 shows that only the owner picks up an item entity with an
owner, and that a player picks up an item entity that another player threw.

**IEN-PCK-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). A pickup and the `give` command
insert the items with the same rule. The server puts the items into the slots in this order, each
slot up to its room:

1. the selected slot, when it holds an alike stack;
2. the off-hand slot, when it holds an alike stack;
3. the main slots that hold an alike stack, from 0 to 35;
4. the empty main slots, from 0 to 35.

A stack with a damage value above 0 goes only to the first empty main slot, as one stack. Result R16
shows the pickup into the selected slot 2 before the hotbar slot 0 and the off-hand slot, and into
the off-hand slot before the hotbar slot 0. This corrects the order of the closed inventory
specification, whose pickup cases all had the selected slot 0.

**IEN-PCK-004** (`source_suggested_black_box_confirmed`, wire, confidence high). After a pickup that takes all items,
the server sends these packets:

1. `take_item_entity` to each viewer, the collector included. Its fields are the network id of the
   item entity, the network id of the collector, and the count of the stack before the pickup.
2. `container_set_slot` with the window id 0 to the collector, for each slot that changed.
3. `remove_entities` with the item entity to each viewer.

Result R16 shows this order.

**IEN-PCK-005** (`black_box_confirmed`, semantic, confidence high). When the slots have room for only a part of the
items, the part moves to the slots, and the rest stays in the item entity. No viewer gets
`take_item_entity`. The server must send `set_entity_data` with the new stack to each viewer.
Result R17 shows the reference, which sends no packet for the item entity (D1). A later pickup of
the rest sends `take_item_entity` with the count of the rest.

**IEN-PCK-006** (`black_box_confirmed`, semantic, confidence high). For a collector in creative mode, the items that
find no room are destroyed, and the pickup takes all items. Result R18 shows a pickup of 10 `stone`
into a full inventory: `take_item_entity` with the count 10, `remove_entities`, and no slot change.

**IEN-PCK-007** (`design`, semantic). When two collectors can pick up one item entity in the same
tick, PHY-CMP-017 selects the collector. Reason: the result must not depend on the order of the
entities.

## 6. Merge of two item entities

**IEN-MRG-001** (`source_suggested`, semantic, confidence medium). An item entity looks for merge partners in a tick
when the tick count is a multiple of `k`. `k` is 2 when the block position of the item entity
changed in this tick, and 40 in the other case. An item entity that changes its dimension looks for
merge partners at once.

**IEN-MRG-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). An item entity can merge when
each of these conditions is true:

- it is not removed;
- the pickup delay is not 32767;
- the age is not -32768, and the age is less than 6000;
- the count is less than the maximum stack size.

Result R19 shows no merge for the pickup delay 32767, for the age -32768, and for a stack of 64
`stone`.

**IEN-MRG-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). The partners of an item entity
are the item entities that can merge and whose box intersects its box, larger by 0.5 on `x` and
`z`. For two item entities, this is a distance of centres below 0.75 on `x` and on `z`, and below
0.25 on `y`. Result R19 shows a merge at 0.74 and no merge at 0.76 on `x`, and a merge at 0.24 and
no merge at 0.26 on `y`.

**IEN-MRG-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). Two partners merge when each of
these conditions is true:

1. The owners are equal: both have no value, or both have the same UUID.
2. The stacks are alike.
3. The sum of the counts is not more than the maximum stack size.

The thrower has no effect. Result R19 shows no merge for different owners and for 40 and 30
`stone`, and a merge for different throwers.

**IEN-MRG-005** (`source_suggested_black_box_confirmed`, semantic, confidence high). Of two partners that merge, the
item entity with the larger count is the target. The target gets all items of the other item
entity, and the other item entity is removed. The target keeps its position, its velocity, its
owner and its thrower. Its pickup delay becomes the larger of the two pickup delays, and its age
becomes the smaller of the two ages. Result R19 shows the target with 30 items and the age of the
younger partner. When the counts are equal, the target is unspecified.

**IEN-MRG-006** (`design`, semantic). An item entity merges with its partners one at a time. Each
merge is a claim of PHY-CMP-017. Reason: the result must not depend on the order of the entities.

## 7. Damage and destruction

**IEN-DMG-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). A damage of the amount `a`
changes the health `h` to the integer part of `h - a`, toward zero. When the health is 0 or less,
the item entity is destroyed: the server removes it, and IEN-DMG-005 applies. An item entity has no
invulnerability time: it can take damage in each tick. Result R20 shows an item entity with the
health 2 on a cactus, removed one tick earlier than one with the health 5.

**IEN-DMG-002** (`black_box_confirmed`, semantic, confidence high). The table gives the damage of each block source
and the tick of the removal of an item entity with the health 5, from the first tick of contact.

| Source | Damage in each tick of contact | Removal | Result |
|--------|--------------------------------|---------|--------|
| lava | 4 | tick 2 | R20 |
| `fire` | 1, plus 1 of burning | tick 3 | R20 |
| `soul_fire` | 2, plus 1 of burning | tick 2 | R20 |
| `cactus` | 1 | tick 5 | R20 |

A burning item entity outside lava takes 1 damage in each tick in which its fire ticks are a
multiple of 20. A fire block sets the fire ticks to 160 in each tick of contact. Result R21 shows an
item entity with 200 fire ticks: 1 damage in the first tick, then none in the next 11 ticks.

**IEN-DMG-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). An explosion damages an item
entity as it damages each entity. `entity-physics.md` (PHY-EXP-002) states the group of explosions
of a tick, and a later specification states the amount. An explosion that does not affect
block-like entities, for example the explosion of a `wind_charge`, has no effect on an item entity.
Result R22 shows the destruction of `stone` at 1, 3, 5 and 7 blocks from a primed TNT.

**IEN-DMG-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). An item entity takes no damage
in each of these cases:

- The `damage_resistant` component of the stack names a tag that contains the damage type. Such an
  item entity also does not burn when the tag contains `in_fire`. Results R20 and R22 show
  `netherite_ingot` in lava and `nether_star` in a TNT explosion with no damage.
- The entity is invulnerable, and the damage type does not bypass invulnerability.
- The game rule `mob_griefing` is false, and the source entity of the damage is a mob.

A player cannot attack an item entity.

**IEN-DMG-005** (`source_suggested`, semantic, confidence medium). When an item entity is destroyed, the items inside
its stack drop: the stacks of the `container` component and of the `bundle_contents` component.
Each stack becomes one item entity with the default form at the position of the destroyed item
entity, and the pickup delay 0.

## 8. Packets

**IEN-NET-001** (`source_suggested_black_box_confirmed`, wire, confidence high). The spawn bundle of an item entity
(TRK-SPN-001) has `add_entity` with the type id 74, then `set_entity_data` with the stack (PHY-CMP-026).
The results show a second `set_entity_data` with the same stack in the first update after the spawn
(TRK-UPD-001). The second packet has no visible effect. Kind of the second packet: unspecified.

**IEN-NET-002** (`source_suggested`, wire, confidence medium). The type data of `minecraft:item` has the client
tracking range 6 and the update interval 20 (`entity-tracking.md`, section 1). The position and the
velocity updates follow PHY-CMP-002 and TRK-UPD-002. The "needs sync" mark of IEN-MOV-003 adds
updates between the periodic ones.

**IEN-NET-003** (`source_suggested_black_box_confirmed`, wire, confidence high). Each change of the stack of an item
entity sends `set_entity_data` with the new stack to each viewer at the next update. A merge
changes the stack of the target. A partial pickup changes it too (IEN-PCK-005).

**IEN-NET-004** (`source_suggested_black_box_confirmed`, wire, confidence high). Each removal of an item entity sends
`remove_entities` to each viewer (TRK-REM-001): the end of the lifetime, the destruction, the merge
into a target, the removal below the world and the pickup. Only a pickup sends `take_item_entity`
first.

## 9. What this document does not state

- The pickup of an item entity by a hopper, a mob, an allay, a fox or a dolphin.
- The amount of the damage of an explosion (IEN-DMG-003).
- The loot tables, and which stacks a block or a mob drops.
- The clicks that make a dropped stack (ITX-OP-020, ITX-JAV-013) and the packets for the slots of
  the player after a drop.
- The statistics and the advancements of a drop and a pickup.
- The sounds of a pickup and of a burning item entity.
- The client drawing of an item entity: its spin, its bob and the take animation.

## 10. Edition differences

Each entry has the fields that issue #656 proposes. No Bedrock experiment has a recorded run, so
each Bedrock value comes from the public protocol documentation or is not known yet.

**ED1.**

- **Rule**: IEN-NET-001, IEN-NET-004, IEN-PCK-004.
- **Java**: `add_entity` and `set_entity_data` show an item entity. `take_item_entity` and
  `remove_entities` end it.
- **Bedrock**: the documentation names `AddItemActor`, which carries the stack, and
  `TakeItemActor` and `RemoveActor`.
- **Decision**: each edge sends the packets of its edition. The state of the item entity is the same.
- **Selected by**: client edition.
- **Visible to the client**: yes, wire.
- **Prediction risk**: none.
- **Parity list or bug reference**: none.

**ED2.**

- **Rule**: IEN-PCK-001, IEN-MRG-003, IEN-AGE-001, IEN-SPN-001.
- **Java**: the pickup box, the merge range, the lifetime of 6000 ticks and the pickup delays of
  this document.
- **Bedrock**: not known. Experiment B1.
- **Decision**: the Java values apply to both editions until B1 has a result.
- **Selected by**: world rule.
- **Visible to the client**: yes.
- **Prediction risk**: low. The client of each edition shows the pickup and the merge that the
  server sends.
- **Parity list or bug reference**: none.

**ED3.**

- **Rule**: IEN-PCK-002, IEN-MRG-004.
- **Java**: the owner limits the pickup and the merge. The thrower limits neither.
- **Bedrock**: not known. Experiment B2.
- **Decision**: the Java rule applies to both editions until B2 has a result.
- **Selected by**: world rule.
- **Visible to the client**: yes.
- **Prediction risk**: none.
- **Parity list or bug reference**: none.

## 11. Intentional deviations, open questions and unresolved items

### 11.1 Intentional deviations

**D1.**

- **Rule**: IEN-PCK-005.
- **Reference**: after a partial pickup, the stack of the item entity has fewer items, but no viewer
  gets `set_entity_data`. Each viewer shows the old count until another change of the stack.
- **Visible to the client**: yes. The item entity shows the old count.
- **Test**: a differential test compares the count of the saved form, not the `set_entity_data`
  packets after a partial pickup.

**D2.**

- **Rule**: IEN-MRG-005.
- **Reference**: a merge moves items only up to the count 64. For a stack with a maximum stack size
  above 64, 50 and 40 items give a target of 64 and a partner of 26 items that both stay.
- **Visible to the client**: yes, only for a stack whose maximum stack size is above 64.
- **Test**: a differential test must not merge stacks with a maximum stack size above 64.

### 11.2 Open questions

For the owner:

- **Q1**: D2. Does mcrs keep the limit 64 of the reference for parity, or move all items? Result R19
  shows the reference.
- **Q2**: IEN-PCK-003. `inventory-transactions.md`, section 8, gives the insert rule of the player
  inventory to a later specification. This document states it for the pickup and for `give`. Does
  the rule move to the specification of the player inventory when it exists?
- **Q3**: IEN-DAT-001. The model of `docs/item-design.md` keeps one entity for a stack. Does the
  item entity get a new network id when a stack goes from a slot to the world, as in the reference,
  or does the stack keep one id? The id that the client sees must be new for each `add_entity`.

### 11.3 Unresolved

- **U1**: IEN-MOV-005. The motion in lava. Result R10 shows a factor of 0.75 on each axis and
  gravity, not the expected step. The test summoned `netherite_ingot` into lava that flows.
  Experiment E14 must repeat it in a lava source with no flow.
- **U2**: IEN-DMG-004. The creeper case of `mob_griefing`. Result R22 shows no damage at 3 blocks
  from a creeper both with `mob_griefing` false and true, so the test did not prove an explosion.
- **U3**: IEN-SPN-001. The rows with no result: the throw click, the click outside the menu, the
  creative drop, the menu close and the leave of a player.

## 12. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`, `level-type=minecraft:flat`, `network-compression-threshold=-1` and the game
rule `spawn_mobs` false. Connect one scripted client as the collector `P` at `(0.5, -60, 0.5)` and
one as the viewer `W` 4 blocks away. Use `tick freeze` and `tick step` to count ticks. Read the item
entities with `data get entity`. Record the clientbound packets of each client after each step.

| Experiment | Steps | Result |
|------------|-------|--------|
| E1 | Put 10 `stone` in the hotbar slot 0 of `P`. Send `player_action` with the status 5. Freeze the ticks and read the item entity. | R1 |
| E2 | Fill the 36 main slots of `P` with `dirt`. Run `give P stone 10`. Read the item entity. Clear `P` and run `give P stone 10` again. | R2, R9 |
| E3 | Put stacks in two main slots, the head slot and the off-hand slot of `P`. Run `kill P` and read the item entities. | R3 |
| E4 | Drop the hand 12 times in each of five look directions. Read `Motion` after the spawn. | R4 |
| E5 | Summon a sheep with `NoAI`. Freeze the ticks and kill it. Read the item entities. | R5 |
| E6 | Place `oak_log`, then run `setblock` with `air` and `destroy`. Read the item entity. | R6 |
| E7 | Place a chest with 64 `stone` and 1 `dirt`. Destroy it as in E6. | R7 |
| E8 | Summon item entities with given fields at `P` and at given distances, with `NoGravity`. | R11, R14, R15 |
| E9 | Set the selected slot of `P` to 2. Put 60 `stone` in the hotbar slots 0 and 2 and in the off-hand slot. Summon 4 `stone` at `P`. Repeat with the hotbar slot 2 empty. | R16 |
| E10 | Put 60 `stone` in window 0 slot 35 of `P`. Fill the other 35 main slots with `dirt`. Summon 10 `stone` at `P`. Wait 2 seconds. Clear the `dirt`. | R17 |
| E11 | Fill the 36 main slots of `P` with `dirt` in creative mode. Summon 10 `stone` at `P`. | R18 |
| E12 | Summon pairs of `stone` item entities with `NoGravity` at the distances and with the fields of R19. Step 45 ticks. | R19 |
| E13 | Summon item entities with the ages 5990 and -32768 and with the pickup delay 32767. Step 9 ticks, then 1 tick. | R13 |
| E14 | Summon an item entity with `Motion` `(0.1, 0, 0)` in water and in lava. Summon an item entity in `fire`, in `soul_fire`, on a `cactus`, and with `Fire` 200. Step 1 tick at a time and read `Motion`, `Health` and if it exists. | R10, R20, R21 |
| E15 | Summon item entities at `y` -127.9 and -128.1. Step 1 tick. | R12 |
| E16 | Summon `stone` and `nether_star` 1 to 7 blocks from a TNT with `fuse` 1. Step 3 ticks. | R22 |
| E17 | Not run. Send a throw click, a click outside the menu and a creative drop. Close a menu with a cursor stack and a full inventory. Disconnect with a cursor stack. Read each item entity. | U3 |
| E18 | Not run. Repeat the lava part of E14 in a lava source with solid blocks on all sides. | U1 |
| E19 | Not run. Summon a creeper that explodes with certainty, with `mob_griefing` false and true. | U2 |
| B1 | Not run. On a Bedrock server, measure the pickup reach, the merge range, the lifetime and the pickup delay of a drop of the hand. | ED2 |
| B2 | Not run. On a Bedrock server, drop an item and try a pickup by another player at once and after the pickup delay. | ED3 |

## 13. Captured results

Each experiment ran on 2026-11-11 against the vanilla server of `26.4-snapshot-7`. Each packet is
shortened to its name and the fields that matter. `P` is the collector and `W` is the viewer.
Values in the saved form are shortened.

**R1.** E1, the drop of the hand, 4 ticks after the spawn.

```text
Item {count: 1, id: stone}, Thrower [I; <UUID of P>], PickupDelay 36s, Age 4s, Health 5s, no Owner
W: add_entity id 12 type 74 pos (0.5, -58.68, 0.5); set_entity_data count 1; set_entity_data count 1
```

**R2.** E2, the rest of `give` into a full inventory, 3 ticks after the spawn.

```text
Item {count: 10, id: stone}, Owner [I; <UUID of P>], PickupDelay 0s, Age 3s, Health 5s, no Thrower
Motion (-0.0059, -0.0443, 0.2901)
```

**R3.** E3, the death of `P` with the look direction (0, 0), 2 ticks after the spawn. Four item
entities: 10 `stone`, 5 `dirt`, 3 `stick`, 1 `iron_helmet`.

```text
each: PickupDelay 38s, no Thrower, no Owner, Pos y -58.4032, Motion y 0.114464
Motion x, z: (-0.0811, -0.0393) (-0.0089, 0.1888) (-0.0153, 0.0222) (0.0098, -0.3261)
```

The `y` velocity agrees with a start of 0.2 after two ticks of gravity and drag.

**R4.** E4, the velocity of a drop of the hand. `look` is 0.3 times the unit vector of the look
direction. 12 drops for each direction, from the closed inventory specification (its result R37).

```text
direction  y - look y          horizontal - look
0 0        0.0400 to 0.1901    0.0187 or less
90 0       0.0219 to 0.1854    0.0197 or less
0 -90      0.0125 to 0.1770    0.0170 or less
0 90       0.0555 to 0.1806    0.0198 or less
45 30      0.0706 to 0.1596    0.0198 or less
```

**R5.** E5, the loot of a sheep, in the tick of the death.

```text
mutton x2, white_wool x1: PickupDelay 10s, Pos (20.5, -60.0, 0.5), Motion (-0.0417, 0.2, -0.0280) and (0.0991, 0.2, 0.0466)
```

**R6.** E6, the drop of `oak_log`, 1 tick after the spawn: `PickupDelay 9s`.

**R7.** E7, the drop of a chest.

```text
chest x1 PickupDelay 10s; stone x12, x15, x17, x20 and dirt x1, each PickupDelay 0s
```

**R8.** E12, `summon item` with `NoGravity` and no `Motion`: `Motion` is `(0, 0, 0)` after 45
ticks. The summon gives no random velocity.

**R9.** E2, `give P stone 10` into an empty inventory.

```text
P: add_entity id 14; set_entity_data count 1; container_set_slot slot 36 count 10; remove_entities [14]
W: add_entity id 14; set_entity_data count 1; remove_entities [14]
no item entity after the command
```

**R10.** E14, the motion in a fluid. The item entity starts with the velocity `(0.1, 0, 0)`.

```text
water, ticks 1 to 4:  v (0.097020, 0.000490, 0) (0.094129, 0.000970, 0) (0.091324, 0.001441, 0) (0.088602, 0.001902, 0)
lava that flows, netherite_ingot, ticks 1 to 2:  v (0.0735, -0.0294, -0.1009) (0.0540, -0.0510, -0.1801)
```

**R11.** E8, a pickup delay of 5: no packet in the first 3 ticks, then `take_item_entity`,
`container_set_slot` and `remove_entities`. E13: the pickup delay 32767 stays 32767 after 10 ticks.

**R12.** E15: the item entity at `y` -128.1 is removed in tick 1. The item entity at -127.9 exists
after 5 ticks.

**R13.** E13.

```text
after 9 ticks: the three item entities exist
after 10 ticks: the item entity with the age 5990 is removed; the age -32768 stays -32768
```

**R14.** E8, the pickup box. `P` stands at `(0.5, -60, 0.5)`. The item entity has `NoGravity`.

```text
dx 1.40: picked up      dx 1.45: not picked up      dx 1.40, dz 1.40: picked up
dy 2.15: picked up      dy 2.30: not picked up
```

**R15.** E8, the owner and the thrower.

```text
Owner W, at P: P gets nothing; W goes to P: W gets take_item_entity collector W, container_set_slot slot 36 count 3, remove_entities
Thrower W, at P: P gets take_item_entity, container_set_slot slot 36 count 3, remove_entities
```

**R16.** E9, the insertion order. Slot numbers are the menu indices of window 0: 36 to 44 are the
hotbar, 45 is the off-hand slot.

```text
selected 2; hotbar 0, hotbar 2, off-hand: stone x60 each; summon stone x4
  P: take_item_entity count 4; container_set_slot slot 38 count 64; remove_entities
  W: take_item_entity count 4; remove_entities
selected 2 empty; hotbar 0, off-hand: stone x60 each; summon stone x4
  P: take_item_entity count 4; container_set_slot slot 45 count 64; remove_entities
```

**R17.** E10, the partial pickup.

```text
P: container_set_slot slot 35 count 64
W: nothing, also after 2 seconds; the saved count of the item entity is 6
after the dirt is cleared: P and W: take_item_entity count 6; P: container_set_slot slot 36 count 6; remove_entities
```

**R18.** E11, the creative pickup into a full inventory.

```text
P and W: take_item_entity count 10; remove_entities; no container_set_slot; no item entity after
```

**R19.** E12, the merge, after 45 ticks. `a` is summoned first.

```text
a 10 (PickupDelay 30, Age 100), b 20 (PickupDelay 5, Age 7), dx 0.5  -> b 30, Age 52
a 10, b 20, dx 0.74 -> b 30            a 10, b 20, dx 0.76 -> no merge
a 10, b 20, dy 0.24 -> b 30            a 10, b 20, dy 0.26 -> no merge
a 40, b 30 -> no merge                 a 10 Owner W, b 20 -> no merge
a 10 Thrower W, b 20 -> b 30           a 10, b 10 -> b 20
a 50, b 40, max_stack_size 99 -> a 64, b 26
a 10 PickupDelay 32767, b 20 -> no merge
a 10 Age -32768, b 20 -> no merge      a 64, b 1 -> no merge
```

**R20.** E14, the tick of the removal of `stone` with the health 5, from the summon.

```text
lava 2; fire 3; soul_fire 2; on cactus 6 (it falls 0.1 block first); on cactus with Health 2: 3
netherite_ingot in lava: exists after 30 ticks with Health 5
```

**R21.** E14, `Fire` 200 on `stone` with `NoGravity`: `Health/Fire` per tick.

```text
4/199 4/198 4/197 4/196 4/195 4/194 4/193 4/192 4/191 4/190 4/189 4/188
```

**R22.** E16, explosions, with `block_drops` false.

```text
stone at 1, 3, 5 and 7 blocks from a TNT: removed
nether_star at 1 block: Health 5
creeper 3 blocks away, mob_griefing false and true: stone Health 5 in both cases
```
