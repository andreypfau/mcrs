# Item entity: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the item entity: a stack that lies in the world. It states the start values of
an item entity for each cause, its motion in a fluid, its age, its pickup by a player, the merge of
two item entities, its damage, its saved form and its packets.

`item-stack.md` states the stack.
`inventory-transactions.md` states the `drop` operation that makes a dropped stack (ITX-OP-020).
`entity-physics.md` states the move, the gravity and the drag of an item on land and in air
(PHY-ORD-005). `entity-tracking.md` states the spawn bundle, the updates and the removal of an
entity. This document refers to the rules of these documents by id.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**: as `login.md`,
  section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **maximum stack size**: as `item-stack.md`, section 0.1,
  defines them.
- **Dropped stack**: as `inventory-transactions.md`, section 0.1, defines it.
- **Placement**, **holder**: as `docs/item-design.md` defines them.
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
- **Tick count**: a counter that is 0 at the spawn and increases by 1 at the start of each tick
  (IEN-MOV-006, step 1). Thus the first tick reads 1. A load sets it to 0. It is not the age.
- **Fluid box**: the box of the item entity, smaller by 0.001 on each side.
- **Fluid height**: for one fluid, the largest value of the fluid top minus the bottom of the box
  of the item entity, over the blocks of that fluid that the item entity touches. The bottom is the
  bottom of the box, not of the fluid box.
- **Fluid update**: a step that reads the fluids in the fluid box. It finds the fluid height of each
  fluid and applies the current (IEN-MOV-007). A tick has two fluid updates (IEN-MOV-006).

### 0.2 Provenance

- `black_box_confirmed`: shown by a result of an experiment.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a result of an experiment.
- `source_suggested`: taken from the reference implementation, with no result. The
  confidence is at most medium.
- `documentation`: taken from public documentation of a protocol.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 11 collects these items.

Each rule states its kind, as `docs/spec/README.md` defines it. A rule with a result of an experiment
states confidence high. A `source_suggested` rule states confidence medium. A `design` or
`unresolved` rule states no confidence.

The reference is not a correct implementation. A rule states the behaviour that a player expects,
not each accident of the reference. Each accident that this document knows is a deviation (`D`)
in section 11.1, and the rule that corrects it names the entry. Where the order of the reference
has no meaning for a player, the rule states a bound and gives the exact value the kind
unspecified. Section 11.2 keeps an accident only when the vanilla client depends on it.

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
in addition to the values of each entity.

| Value | Range | Default | Meaning |
|-------|-------|---------|---------|
| stack | a stack that is not empty | none | the items |
| age | -32768..32767 | 0 | IEN-AGE-001 |
| pickup delay | -32768..32767 | 0 | IEN-MOV-002, IEN-PCK-002 |
| health | -32768..32767 | 5 | IEN-DMG-001 |
| owner | a UUID, or no value | no value | IEN-PCK-002, IEN-MRG-004 |
| thrower | a UUID, or no value | no value | the entity that threw the stack |

**IEN-DAT-003** (`source_suggested_black_box_confirmed`, wire, confidence high). The saved form of an item entity has
the fields of each entity and these fields.

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

| Cause | Position and velocity | Pickup delay | Thrower | Owner |
|-------|-----------------------|--------------|---------|-------|
| drop of the hand (`player_action` status 4 and 5) | hand form, look | 40 | the player | no value |
| `drop` of a click: the throw click, a click outside the menu | hand form, look | 40 | the player | no value |
| drop of the creative inventory (IEN-SPN-007) | hand form, look | 40 | the player | no value |
| rest of the `give` command (IEN-SPN-004) | hand form, look | 0 | no value | the player |
| rest of an item reward of an advancement | hand form, look | 0 | no value | the player |
| death of a player | hand form, scatter | 40 | no value | no value |
| the rest of the cursor stack and of the craft grid at a menu close or a leave (IEN-SPN-006) | hand form, look | 40 | no value | no value |
| loot of a mob death, drop of a mob | default form at the feet | 10 | no value | no value |
| drop of a block (`block_drops` true) | PHY-CMP-025 | 10 | no value | no value |
| contents of a destroyed container block | IEN-SPN-005 | 0 | no value | no value |
| contents of a destroyed item entity (IEN-DMG-005) | default form at the item entity | 0 | no value | no value |
| `summon item` | the command position, velocity 0 | 0 | no value | no value |

The table is not complete.

**IEN-SPN-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). The hand form of an entity at
`(x, y, z)` with the eye height `e` has the position `(x, y + e - 0.3F, z)`. It has one of two
velocities:

- look: `(-sinT(a) × cosT(b) × 0.3F + cos(θ) × q, -sinT(b) × 0.3F + 0.1F + T(0, 0.1F),
  cosT(a) × cosT(b) × 0.3F + sin(θ) × q)`, with `q = U(0, 0.02F)`;
- scatter: `(-sinT(θ) × p, 0.2F, cosT(θ) × p)`, with `p = U(0, 0.5F)`.

`a` and `b` are the yaw and the pitch in radians, as PHY-NUM-005 converts a yaw. `θ` and `q` are
f32 values; `θ` is `U(0, f32(2π))`. `sinT` and `cosT` read the sine table of PHY-NUM-005 and give
f32 values. `sin` and `cos` in the random part of the look velocity are the exact f64 functions.
Thus the `x` and `z` parts of the look velocity are f64 sums, and the `y` part is an f32 value. The
scatter velocity has f32 parts. The up part of the look velocity is triangular, with the mean 0.1.

**IEN-SPN-003** (`source_suggested`, semantic, confidence medium). The server does not limit the
number of drops of the hand. Forty drops of 1 item in one burst give forty item entities.

**IEN-SPN-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). The `give` command divides the
count into parts of up to the maximum stack size. For each part, it inserts the part into the
inventory (IEN-PCK-003). Then:

- When all items of the part found room, the server makes a display item entity: the hand form with
  the look velocity, the stack with the count 1, the pickup delay 32767 and the age 5999. The age
  reaches 6000 in the next tick, and the server removes it (IEN-AGE-001). No viewer gets
  `take_item_entity` for it.
- When some items found no room, the rest becomes one item entity with the values of the table row
  "rest of the `give` command".

The display item entity is not an item in the world. It cannot be picked up or merged, and it is
not saved.

These steps apply also to a player in creative mode (D6).

**IEN-SPN-005** (`source_suggested_black_box_confirmed`, semantic, confidence high). When a container block is
destroyed, each stack of the container becomes item entities in this way:

1. The position is `(floor(x) + U(0, 0.75) + 0.125, floor(y) + U(0, 0.75), floor(z) + U(0, 0.75) +
   0.125)`, where `(x, y, z)` is the block position. One stack has one position.
2. While the stack is not empty, take a part of `n` items, where `n` is a uniform integer in
   `10..30`, or the rest when it is smaller. Make one item entity with the part.
3. Each item entity has the velocity `(T(0, s), T(0.2, s), T(0, s))`, with `s` the f64 value of
   `0.11485F`, 0.11485000171139836.
 The
block item of the container itself is a block drop, with the pickup delay 10.

**IEN-SPN-006** (`source_suggested`, semantic, confidence medium). When a player closes a menu, the cursor stack goes
back into the inventory. The server inserts it into the slots that hold an alike stack with room,
then into the empty main slots, in the order of IEN-PCK-003. The rest that finds no room becomes an
item entity with the values of the table. The stacks of the craft grid of the player do the same.
A leave of the player has the same result (`inventory.md`, INV-CLS-003).

**IEN-SPN-007** (`source_suggested`, semantic, confidence medium). A creative drop is a `set_creative_mode_slot`
packet with a negative slot number from a player in creative mode. The stack must be empty, or have a
count of at most its maximum stack size. The server keeps a counter for each
connection. Each creative drop adds 20, and each tick subtracts 1 while the counter is above 0. The
server ignores a creative drop while the counter is 1480 or more. Thus a burst can have 74 creative
drops. A creative drop with the empty stack adds 20 to the counter and makes no item entity.

**IEN-SPN-008** (`source_suggested_black_box_confirmed`, semantic, confidence high). The default form at a point
`(x, y, z)` has that position and the velocity `(U(-0.1, 0.1), 0.2, U(-0.1, 0.1))`.

**IEN-SPN-009** (`source_suggested`, semantic, confidence medium). A player death drops the stacks only when the game
rule `keep_inventory` is false. The server first destroys each stack whose enchantments prevent the
drop of equipment, for example `vanishing_curse`. Then it drops each stack of the main slots, in
the order of the slots. Then it drops each equipment slot in this order: off-hand, feet, legs,
chest, head, body, saddle. The order of the drops gives the order of the network ids.

**IEN-SPN-010** (`source_suggested`, wire, confidence medium). A drop of a player swings the main
hand. The server sends `swing_animation` for the main hand to each viewer of the player. The rows
of IEN-SPN-001 give the receivers:

- the drop of the hand, the throw click, the click outside the menu and the creative drop: the
  viewers of the player, not the player, because the client of the player shows the swing itself;
- the rest at a menu close, the leave of a player and the rest of an advancement reward: the
  viewers of the player and the player;
- the `give` command and the death of a player: no swing.

A drop sends no swing while the main hand swings already.

## 3. Motion and counters

**IEN-MOV-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick, an item entity
moves as the row "item" of the table of PHY-ORD-005 states, with one change. When the item entity is
in water and the fluid height of water is more than 0.1F, step `G` changes to this step.
The water height is the value of the fluid update of the tick (IEN-MOV-006, step 3).

- multiply `v.x` and `v.z` by 0.99F;
- when `v.y < 0.06F`, add 5.0E-4F to `v.y`.

The drag `D` of PHY-ORD-005 follows.

**IEN-MOV-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick, the pickup delay
decreases by 1 when it is above 0 and is not 32767. The pickup delay 32767 never changes.

**IEN-MOV-003** (`source_suggested`, wire, confidence medium). The server gives an item entity the
"needs sync" mark of TRK-UPD-001 in two cases:

- the velocity changed by a length of more than 0.1 from the start of step 6 to the end of step 9
  of IEN-MOV-006;
- at the end of the tick, the item entity touches a fluid (IEN-MOV-006, step 9).

An item entity touches a fluid when its fluid box overlaps a block with a fluid, and the surface of
that fluid is at or above the bottom of the fluid box. Thus an item entity in a fluid gets the mark
in each tick. The vanilla client simulates the motion of an item entity itself, so this mark also
corrects the client after D3, D4 and D5.

**IEN-MOV-004** (`black_box_confirmed`, semantic, confidence high). When the `y` of an item entity is less than the
minimum `y` of the dimension minus 64, the server removes the item entity.

**IEN-MOV-005** (`source_suggested`, semantic, confidence medium). When the item entity is not
under the condition of IEN-MOV-001, is in lava, and the fluid height of lava is more than 0.1F,
step `G` changes to the step of IEN-MOV-001 with the factor 0.95F in place of 0.99F. Water takes
priority over lava.

**IEN-MOV-006** (`source_suggested`, semantic, confidence medium). The server does the steps of the
tick of an item entity in this order:

1. Add 1 to the tick count.
2. When the stack is empty, remove the item entity, and stop.
3. Do the fluid update (IEN-MOV-007). In this step, an item entity can also go through a portal.
4. Apply the burn of IEN-DMG-002 and the removal below the world of IEN-MOV-004.
5. Decrease the pickup delay (IEN-MOV-002).
6. Move the item entity (IEN-MOV-001). The block effects of the move apply in this step, for
   example the damage of lava, `fire` and `cactus`.
7. Look for merge partners (IEN-MRG-001). The merge reads the age before step 8.
8. Increase the age (IEN-AGE-001).
9. Find whether the item entity touches a fluid at its new position, and set the "needs sync"
   mark (IEN-MOV-003). This step applies no current (D3).
10. When the age is 6000 or more, remove the item entity.

A pickup is not a step of this tick. It occurs in the tick of the player (IEN-PCK-001). For
example, an item entity with the age 5999 can merge in step 7 and is then removed in step 10.

**IEN-MOV-007** (`source_suggested`, semantic, confidence medium). The fluid update applies one
current for each kind of fluid that the item entity touches, once in a tick (D3). A kind of fluid
is a fluid tag: `minecraft:water` or `minecraft:lava`. A source block and a flowing block of one
kind are the same kind (D5). For one kind:

1. Let `h` be the fluid height of the kind. Let `S` be the sum of the flow vectors of the blocks of
   the kind that the item entity touches. When `h < 0.4`, multiply `S` by `h` (D4).
2. When `|S|²` is less than 1.0E-5F, the kind adds no current.
3. Otherwise the current of the kind is `normalize(S) × 2s`. `s` is 0.014 for water. For lava, `s`
   is 0.0023333333333333335, or 0.007 in a dimension with the attribute `fast_lava`.

Let `I` be the sum of the currents of all kinds. When `(v.x + I.x)² + (v.z + I.z)²` is less than
9.0E-6, and `|I|` is more than 2.0E-5 and less than 0.009, scale `I` to the length 0.009. Then add
`I` to `v`. The factor 2 and the length 0.009 keep the speed of an item entity in a current near
the speed in the reference, which applies the current two times in a tick.

When a chunk that holds a block of the fluid box, larger by 1 on `x` and `z`, is not loaded, the
fluid update finds no fluid and applies no current. The flow vector of a fluid block is not stated here.

## 4. Age and the end of the lifetime

**IEN-AGE-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick, the age increases
by 1, except when it is -32768. When the age is 6000 or more at the end of the tick, the server
removes the item entity. Thus an item entity with the age 0 exists for 6000 ticks, 5 minutes. The
age -32768 never changes, and the item entity stays.

**IEN-AGE-002** (`source_suggested`, semantic, confidence medium). Some causes set a special age: 5999 for the display
item of IEN-SPN-004, and -6000 for the `nether_star` of a wither, which then exists for 12000 ticks.

**IEN-AGE-003** (`design`, semantic). An item entity whose item is in the item tag
`mcrs:item_never_despawns` is not removed at the end of its lifetime. Its age does not increase.
The conditions on the age in IEN-MRG-002 do not apply to it. The tag is empty by default. Reason:
a data pack gives the Bedrock lifetime of ED4 without a change of code. An item in the tag stays
without a time limit, so the tag must stay small.

## 5. Pickup by a player

**IEN-PCK-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). In each tick of a player that is
alive and not in spectator mode, the server collects each item entity whose box intersects the
pickup box. The pickup box is one of these:

- the box of the player, larger by 1 on `x` and `z` and by 0.5 on `y`, on each side;
- when the player is a passenger of a vehicle that is not removed: the smallest box that contains
  the box of the player and the box of the vehicle, larger by 1 on `x` and `z`.

In one server tick, the pickups occur after the ticks of all item entities in all dimensions
(IEN-MOV-006). Thus an item entity whose pickup delay becomes 0 in its tick can be picked up in
the same server tick. An item entity that is removed in its tick cannot be picked up. Provenance
of this paragraph: `source_suggested`, confidence medium.

The intersection is strict: boxes that only touch do not intersect. For a player of width 0.6 and an
item entity, a horizontal distance of centres below 1.425 is in reach. The bottom of the item entity must be below the top of the player plus
0.5.

**IEN-PCK-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). A player
inserts the stack of an item entity into the slots (IEN-PCK-003) when each of these conditions is
true:

1. The pickup delay is 0.
2. The owner has no value, or the owner is this player.

The insertion has one of three results:

- **Full pickup**: all items find room, or the player is in creative mode (IEN-PCK-006). IEN-PCK-004
  applies.
- **Partial pickup**: some items find room, and some items do not. IEN-PCK-005 applies.
- **No pickup**: no item finds room. Nothing changes.

The thrower has no effect.

**IEN-PCK-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). A pickup and the `give` command
insert the items with the insert rule of the player inventory. `inventory.md` states that rule
(INV-INS-001).

**IEN-PCK-004** (`source_suggested_black_box_confirmed`, wire, confidence high). After a pickup that takes all items,
the server sends these packets:

1. `take_item_entity` to each viewer, the collector included. Its fields are the network id of the
   item entity, the network id of the collector, and the count of the stack before the pickup.
2. `container_set_slot` with the window id 0 to the collector, for each slot that changed.
3. `remove_entities` with the item entity to each viewer.


**IEN-PCK-005** (`design`, client agreement). In a partial pickup, the items that find room move
to the slots, and the rest stays in the item entity. The server sends `take_item_entity` to each
viewer, with the count of the items that moved. The vanilla client subtracts that count from the
stack that it shows, and removes the item entity only when the stack is empty. The server also
sends `set_entity_data` with the new stack (IEN-NET-003). Each viewer must get `take_item_entity`
before that `set_entity_data`. In the other order, the client subtracts the count from the new
stack, and shows too few items. The server sends no `remove_entities`. A later full pickup of the
rest sends `take_item_entity` with the count of the rest. A partial pickup is a pickup of the
items that moved, for each effect of a pickup, for example a statistic. Reason: D1.

**IEN-PCK-006** (`black_box_confirmed`, semantic, confidence high). For a collector in creative mode, the items that
find no room are destroyed, and the pickup takes all items.

**IEN-PCK-007** (`design`, semantic). When two collectors can pick up one item entity in the same
tick, PHY-CMP-017 selects the collector. Reason: the result must not depend on the order of the
entities.

## 6. Merge of two item entities

**IEN-MRG-001** (`source_suggested`, semantic, confidence medium). An item entity that can merge
looks for merge partners at least once in each 40 ticks. When its block position changed in the
tick, it looks at least once in each 2 ticks. Kind of the exact tick of a search: unspecified. The
reference looks when the tick count is a multiple of `k`, with `k` 2 or 40 as above. An item
entity that teleports looks for merge partners at once. This applies to a teleport in the same dimension, for example `tp` or an
`end_gateway`, and to a change of the dimension.

**IEN-MRG-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). An item entity can merge when
each of these conditions is true:

- it is not removed;
- the pickup delay is not 32767;
- the age is not -32768, and the age is less than 6000;
- the count is less than the maximum stack size.


**IEN-MRG-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). The partners of an item entity
are the item entities that can merge and whose box intersects its box, larger by 0.5 on `x` and
`z`. For two item entities, this is a distance of centres below 0.75 on `x` and on `z`, and below
0.25 on `y`.

**IEN-MRG-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). Two partners merge when each of
these conditions is true:

1. The owners are equal: both have no value, or both have the same UUID.
2. The stacks are alike.
3. The sum of the counts is not more than the maximum stack size.

The thrower has no effect.

**IEN-MRG-005** (`source_suggested_black_box_confirmed`, semantic, confidence high). Of two partners that merge, the
item entity with the larger count is the target. The target gets all items of the other item
entity, and the other item entity is removed. The target keeps its position, its velocity, its
owner and its thrower. Its pickup delay becomes the larger of the two pickup delays, and its age
becomes the smaller of the two ages. When the counts are equal, the kind of the target is unspecified: a parity test
compares the stack and the count, not which item entity stays. In the reference, the item entity
that looks for partners gives its items. A merge moves all
items of the other item entity, also above the count 64 (D2).

**IEN-MRG-006** (`design`, semantic). Each merge is a claim of PHY-CMP-017. An item entity takes
part in at most one merge in a tick. A later merge waits for its next search (IEN-MRG-001).
Reason: the result must not depend on the order of the entities, and the delay stays within the
bound of IEN-MRG-001.

## 7. Damage and destruction

**IEN-DMG-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). A damage of the amount `a`
changes the health `h` to the integer part of `h - a`, toward zero. When the health is 0 or less,
the item entity is destroyed: the server removes it, and IEN-DMG-005 applies. An item entity has no
invulnerability time: it can take damage in each tick.

Each damage that the item entity takes also sends `set_entity_motion` to each viewer at the end of
the next tracker tick, as for a hurt subject (TRK-UPD-007). This applies also when the item entity
is not destroyed.

**IEN-DMG-002** (`black_box_confirmed`, semantic, confidence high). The table gives the damage of each block source
and the tick of the removal of an item entity with the health 5, from the first tick of contact.

| Source | Damage in each tick of contact | Removal |
|--------|--------------------------------|---------|
| lava | 4 | tick 2 |
| `fire` | 1, plus 1 of burning | tick 3 |
| `soul_fire` | 2, plus 1 of burning | tick 2 |
| `cactus` | 1 | tick 5 |

A burning item entity outside lava takes 1 damage in each tick in which its fire ticks are a
multiple of 20. In each tick of contact, a fire block changes the fire ticks. When they are
negative, it adds 1. Then, when they are 0 or more, it raises them to 160 when they are less than
160. It does not decrease a larger value. A new item entity has the fire ticks 0, so the first
tick of contact sets 160. Only a saved `Fire` field gives negative fire ticks.

**IEN-DMG-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). An explosion damages an item
entity as it damages each entity. `entity-physics.md` (PHY-EXP-002) states the group of explosions
of a tick, and a later specification states the amount. An explosion that does not affect
block-like entities, for example the explosion of a `wind_charge`, has no effect on an item entity.

**IEN-DMG-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). An item entity takes no damage
in each of these cases:

- The `damage_resistant` component of the stack names a tag that contains the damage type.
 
- The tag of the `damage_resistant` component contains `in_fire`, and the damage type is in the
  tag `minecraft:is_fire`. Thus `in_fire` alone in the tag also protects from lava. Such an item
  entity does not burn: its fire ticks become 0 when they are above 0.
- The entity is invulnerable, and the damage type does not bypass invulnerability.
- The game rule `mob_griefing` is false, and the source entity of the damage is a mob.

A player cannot attack an item entity.

**IEN-DMG-005** (`source_suggested`, semantic, confidence medium). When an item entity is destroyed, the items inside
its stack drop. The kind of the item selects the component:

- the item of a block: the stacks of the `container` component;
- a bundle: the stacks of the `bundle_contents` component.

The component alone does not cause a drop. For example, a data pack can give `bundle_contents` to
an item that is not a bundle. The destruction of such an item drops nothing. Each stack becomes one
item entity with the default form at the position of the destroyed item entity, and the pickup
delay 0.

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
changes the stack of the target. A partial pickup changes the stack (IEN-PCK-005).

**IEN-NET-004** (`source_suggested_black_box_confirmed`, wire, confidence high). Each removal of an item entity sends
`remove_entities` to each viewer (TRK-REM-001): the end of the lifetime, the destruction, the merge
into a target, the removal below the world and the pickup. Only a pickup sends `take_item_entity`
first.

## 10. Edition differences

Each Bedrock value comes from the public protocol documentation or is not known yet.

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
- **Bedrock**: not known.
- **Decision**: the Java values apply to both editions until the Bedrock value is known.
- **Selected by**: world rule.
- **Visible to the client**: yes.
- **Prediction risk**: low. The client of each edition shows the pickup and the merge that the
  server sends.
- **Parity list or bug reference**: none.

**ED3.**

- **Rule**: IEN-PCK-002, IEN-MRG-004.
- **Java**: the owner limits the pickup and the merge. The thrower limits neither.
- **Bedrock**: not known.
- **Decision**: the Java rule applies to both editions until the Bedrock value is known.
- **Selected by**: world rule.
- **Visible to the client**: yes.
- **Prediction risk**: none.
- **Parity list or bug reference**: none.

**ED4.**

- **Rule**: IEN-AGE-003.
- **Java**: each item entity with the age 0 is removed after 6000 ticks, also a `nether_star`.
- **Bedrock**: a `nether_star` stays.
- **Decision**: the Java lifetime applies. The tag `mcrs:item_never_despawns` is empty by default.
  A Bedrock flavour data pack fills it.
- **Selected by**: data pack.
- **Visible to the client**: yes.
- **Prediction risk**: none.
- **Parity list or bug reference**: MC-125027, closed as "works as intended" for Java.

## 11. Intentional deviations and unresolved items

### 11.1 Intentional deviations

**D1.**

- **Rule**: IEN-PCK-005.
- **Reference**: a partial pickup moves the items, but the reference then treats the insertion as
  a failure, because the last pass of the insertion found no room. Thus no viewer gets
  `take_item_entity`, and the collector gets no statistic. The stack of the item entity changes
  without a sync, so no viewer gets `set_entity_data`. Each viewer shows the old count until
  another change of the stack. The client handles a `take_item_entity` with a count smaller than
  the stack, so the protocol supports a partial pickup.
- **Visible to the client**: yes. The item entity shows the old count, and the pickup has no sound
  and no animation.

**D2.**

- **Rule**: IEN-MRG-005.
- **Reference**: a merge moves items only up to the count 64. For a stack with a maximum stack size
  above 64, 50 and 40 items give a target of 64 and a partner of 26 items that both stay. A target
  with more than 64 items gives items back: 70 and 10 give a target of 64 and a partner of 16
 . The viewers get `set_entity_data` for the target, but not for the partner. Each
  viewer shows the old count of the partner until another change of its stack.
- **Visible to the client**: yes, only for a stack whose maximum stack size is above 64.

**D3.**

- **Rule**: IEN-MOV-006, IEN-MOV-007.
- **Reference**: a tick has two fluid updates, before and after the move. Each applies the
  current, so a tick applies it two times, at two positions.
- **Visible to the client**: yes, a small difference of the path in a current. The "needs sync"
  mark of IEN-MOV-003 corrects the client in each tick in a fluid.

**D4.**

- **Rule**: IEN-MOV-007.
- **Reference**: in a shallow fluid, the factor `h` of a block is the fluid height over the blocks
  visited before it, in the order of `x`, `y`, `z`. Thus the push depends on the order of the
  search, not only on the fluid.
- **Visible to the client**: yes, only at the edge of a shallow fluid.

**D5.**

- **Rule**: IEN-MOV-007.
- **Reference**: a source block and a flowing block of one fluid are two fluids. Each gives its own
  current of the full length, so an item entity on the boundary gets up to two times the current.
- **Visible to the client**: yes, on the boundary.

**D6.**

- **Rule**: IEN-SPN-004.
- **Reference**: for a player in creative mode with no room, the `give` command destroys the items,
  and the viewers see the display item as for a success.
- **Reason**: uninserted items remain available instead of being destroyed.
- **Visible to the client**: yes. In the reference, the target receives no item. Command feedback
  to the source follows `CMD-GIVE-012` in [command-give.md](command-give.md).
- **Bug reference**: none.

### 11.2 Known vanilla quirks

None. The motion of an item entity on land and in air keeps the quirks of `entity-physics.md`.

### 11.3 Unresolved

- **Q3**: IEN-DAT-001. The model of `docs/item-design.md` keeps one entity for a stack. Does the
  item entity get a new network id when a stack goes from a slot to the world, as in the reference,
  or does the stack keep one id? The id that the client sees must be new for each `add_entity`.
- **Q4**: IEN-MRG-006. The reference merges one item entity with each partner in one search. With
  one merge in a tick, a pile of `n` item entities at rest needs about `log2(n)` searches, 40 ticks
  apart. A pile of 8 item entities stays visible for up to 120 ticks. Is this delay accepted, or
  does the arbitration of PHY-CMP-017 run more than one round in a tick?
- **Q5**: D3, D4 and D5. Players build water streams that move items. D5 changes the speed at the
  edge of a source block in a stream. The speed in a current is not measured. Does the
  owner accept D3 to D5?
- **Q6**: D6 and IEN-PCK-006. Both have one cause: an insertion for a player in creative mode
  destroys the items that find no room. D6 corrects the `give` command, but IEN-PCK-006 keeps the
  pickup. Does the owner want one rule for both?
- **Q7**: IEN-AGE-003. No current need exists for the tag `mcrs:item_never_despawns`. Does the rule
  stay, or does it wait until the Bedrock lifetime is needed?
- **U3**: IEN-SPN-001. The rows that no experiment decided: the throw click, the click outside the menu, the
  creative drop, the menu close, the leave of a player and the reward of an advancement.

## 12. Experiments

Experiments E1 to E16 ran on the vanilla server in a flat world with scripted clients. They decide the start values of each cause, the motion in fluids, the pickup, the merge, the damage times and the removal.
