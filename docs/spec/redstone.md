# Redstone: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the redstone components: how a signal moves between blocks, and what each
component does with a signal. It covers the power model, redstone wire, the power sources (lever,
buttons, pressure plates, redstone torch, block of redstone), the redstone lamp, the repeater, the
comparator, the observer and the pistons.

`block-simulation.md` states the sets, the neighbor updates, the shape updates, the scheduled
ticks and the block events. `block-support.md` states when a block stays on its support. This
document refers to the rules of these documents by id. The bounds of the handlers of this
document are in `block-simulation.md`, Tables 2 to 7.

## 0. Conventions

### 0.1 Terms

- **Set**, **neighbor update**, **neighbor request**, **shape update**, **scheduled tick**,
  **block event**, **placement side effect**, **removal side effect**, **chain**: as
  `block-simulation.md` defines them.
- **Flags**: the flags of a set (SIM-SET-006). This document writes them as numbers.
- **Signal strength**: an integer in 0..15. The value 0 is no signal.
- **Component**: a block state that has a row in Table 1.
- **Conductor**: a block state whose block definition sets `redstone_conductor` to true in the
  component `minecraft:redstone_conductivity`.
- **Output signal**: the signal strength that a block gives to one neighbor (RS-PWR-002).
- **Strong output**: the signal strength that a component gives into one neighbor so that the
  neighbor, if it is a conductor, becomes strongly powered (RS-PWR-003).
- **Signal of a block toward a neighbor**: the value of RS-PWR-004.
- **Best neighbor signal** of a position: the value of RS-PWR-005.
- **Powered position**: a position whose best neighbor signal is more than 0.
- **Wire**: a block state of `minecraft:redstone_wire`.
- **Wire power**: the value of the property `power` of a wire.
- **Connection**: the value of one of the properties `north`, `east`, `south` and `west` of a
  wire. A connection is `up`, `side` or `none`. A connection that is not `none` is **connected**.
- **Dot**: a wire whose four connections are `none`. **Cross**: a wire whose four connections are
  `side`.
- **Diode**: a block state of `minecraft:repeater` or of `minecraft:comparator`.
- **Input side** of a diode: the direction of its property `facing`. The **input block** is the
  neighbor in that direction. The **output side** is the opposite direction. The **output block**
  is the neighbor on the output side. The **lateral sides** are the two horizontal directions
  that are not the input side or the output side.
- **Attached block** of a lever, a button, a torch or a tripwire hook: the block that holds the
  component (RS-SRC-001).
- **Mechanism**: a block that reacts to the powered-position test and gives no signal, except
  as section 10 states (doors, trapdoors, fence gates, rails, note blocks, hoppers, dispensers,
  droppers, crafters, copper bulbs).
- **Sensor**: a block whose state the server reads from the entities or the sky: a pressure
  plate, a tripwire, a daylight detector. The host indexes the sensors of each loaded section.
- **Push set**: the blocks that one piston move carries (RS-PST-004).
- **Moving block**: a block state of `minecraft:moving_piston`, with its **payload**: the carried
  block state, the direction of the move and the kind of the carried block (RS-PST-001).
- **World rule**: a value of RS-RUL-001 that the world holds and that a server operator can
  change.

### 0.2 Provenance

- `black_box_confirmed`: shown by a game-produced artefact that is cited by path.
- `documentation`: taken from public documentation of player-facing behaviour, or from a public
  bug report.
- `design`: a choice of this project or of the owner. The rule states the reason.
- `unresolved`: not decided. Section 10 collects these items.

Each rule states its kind, as `docs/spec/README.md` defines it.

### 0.3 Artefacts cited

- The block definitions under `assets/mcrs/block_definition/`: the properties of each component,
  and the components `minecraft:redstone_conductivity`, `minecraft:movable` and
  `minecraft:redstone_producer`.
- The data pack of the game: the block tags `minecraft:buttons`, `minecraft:wooden_buttons`,
  `minecraft:stone_buttons`, `minecraft:pressure_plates`, `minecraft:wooden_pressure_plates`
  and `minecraft:stone_pressure_plates`.
- `data/minecraft/datapacks/redstone_experiments/pack.mcmeta` in the client jar: the feature pack
  of the experimental wire evaluator (SIM-NBR-007).

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `RS-PWR` | the power model: output signals, strong outputs, conductors, the best neighbor signal |
| `RS-WIR` | redstone wire: connections, power, propagation, placement and removal |
| `RS-SRC` | the power sources: lever, buttons, redstone torch, block of redstone |
| `RS-PLT` | the pressure plates |
| `RS-LMP` | the redstone lamp |
| `RS-REP` | the repeater |
| `RS-CMP` | the comparator |
| `RS-OBS` | the observer |
| `RS-PST` | the pistons, the piston head and the moving block |
| `RS-RUL` | the world rules of this domain |

### 0.5 Directions

The directions are `D` (down), `U` (up), `N`, `S`, `W` and `E`. A position P plus a direction d
is the neighbor P + d. The horizontal order is `N, E, S, W` (SIM-NBR-001). The **left** of a
horizontal direction d is d turned counterclockwise when seen from above. The **right** is d
turned clockwise.

---

## 1. The power model

### RS-PWR-001: Signal strength

- **Behavior:** a signal strength is an integer in 0..15. A component that stores a signal
  strength stores it in its property `power` (wire, weighted pressure plates, target, daylight
  detector) or in its payload (comparator). A value outside 0..15 does not occur.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-PWR-002: Output signal of a component toward a neighbor

- **Inputs:** a block state S at P, and a direction d toward the neighbor Q = P + d.
- **Behavior:** the output signal of S toward Q is the value of Table 1. A block state that is
  not in Table 1 has the output signal 0 toward each neighbor. A `minecraft:redstone_producer`
  component in the block definition with a fixed `power` gives that power toward each neighbor;
  the block of redstone is the vanilla case.

**Table 1. Output signals and strong outputs.** "Powered" refers to the property `powered`,
"lit" to `lit`. `A` is the direction of the attached block (RS-SRC-001). `F` is the property
`facing`.

| Component | Output signal toward Q | Strong output into Q |
|-----------|------------------------|----------------------|
| lever, powered | 15 toward each neighbor | 15 if d = A, else 0 |
| button, powered | 15 toward each neighbor | 15 if d = A, else 0 |
| pressure plate, with signal strength p (RS-PLT-002) | p toward each neighbor | p if d = D, else 0 |
| redstone torch, lit | 15 toward each neighbor, except 0 if d = A | 15 if d = U, else 0 |
| block of redstone | 15 toward each neighbor | 0 |
| wire with wire power p (RS-WIR-005) | p if d = D; 0 if d = U; p if d is horizontal and the wire connects toward d; 0 otherwise | the same as the output signal, except 0 in a wire evaluation (RS-PWR-004) |
| repeater, powered | 15 if d is the output side, else 0 | the same as the output signal |
| comparator, powered, with payload p | p if d is the output side, else 0 | the same as the output signal |
| observer, powered | 15 if d is the opposite of F, else 0 | the same as the output signal |
| tripwire hook, powered (RS-MEC-007) | 15 toward each neighbor | 15 if d = A, else 0 |
| daylight detector with power p (RS-MEC-008) | p toward each neighbor | 0 |
| a block with `minecraft:redstone_producer` and power p | p toward each neighbor | 0 |

- **Edge cases:** a dot has no horizontal connection (RS-WIR-002) and gives its power only to
  the block below it. A wire does not give a signal to the block above it.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-PWR-003: Strong output

- **Behavior:** the strong output of a component into a neighbor Q is the value of the third
  column of Table 1. A block state that is not in Table 1 has the strong output 0.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-PWR-004: Signal of a block toward a neighbor

- **Inputs:** a position P with block state S, and a direction d toward Q = P + d.
- **Behavior:** the signal of P toward Q is the maximum of these two values:
  1. the output signal of S toward Q (RS-PWR-002);
  2. if S is a conductor: the maximum over the six neighbors R of P of the strong output of R
     into P (RS-PWR-003). If S is not a conductor, this value is 0.
- **Edge cases:** a **wire evaluation** is the read that a wire makes to calculate its own
  power (RS-WIR-005). In a wire evaluation, each wire has the output signal 0 and the strong
  output 0. As a result, a conductor that only a wire powers gives its power to a mechanism
  (RS-PWR-005), a torch (RS-SRC-004) or a diode (RS-REP-002), but not to another wire. Players
  call this **weak power**. A conductor that a component powers through its strong output gives
  its power to wire too; players call this **strong power**.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-PWR-005: Best neighbor signal and powered positions

- **Inputs:** a position P.
- **Behavior:** the best neighbor signal of P is the maximum over the six directions d of the
  signal of P + d toward P (RS-PWR-004). The position P is **powered** if this value is more than
  0. A mechanism that reacts to power (RS-LMP-001, RS-PST-002, doors, trapdoors, fence gates,
  note blocks, dispensers) reads this test.
- **Edge cases:** the test is not a wire evaluation. A conductor that only a wire powers makes
  its neighbors powered positions. A lamp next to a block that a wire runs over lights.
- **Conformance tests:**
  - Put a lever on the top of a stone block. Put a lamp on the east face of the stone block and a
    wire on the ground west of the stone block, at the same height as the stone block. Pull the
    lever. Result: the lamp lights and the wire shows power 15. Replace the stone block with
    glass. Result: the lamp stays dark and the wire stays at 0.
  - Make a line of two wires on the ground that points into a stone block, and power the line
    from a block of redstone. Put a lamp on a side of the stone block, and a second wire on the
    ground on the far side of the stone block. Result: the lamp lights and the second wire stays
    at 0.

Provenance: `documentation`. Kind: semantic. Confidence: high.

---

## 2. Redstone wire

### RS-WIR-001: State

- **Behavior:** a wire has the properties `power` in 0..15 and the connections `north`, `east`,
  `south`, `west`, each `up`, `side` or `none`. The default state is a dot with power 0.

Provenance: `black_box_confirmed` (`assets/mcrs/block_definition/redstone_wire.json`). Kind:
semantic. Confidence: high.

### RS-WIR-002: Connections

- **Inputs:** a wire at P and the blocks around it.
- **Behavior:** the connection of the wire toward a horizontal direction d is calculated in this
  order. Q = P + d.
  1. If the block above P is not a conductor, Q **can hold a wire** (RS-WIR-008) and Q + U holds
     a wire: the connection is `up` when the face of Q toward P is a full face, else `side`.
  2. Otherwise, if Q holds a wire, or Q holds a block that **accepts a wire connection** from the
     direction opposite to d, the connection is `side`.
  3. Otherwise, if Q is not a conductor and Q + D holds a wire, the connection is `side`.
  4. Otherwise the connection is `none`.

  A block accepts a wire connection from a direction e when one of these is true:
  - it is a **signal source** that is not a diode, an observer or a piston. The signal sources
    are the blocks whose definition has the component `minecraft:redstone_producer`, and the
    components of Table 1 that are not conductors: the block of redstone, the lever, the
    buttons, the pressure plates, the redstone torch, the target, the daylight detector, the
    trapped chest, the lectern, the tripwire hook, the daylight detector, the detector rail and
    the lightning rod;
  - it is a repeater, and e is on the axis of its `facing`;
  - it is a comparator, from each direction (a wire on a lateral side is a lateral input,
    RS-REP-002);
  - it is an observer, and e is the opposite of its `facing` (the output side);
  - a piston does not accept a connection, unless the world rule
    `mcrs:redstone_dust_connects_to_pistons` is on.

  After the four connections are calculated, the wire is **straightened**. Both tests of the
  straightening read the four connections as they were calculated, before the straightening
  changes any of them: if neither `north` nor `south` is connected, each of `west` and `east`
  that is not connected becomes `side`; if neither `west` nor `east` is connected, each of
  `north` and `south` that is not connected becomes `side`. As a result, a wire with no
  connected neighbor is a cross, and a wire with connections on one axis only is a line on that
  axis, unless it is a kept dot (RS-WIR-004).
- **Edge cases:** a wire with a single neighbor wire is a line, and it gives its power to both
  ends of the line (Table 1). A dot is a wire with four `none` connections; it gives its power
  only to the block below it.
- **Conformance tests:** put a wire at P and a wire at P + E. Result: both wires are lines on the
  east-west axis, with `east` and `west` `side`. Put a wire at P + N + U with a non-conductor
  (glass) at P + N. Result: the `north` connection of P is `up`. Replace the glass with stone.
  Result: `north` stays `up`: the stone has a full face toward P, and no conductor is above P.
  Put stone above P. Result: `north` is `none`, unless a wire is at P + N.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-WIR-003: A wire updates its connections on a shape update

- **Inputs:** a shape update at a wire (SIM-SHP-001), from the direction f.
- **Behavior:** if f is `D` and the wire cannot stay (RS-WIR-008), the result of the shape
  function is air. Otherwise the result is the wire with the connections of RS-WIR-002 and the
  same power. A wire that receives the shape update from `U` recalculates all four connections,
  because the block above it decides which connections can be `up`. A shape update from a
  horizontal direction recalculates the connection toward that direction. When the connected
  state of that side changes, or the wire is a cross, all four connections are recalculated,
  and the result is not a kept dot.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-WIR-004: Dot and cross

- **Inputs:** a player uses a wire (a block use, `block-placing.md`).
- **Behavior:** if the wire is a dot, it becomes a cross. If the wire is a cross, it becomes a
  dot. In both cases the set has the flags 3. A wire with another shape does not react to a use.
  A dot that gains a connected neighbor becomes connected again at the next shape update
  (RS-WIR-003); a dot keeps its shape only while no neighbor connects to it.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-WIR-005: Wire power

- **Inputs:** a wire at P.
- **Behavior:** the target power of the wire is the maximum of these two values:
  1. the best neighbor signal of P (RS-PWR-005), where each neighbor that is a wire gives 0;
  2. the maximum wire power of the **neighbor wires** of P, minus 1, and not less than 0.

  The neighbor wires of P are:
  - a wire at P + d, for each horizontal d;
  - a wire at P + d + D, for each horizontal d, if P + d is not a conductor;
  - a wire at P + d + U, for each horizontal d, if P + d is a conductor and P + U is not a
    conductor.

- **Edge cases:** a wire one block lower beside a glass block gives its power up to the wire on
  the glass: the upper wire reads the wire below its neighbor, because that neighbor (air) is
  not a conductor. The wire on the glass does not give its power down: the lower wire reads a
  wire above its neighbor only when that neighbor is a conductor, and glass is not. Power
  climbs the step and does not come down it. This is the one-way step of glass, top slabs and
  upside-down stairs.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-WIR-006: Propagation

- **Inputs:** a neighbor update at a wire, a placement of a wire (RS-WIR-007) or a set that
  changes the blocks around a wire.
- **Behavior:** when a wire receives a neighbor update, it calculates its target power
  (RS-WIR-005). If the target power is the same as its wire power, nothing occurs. If not, the
  wire sets its state to the target power with the flags 2, and then makes a neighbor request
  (SIM-NBR-001) at P and at each of the six neighbors of P. Each wire that these requests reach
  does the same. As a result, the whole network has its target power at the end of the chain.
- **Ordering constraints:** the order of the updates inside the chain is the wire evaluation
  policy (P-WIRE of `block-simulation.md`). The default evaluator, which the `java` preset uses,
  runs the chain in the vanilla order of SIM-NBR-003 (depth first, west, east, down, up, north,
  south), so a circuit whose result depends on the update order of wire, such as two sticky
  pistons that pull one block, gives the result of the reference. The experimental evaluator
  (RS-WIR-009) keeps only the final powers. The observable result is the final power of each wire
  and of each component that the network feeds. A component that reacts at once (a lamp
  that turns on, RS-LMP-001) can react more than one time inside one chain, when the wire
  evaluation visits it with two values. A component that schedules a tick reacts one time
  (SIM-SCH-004).
- **Edge cases:** a wire in a section that the chain cannot write (`block-simulation.md`,
  SIM-MODEL-102) continues at the next sync point.
- **Conformance tests:** put a lever on a block, and a line of 15 wires on the ground from the
  block. Put a lamp against the end of the line. Pull the lever. Result: the wires show the
  powers 15 down to 1 and the lamp lights. Add a sixteenth wire and move the lamp after it.
  Result: the sixteenth wire shows 0 and the lamp stays dark.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-WIR-007: Placement and removal

- **Behavior:**
  - Placement side effect of a wire at P: the wire calculates its target power (RS-WIR-005) and
    its connections, and sets itself if they differ. Then it makes a neighbor request at P + U
    and at P + D, and it updates the **corner wires**: first the wire at P + d for each
    horizontal d, then, for each horizontal d, the wire at P + d + U if P + d is a conductor,
    else the wire at P + d + D. An update of a corner wire is a neighbor request at the wire
    and at each of its six neighbors. A position that holds no wire gets no request.
  - Removal side effect of a wire at P, when the removal is not a piston move: a neighbor
    request at each of the six neighbors of P, and the corner wires as in placement.
  - A player places a wire with the item `minecraft:redstone` (PLC-INV-007).
- **Conformance tests:** place a wire next to a lit torch. Result: the wire shows 15 before the
  placement returns. Break the wire. Result: a lamp that this wire fed goes dark after
  RS-LMP-001.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-WIR-008: Support

- **Behavior:** a block **can hold a wire** when the top face of its support shape covers the
  center of the face (the test `center` of `block-support.md`), or when it is a hopper. A wire
  stays at P only if the block at P + D can hold a wire. When the block below changes so that
  this test fails, the shape update from `D` destroys the wire with drops (SIM-SHP-001).

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-WIR-009: The experimental evaluator

- **Inputs:** a neighbor update, a placement or a removal at a wire, when the world rule
  `mcrs:experimental_wire` is on (RS-RUL-001).
- **Behavior:** the wire collects its **network**: the wires that it reads and the wires that
  read it (the neighbor wire relation of RS-WIR-005), and their wires in turn, inside a box of
  ±17 blocks around the wire on each axis (`block-simulation.md`, Table 2). For each wire of the
  network, the evaluator reads the best neighbor signal with the wire evaluation of RS-PWR-004.
  Then it gives each wire the fixpoint of RS-WIR-005 over the network: the largest of its own
  neighbor signal and the power of each wire that it reads, minus 1. Each wire whose power
  changes is set with the flags 130 (*sync-clients* and *wire-skips-shape*, SIM-SET-006). After
  all sets, the evaluator makes a neighbor request at each changed wire and at each of its six
  neighbors that is not a wire.
- **Ordering constraints:** the result is the same set of final powers as RS-WIR-006 gives. The
  order of the requests inside the chain is the wire evaluation policy (P-WIRE). The experimental
  evaluator makes no request at a wire that did not change, and no shape update at a wire.
- **Edge cases:** a wire that the network reaches outside the box is a **boundary wire**: it
  enters the network with its current power, gives that power minus 1 to the wires of the
  network that read it, and the evaluator does not write it. A wire at the edge of the box
  therefore keeps the power that the whole network gives it. A network that reaches the box
  limit continues in the next evaluation that an update at its edge starts. A wire that receives
  a neighbor update and already has its target power makes no network evaluation.
- **Conformance tests:** the tests of RS-WIR-006 give the same powers with the rule on. Remove the
  source of a line of five wires. Result: all five wires show 0 after the removal returns. A
  line of 31 wires with a source at each end; remove the source at the first wire. Result: the
  wire 17 places from the first wire keeps the power 2 of the other source.

Provenance: `design`, the semantic form of the experimental wire evaluator of the feature
`minecraft:redstone_experiments`. Kind: semantic. Confidence: medium.

---

## 3. Power sources

### RS-SRC-001: Attached block

- **Behavior:** the attached block of a lever or a button is given by its properties `face` and
  `facing`. For `face` = `floor`, A = `D`. For `face` = `ceiling`, A = `U`. For `face` = `wall`,
  A is the opposite of `facing`. The attached block of a standing redstone torch is at `D`. The
  attached block of a wall torch is at the opposite of its `facing`. A lever, a button or a torch
  stays only while its attached block is a conductor (`block-support.md`); when it is not, the
  shape update from A destroys the component with drops.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-SRC-002: Lever

- **Inputs:** a player uses a lever at P. The use follows the rules of a block use: a spectator
  does not use blocks (PLC-INT-005), a player with a pending teleport does not (PLC-TEL-001), and
  a player who sneaks with an item in a hand does not (PLC-INT-002). The sneak state is the one
  received for the current tick.
- **Behavior:** the lever changes its property `powered` to the opposite value with the flags 3.
  Then it makes a neighbor request at P and at P + A. The output signal is as in Table 1.
- **Edge cases:** two uses of one lever in one tick apply in order: the second use reads the
  state that the first use leaves, so two uses of an open lever leave it open.
- **Conformance tests:** put a lever on the top of a stone block and a lamp under the block.
  Pull the lever. Result: the lamp lights in the same tick. Pull again. Result: the lamp goes
  dark after 4 ticks (RS-LMP-001).

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-SRC-003: Buttons

- **Inputs:** a player uses a button at P that is not powered.
- **Behavior:** the button sets `powered` to true with the flags 3, makes a neighbor request at
  P and at P + A, and schedules a tick at P with the **press time**. The press time is 20 ticks
  for a button of the tag `minecraft:stone_buttons`, and 30 ticks for a button of the tag
  `minecraft:wooden_buttons`. A use of a powered button changes nothing, but it is a use of the
  block: the held item is not placed (PLC-INT-001). When the tick runs and the button is powered,
  it sets `powered` to false with the flags 3 and makes the same two neighbor requests.
- **Projectiles:** a wooden button whose block holds a projectile (an arrow or a trident) is
  pressed as if a player used it. When its tick runs and a projectile is still inside, the
  button stays pressed and schedules its tick again with the press time. A button of
  `minecraft:stone_buttons` ignores projectiles. The server tells the button about a projectile
  through a neighbor update; the button reads the count of projectiles inside its block.
- **Conformance tests:** press a stone button that feeds a lamp. Result: the lamp lights at
  once and goes dark 20 + 4 ticks after the press. Do the same with an oak button. Result: the
  lamp goes dark 30 + 4 ticks after the press.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-SRC-004: Redstone torch

- **Inputs:** a redstone torch at P with the attached block at P + A.
- **Behavior:**
  - The torch **must be off** when the signal of P + A toward P (RS-PWR-004) is more than 0.
  - Neighbor update: if `lit` is equal to "must be off" and no tick is scheduled at P for the
    torch, the torch schedules a tick at P with delay 2. A dark torch whose toggle record shows a
    burnout (RS-SRC-005) schedules no tick here; its tick of delay 160 lights it.
  - Tick: let `off` = "must be off". If the torch is lit and `off`: the torch sets `lit` to false
    with the flags 3 and records a toggle (RS-SRC-005). If the toggle record shows a burnout, the
    torch schedules a tick at P with delay 160 and plays the level event `1502` at P. If the
    torch is not lit, not `off`, and the toggle record shows no burnout: the torch sets `lit` to
    true with the flags 3.
  - Placement side effect: a neighbor request at each of the six neighbors of P. Removal side
    effect, when not a piston move: a neighbor request at each of the six neighbors of P.
  - The output signal and the strong output are as in Table 1.
- **Conformance tests:** put a torch on the side of a stone block and a lever on the top of the
  block. Pull the lever. Result: the torch goes dark 2 ticks later. Pull the lever again.
  Result: the torch lights 2 ticks later.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-SRC-005: Torch burnout

- **Inputs:** the toggle record of the dimension: a list of (position, game time) pairs.
- **Behavior:** before each test of the record, the pairs older than 60 ticks are removed. A
  toggle of a torch at P adds the pair (P, now). The record shows a **burnout** at P when the
  record holds 8 or more pairs at P. A burnt-out torch stays dark until its tick of delay 160
  runs; at that time the pairs have aged out, and the torch lights if it must not be off.
- **Edge cases:** the record belongs to the dimension. A partition of a dimension that runs
  alone keeps its own record (`block-simulation.md`, B.3).
- **Conformance tests:** make a clock of one torch and one wire that toggles each 2 ticks.
  Result: the torch fizzes (level event `1502`) and stays dark for about 160 ticks, then runs
  again.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-SRC-006: Block of redstone

- **Behavior:** a block of redstone gives the output signal 15 toward each neighbor, and no
  strong output. It is not a conductor. A piston can move it.

Provenance: `documentation`. Kind: semantic. Confidence: high.

---

## 4. Pressure plates

### RS-PLT-001: Kinds

- **Behavior:** a plate of the tag `minecraft:wooden_pressure_plates` reacts to each entity. A
  plate of the tag `minecraft:stone_pressure_plates` reacts to living entities only. These two
  kinds have the property `powered` and give the signal 15 when powered. The weighted plates
  `minecraft:light_weighted_pressure_plate` and `minecraft:heavy_weighted_pressure_plate` have
  the property `power` and react to each entity.
- **Edge cases:** players and mobs are living entities; a spectator is not an entity to a
  plate. Items are entities. The server reports the entities on each plate every tick. The
  server knows the plates of a section from the moment the section loads, so a saved plate
  senses entities as a placed plate does. A player that another cell owns is not reported to
  the plates of this cell (section 13).

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-PLT-002: Signal strength of a plate

- **Inputs:** the count n of the entities of the kind of RS-PLT-001 whose bounding box
  intersects the **sensing box** of the plate: the box from (1/16, 0, 1/16) to (15/16, 1/4, 15/16)
  of the block of the plate.
- **Behavior:** a wooden or stone plate has the signal 15 if n > 0, else 0. A light weighted plate
  has the signal min(n, 15). A heavy weighted plate has the signal ceil(n × 15 / 150), not more
  than 15.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-PLT-003: Press and release

- **Behavior:** when an entity enters the sensing box of a plate whose signal is 0, the plate
  sets its signal to RS-PLT-002 with the flags 2, makes a neighbor request at P and at P + D, and
  schedules a tick with the **check time**: 20 ticks for a wooden or stone plate, 10 ticks for a
  weighted plate. When the tick runs and the signal is more than 0, the plate calculates
  RS-PLT-002 again. If the value changed, the plate sets it with the flags 2 and makes the same
  two neighbor requests. If the value is more than 0, the plate schedules the tick again.
- **Conformance tests:** drop one item on an oak plate. Result: the plate is powered. Drop one
  item on a stone plate. Result: the plate stays unpowered. Remove the item from the oak plate.
  Result: the plate is unpowered not more than 20 ticks later.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

---

## 5. Redstone lamp

### RS-LMP-001: Lamp

- **Inputs:** a lamp at P.
- **Behavior:**
  - Placement: the lamp is placed lit if P is powered (RS-PWR-005), else unlit.
  - Neighbor update: if the lamp is lit and P is not powered, and no tick is scheduled at P for
    the lamp, the lamp schedules a tick with delay 4. If the lamp is unlit and P is powered, the
    lamp sets `lit` to true with the flags 2.
  - Tick: if the lamp is lit and P is not powered, the lamp sets `lit` to false with the
    flags 2.
- **Conformance tests:** as RS-SRC-002.

Provenance: `documentation`. Kind: semantic. Confidence: high.

---

## 6. Repeater

### RS-REP-001: State and delay

- **Behavior:** a repeater has `facing` (the input side, section 0.1), `delay` in 1..4,
  `locked` and `powered`. The tick delay of a repeater is `delay` × 2 ticks. A player use of a
  repeater adds 1 to `delay`, and after 4 it becomes 1, with the flags 3.
- **Edge cases:** a repeater stays only while the block at P + D is a conductor
  (`block-support.md`). When it is not, the shape update from `D` destroys it with drops.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-REP-002: Input and lock

- **Inputs:** a diode at P with the input block at P + F.
- **Behavior:** the **input signal** of a diode is the signal of P + F toward P (RS-PWR-004),
  except that a wire at P + F gives its wire power, and a block of redstone gives 15. The
  **lateral signal** of a diode is the maximum over the two lateral sides s of the **lateral
  input** at P + s: for a repeater, the strong output of a diode at P + s into P; for a
  comparator, also the wire power of a wire at P + s and 15 for a block of redstone at P + s.
  Other blocks give 0 as a lateral input.
- **Behavior (lock):** a repeater is **locked** when its lateral signal is more than 0. The
  repeater reads the lateral signal each time it reacts (RS-REP-003), not the property. The
  property `locked` follows: a shape update from a horizontal direction (SIM-SHP-001) sets it
  to the current value, so the client draws the lock. A locked repeater keeps its `powered`
  value and schedules no tick.
- **Conformance tests:** put a powered repeater whose output side faces the lateral side of a
  second repeater. Result: the second repeater shows `locked` true, and a change of its input
  does not change its output.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-REP-003: Reaction to a neighbor update

- **Behavior:** when a repeater that is not locked receives a neighbor update: let `on` = the
  input signal is more than 0. If `powered` is not equal to `on`, and the repeater will not tick
  this tick (SIM-SCH-010), it schedules a tick at P with its tick delay and with the priority:
  - extremely high (−3), if the output block holds a diode whose `facing` is not the output
    side of this repeater (the output feeds that diode);
  - very high (−2), if `powered` is true (the repeater turns off);
  - high (−1) otherwise.
- **Tick:** if the repeater is locked, nothing occurs. Otherwise, let `on` as above. If `powered`
  and not `on`: set `powered` to false with the flags 2. If not `powered`: set `powered` to true
  with the flags 2, and if not `on`, schedule a tick at P with the tick delay and the priority
  very high (the pulse is extended to the delay).
- **After each change of `powered`:** the repeater updates its output block: a neighbor update
  at P − F with the repeater as source, and then a neighbor request at P − F that skips the
  direction toward P (SIM-NBR-001, skip = F).
- **Placement side effect:** if the diode would turn on (a repeater: the input signal is more
  than 0; a comparator: the output of RS-CMP-003 is more than 0), schedule a tick at P with
  delay 1. The output block is updated as above. **Removal side effect**, when not a piston
  move: the output block is updated as above.
- **Conformance tests:**
  - A lever feeds a repeater with `delay` 4, and the repeater feeds a lamp. Pull the lever.
    Result: the lamp lights 8 ticks after the lever.
  - Two repeaters with `delay` 1 that one lever feeds. One repeater feeds a second repeater,
    the other feeds a lamp. Pull the lever. Result: both ticks are due at the same game time,
    and the repeater that feeds the other repeater runs first (priority −3 before −1).
  - A lever gives a 1-tick pulse into a repeater with `delay` 2. Result: the output stays on for
    4 ticks.

Provenance: `documentation`. Kind: semantic. Confidence: high.

---

## 7. Comparator

### RS-CMP-001: State and payload

- **Behavior:** a comparator has `facing` (the input side), `mode` (`compare` or `subtract`) and
  `powered`. Its payload is its **output signal**, a signal strength that the save stores as
  `OutputSignal` of the block entity of the comparator. The server writes the block entity when
  the output changes, once for each comparator whose output changed since the last write, and
  reads it when the column loads into a comparator that has no payload yet; a payload that the
  simulation computed in the meantime stays. A player use of a comparator changes `mode` to the
  other value with the flags 2, and then the comparator refreshes (RS-CMP-004). A comparator stays only while the block at
  P + D is a conductor.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-CMP-002: Input signal with analog sources

- **Behavior:** the input signal of a comparator is the input signal of RS-REP-002, with these
  changes. Let Q = P + F.
  1. If the block at Q has an **analog output** (a container, or a block that
     `block-simulation.md` lists with an analog output signal), the input signal is that analog
     output.
  2. Otherwise, if the input signal is less than 15 and Q is a conductor, the comparator reads
     the position Q + F: the analog output of the block there if it has one, and the analog
     output of an **item frame** at Q + F that hangs on Q (it faces F). When one or both exist,
     the input signal is the larger of the two. The analog output of an item frame is 0 for an
     empty frame, else its rotation plus 1 (1..8).

  The analog output of a container is the fill signal of its slots: 0 for an empty container,
  else `floor(14 × fill) + 1` where `fill` is the sum over the slots of (count ÷ maximum stack
  size), divided by the slot count, not more than 1.
- **Edge cases:** the server tells the comparator about an item frame through the same record
  as a container, and the comparator reads the frame only through a conductor.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-CMP-003: Output signal

- **Behavior:** let `in` = the input signal (RS-CMP-002) and `side` = the lateral signal
  (RS-REP-002). If `in` is 0, the output is 0. Otherwise, if `side` is more than `in`, the output
  is 0. Otherwise the output is `in` in `compare` mode, and `in` − `side` in `subtract` mode.
- **Conformance tests:** a block of redstone behind a comparator and a wire with power 10 on a
  lateral side. Result: compare mode outputs 15, subtract mode outputs 5. A wire with power 15 on
  the side and 10 behind. Result: 0 in both modes.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-CMP-004: Reaction and refresh

- **Behavior:**
  - Neighbor update, when the comparator will not tick this tick: let `out` = RS-CMP-003. If
    `out` is not equal to the payload, or `powered` is not equal to (`out` > 0), the comparator
    schedules a tick at P with delay 2 and the priority high (−1) if the output block holds a
    diode whose `facing` is not the output side, else normal (0).
  - Tick (refresh): let `out` = RS-CMP-003 and `old` = the payload. The payload becomes `out`.
    If `old` is not equal to `out`, or `mode` is `compare`: if `powered` and `out` = 0, set
    `powered` to false with the flags 2; if not `powered` and `out` > 0, set `powered` to true
    with the flags 2. Then the output block is updated as RS-REP-003 states.
  - Placement and removal side effects: as RS-REP-003.
- **Conformance tests:** a hopper with one item behind a comparator and a wire in front. Result:
  the wire shows 1. A hopper full of 64-stacks. Result: the wire shows 15.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

---

## 8. Observer

### RS-OBS-001: Observer

- **Inputs:** an observer at P with `facing` F toward the **watched block** at P + F and the
  output block at P − F.
- **Behavior:**
  - Shape update from F (the watched block changed its shape or state): if `powered` is false
    and no tick is scheduled at P for the observer, schedule a tick at P with delay 2. The shape
    function returns the state unchanged.
  - Tick: if `powered` is true, set `powered` to false with the flags 2. Otherwise set
    `powered` to true with the flags 2 and schedule a tick at P with delay 2. After each change,
    update the output block as RS-REP-003 states (the neighbor update at P − F, then the request
    that skips the direction toward P).
  - Placement side effect: if `powered` is true and no tick is scheduled, set `powered` to false
    with the flags 18. Removal side effect, when not a piston move: if `powered` and a tick is
    scheduled, update the output block.
  - The output signal and the strong output are as in Table 1 (15 toward P − F while powered).
- **Edge cases:** a shape update from a direction other than F has no effect. A neighbor update
  has no effect on an observer. An observer that a piston moves receives shape updates at its
  new position, and so it pulses after the move.
- **Conformance tests:** place a block in front of an observer that feeds a lamp. Result: the
  lamp lights 2 ticks after the placement and goes dark 2 + 4 ticks after that.

Provenance: `documentation`. Kind: semantic. Confidence: high.

---

## 9. Pistons

### RS-PST-001: States

- **Behavior:** `minecraft:piston` and `minecraft:sticky_piston` have `facing` (the direction of
  the head) and `extended`. `minecraft:piston_head` has `facing`, `type` (`normal` or `sticky`)
  and `short`. `minecraft:moving_piston` has `facing` and `type`, and a payload: the carried
  block state, `facing` and the kind of the carried block: a block of the push set, the piston
  head of an extension, or the piston base of a retraction. A piston that is
  `extended` is immovable (RS-PST-004).

Provenance: `black_box_confirmed` (`assets/mcrs/block_definition/piston.json`,
`piston_head.json`, `moving_piston.json`). Kind: semantic. Confidence: high.

### RS-PST-002: Power of a piston

- **Inputs:** a piston at P with `facing` F.
- **Behavior:** a piston is **powered** when one of these is true:
  1. for a direction d that is not F, the signal of P + d toward P (RS-PWR-004) is more than 0;
  2. the world rule `mcrs:quasi_connectivity` is on, and for a direction d that is not `D`, the
     signal of P + U + d toward P + U is more than 0.
- **Edge cases:** the second test reads the position above the piston as if a mechanism were
  there. It does not read the piston itself, and it does not need a block at P + U. A piston
  that this test powers does not react until an update reaches it (RS-PST-003).

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-PST-003: Reaction to an update

- **Behavior:** on a placement side effect and on each neighbor update, a piston that is not a
  moving block does the **extend check**: let `powered` = RS-PST-002.
  - If `powered` and not `extended`: if the push set of an extension resolves (RS-PST-004), queue
    a block event at P with `a` = 0 and `b` = the index of F.
  - If not `powered` and `extended`: queue a block event at P with `a` = 1 and `b` = the index of
    F; except that a sticky piston whose block at P + 2F is a moving block of an extension with
    the same `facing` queues `a` = 2 when the world rule `mcrs:sticky_piston_short_pulse` is on
    (the block that it pushes is not pulled back, see RS-PST-006).

  The index of a direction is `D=0, U=1, N=2, S=3, W=4, E=5`.
- **Edge cases:** a powered piston whose push set does not resolve queues nothing and stays
  retracted. It does the extend check again at the next update.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-PST-004: Push set

- **Inputs:** a piston at P with `facing` F, and a move direction M: M = F for an extension, and
  M is the opposite of F for a retraction. The **start position** is P + F for an extension and
  P + 2F for a retraction.
- **Behavior:** a block state is **movable** in the direction M according to its component
  `minecraft:movable`:
  - `push_pull`: movable;
  - `push_only`: movable only when M = F (an extension pushes it; a retraction does not pull it);
  - `popped`: the block is **destroyed** by a push: it leaves the push set and drops its loot;
  - `immovable`: not movable;
  - a block with a block entity is not movable, unless the world rule
    `mcrs:pistons_move_block_entities` is on; with the rule on, the block entity travels with
    the block and arrives with it (RS-PST-007);
  - a piston that is `extended`, a piston head and a moving block are not movable;
  - air ends a line; a position outside the world (void air) makes the set fail, so a piston
    never pushes a block out of the world; a position that this partition cannot read, or a
    destination that it cannot write (a column of another cell, `cells.md`), makes the set fail,
    so a move never leaves its payload behind in one partition and its block in another;
  - a block with the explosion resistance of bedrock (hardness −1) is not movable.

  The push set is built from the start position in this way. A **line** from a position Q in the
  direction M adds Q and the following blocks in the direction M to the set, until it reaches
  air, a destroyed block (added to the destroy list; the line ends) or the piston (the set fails).
  An immovable block in the line makes the set fail. A **sticky block** (slime block, honey
  block) in the set adds, for each direction e not on the axis of M, a line from its neighbor in
  the direction e, when the two blocks **stick to each other**: a slime block does not stick to
  a honey block, and a block that is `push_only` or not movable does not come along. Before a
  line is added, the blocks behind Q in the direction opposite to M that stick to Q are added
  too. The set fails when it holds more than 12 blocks. The piston position never enters the
  set.
- **Conformance tests:** 12 stone blocks in front of a piston. Result: the piston extends and all
  12 move. 13 stone blocks. Result: the piston does not extend. A chest in front. Result: the
  piston does not extend. A slime block with a stone block on its side. Result: both move. A
  slime block with a honey block on its side. Result: the honey block stays.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-PST-005: Block event: extension

- **Inputs:** a block event at a piston with `a` = 0.
- **Behavior:** if the piston is not powered (RS-PST-002), nothing occurs. Otherwise, if the push
  set of RS-PST-004 fails, nothing occurs. Otherwise the piston **moves the blocks**:
  1. Each block of the destroy list is destroyed with drops (SIM-SET-004).
  2. Each block of the push set, in the order from the farthest to the nearest, is replaced at
     its position by a moving block whose payload is the old state as a carried block, with
     `facing` M, at the position Q + M, with the flags 68. The old position becomes air with
     the flags 82 (the removal side effect sees "moved by piston").
  3. The position P + F receives a moving block whose payload is the piston head state (`facing`
     F, `type` of the piston) as the head, with the flags 68.
  4. The piston sets `extended` to true with the flags 67.
  5. Each moving block schedules a tick at its position with delay 2 (RS-PST-007).
  6. A neighbor request at P and at each position of the destroy list and of the push set
     (the old and the new positions).
  7. The level event of the extension plays at P (sound).
- **Postconditions:** the event is delivered to the clients (SIM-BEV-003).

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-PST-006: Block event: retraction

- **Inputs:** a block event at a piston with `a` = 1 or 2.
- **Behavior:** if the piston is powered (RS-PST-002), the piston sets `extended` to true with
  the flags 2 and nothing else occurs (the retraction is cancelled). Otherwise:
  1. If P + F holds a moving block, that move completes at once (RS-PST-007).
  2. The position P receives a moving block whose payload is the piston state with `extended`
     false as the base, with `facing` F and the flags 20. It schedules its tick with delay 2.
  3. A sticky piston with `a` = 1 pulls: let R = P + 2F. If R holds a block that is movable in
     the direction opposite to F (RS-PST-004), the piston moves the blocks as RS-PST-005 states
     for a retraction (M = the opposite of F, the start position R). If R holds a moving block
     with `facing` F, that move completes at once; then nothing is pulled when the world rule
     `mcrs:sticky_piston_short_pulse` is on, and the completed block is pulled when it is off.
     Otherwise the head at P + F is removed with the flags 3 and no drops.
  4. A piston that is not sticky, or a sticky piston with `a` = 2, removes the head at P + F with
     the flags 3 and no drops.
  5. A neighbor request at P.
  6. The level event of the retraction plays at P.
- **Edge cases:** a sticky piston that receives a 1-tick pulse (extension and retraction in
  consecutive ticks) leaves the pushed block at its new position when the world rule
  `mcrs:sticky_piston_short_pulse` is on (the default). This is RS-PST-003 with `a` = 2.
- **Conformance tests:** a sticky piston pushes a stone block with a pulse of 1 tick. Result:
  the stone block stays one block away and the piston retracts without it.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-PST-007: The moving block

- **Behavior:** a moving block exists for 2 ticks. Its tick completes the move: the position
  receives the payload state with the flags 67 (3 with *piston-move*), and then a neighbor
  update at the position with the payload block as source. A payload of the base kind completes
  to the piston state, and a payload of the head kind completes to the piston head.
- **Record:** the server keeps the payload of each moving block in the save as the block entity
  `minecraft:piston` at the moving block: `blockState` (the payload state as `Name` and
  `Properties`), `facing`, `extending`, `source` (true for a piston head or base) and
  `mcrs:carried`, the block entity that the payload carries (RS-PST-004). When a column loads
  with a moving block, the server restores the payload from the record and makes a neighbor
  request at the moving block; a neighbor request at a moving block whose move is due and has no
  scheduled tick completes the move. The server writes the record in the tick of the move,
  after the block entities that the move replaced are removed, and links it to its section at
  once, so the save of that tick holds it.
- **Edge cases:** a moving block is not movable, not a conductor and has no output signal. A set
  that replaces a moving block before its tick discards its payload. A moving block whose tick
  finds no payload keeps its place until a record restores the payload. A tick that comes before
  the payload is 2 ticks old (a tick left by an earlier payload at the same position) schedules
  itself again for the ticks that are left. A moving block has the collision shape of its
  payload, offset by the progress (RS-PST-009). A block event whose direction is not the
  `facing` of the piston at that time is stale: the piston checks its power again and the event
  does nothing.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-PST-009: Moving blocks push entities

- **Inputs:** a moving block whose payload is a carried block or a piston head, at the position
  Q, with the move direction M and the progress p of RS-PST-001.
- **Behavior:** in each tick of the move, the moving block has the collision shape of its payload
  at Q − M × (1 − p). An entity whose bounding box intersects that shape is moved along M so that
  its box leaves the shape, plus 1/100 of a block. A payload that is a slime block gives the
  entity the velocity 1 along M instead of the move. A player is moved in the same tick, before
  the movement of the tick, with a position sync that the player confirms (PLC-TEL-001); a
  slime block does not launch a player. The piston base of a retraction pushes nothing.
- **Edge cases:** an entity in the path of a second moving block is pushed by each of them.
- **Conformance tests:** a piston pushes a stone block into an item that lies in front of it.
  Result: the item is one block farther along M after the move.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-PST-008: Piston head

- **Behavior:** a piston head at H with `facing` F stays only while H − F holds a piston with
  `facing` F and `extended` true, or a moving block with `facing` F. When it does not, the shape
  update from the opposite of F destroys the head with no drops. A neighbor update at the head
  is passed to the base as a neighbor update. A head is not movable.

Provenance: `documentation`. Kind: semantic. Confidence: high.

---

## 10. Mechanisms that react to power

The mechanisms of this section react to the powered-position test (RS-PWR-005) and have no
output signal, except the tripwire hook and the daylight detector, which are signal sources
(Table 1). A **rising edge** is a change of the test from false to true.

### RS-MEC-001: Doors, trapdoors and fence gates

- **Inputs:** a neighbor update at a block of the tag `minecraft:doors`, `minecraft:trapdoors`
  or `minecraft:fence_gates`, or a placement of such a block.
- **Behavior:** the block reads the powered-position test at P. A door reads the test at both
  halves: the lower half and the upper half. When the result is not equal to `powered`, the
  block sets `powered` and `open` to the result with the flags 2. A door also sets the other
  half the same way. A player use of a block of `minecraft:wooden_doors`,
  `minecraft:wooden_trapdoors` or `minecraft:fence_gates` changes `open` to the other value with
  the flags 2; the other half of a door follows the half that the player used. An iron door and
  an iron trapdoor do not react to a use.
- **Edge cases:** a block reacts only when the result of the test changes. A use while powered
  changes `open`, and the block keeps that `open` until the test changes again: a door that a
  player closes under power stays closed while the power stays. A fence gate keeps `in_wall`.
- **Conformance tests:** put a block of redstone beside the upper half of an iron door. Result:
  both halves are `open` and `powered`. Remove the block. Result: both halves close. Use the upper
  half of an oak door. Result: both halves open.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-MEC-002: Powered and activator rails

- **Inputs:** a neighbor update at a `minecraft:powered_rail` or a `minecraft:activator_rail`,
  or a placement of one.
- **Behavior:** the rail is powered when the powered-position test at P is true, or when a rail
  of the same block in its line is powered and that rail is powered itself. The **line** of a
  rail follows the axis of its `shape`: the next rail is the neighbor along the axis at the same
  height, one block higher when this rail is `ascending` toward it, or one block lower. The
  search follows the line in both directions, for at most 8 rails. When the result is not equal
  to `powered`, the rail sets `powered` with the flags 3, makes a neighbor request at the block
  below it, and at the block above it when it is ascending. The reach of a rail is 10 blocks
  (`block-simulation.md`, Table 2).
- **Conformance tests:** a line of 12 powered rails with a block of redstone beside the first.
  Result: the first 9 rails are powered and the tenth is not. Remove the block. Result: no rail
  is powered.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-MEC-003: Note block

- **Inputs:** a neighbor update at a `minecraft:note_block`, a placement of one, or a player
  use.
- **Behavior:** the note block sets `instrument` to the instrument of the block below it (the
  component `minecraft:note_block_instrument` of the block definition), or to the instrument of
  the block above it when that instrument is one that works above a note block. When the
  powered-position test changes, the note block sets `powered` with the flags 3. A rising edge
  **plays** the note: the note block queues a block event with `a` = 0 and `b` = 0 at P
  (SIM-BEV-001), and the event is delivered to the clients, which play the sound of
  `instrument` and `note`. A use sets `note` to (`note` + 1) modulo 25 with the flags 3 and
  plays the note.
- **Conformance tests:** power a note block. Result: one block event at P. Remove the power.
  Result: no event. Use the note block. Result: `note` is 1 and one block event at P.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-MEC-004: Hopper lock

- **Inputs:** a neighbor update at a `minecraft:hopper`, or a placement of one.
- **Behavior:** `enabled` is the opposite of the powered-position test at P. When the value
  changes, the hopper sets it with the flags 2. A hopper that is not `enabled` moves no items.
- **Conformance tests:** a block of redstone beside a hopper. Result: `enabled` is false.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-MEC-005: Dispenser, dropper and crafter triggers

- **Inputs:** a neighbor update at a `minecraft:dispenser`, a `minecraft:dropper` or a
  `minecraft:crafter`, or a placement of one.
- **Behavior:** the block is powered when the powered-position test at P is true. A dispenser
  and a dropper are also powered when the test at P + U is true, when the world rule
  `mcrs:quasi_connectivity` is on (RS-RUL-001). When powered and not `triggered`, the block sets
  `triggered` to true with the flags 2 and schedules a tick at P with the delay 4. When not
  powered and `triggered`, it sets `triggered` to false with the flags 2. The tick is the moment
  of the **dispense** or the **craft**: the item logic of these blocks is not part of this
  document (section 13).
- **Conformance tests:** a block of redstone beside a dispenser. Result: `triggered` is true
  and stays true. Remove the block. Result: `triggered` is false.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-MEC-006: Copper bulb

- **Inputs:** a neighbor update at a copper bulb (the eight `*copper_bulb` blocks), or a
  placement of one.
- **Behavior:** when the powered-position test at P is not equal to `powered`, the bulb sets
  `powered` to the test. On a rising edge it also changes `lit` to the other value. The set
  has the flags 3. A falling edge keeps `lit`.
- **Conformance tests:** power a dark bulb. Result: `lit` is true. Remove the power and power it
  again. Result: `lit` is false.

Provenance: `documentation`. Kind: semantic. Confidence: high.

### RS-MEC-007: Tripwire and tripwire hook

- **Inputs:** a placement or a removal of a `minecraft:tripwire_hook` or a `minecraft:tripwire`,
  a neighbor update at a tripwire, a tick at a tripwire or at a hook.
- **Behavior:**
  - The **line** of a hook at P with `facing` F is the run of tripwire blocks at P + F, P + 2F,
    ... up to 40 blocks, that ends at a hook whose `facing` is the opposite of F. A line with
    another block in it, or without the far hook, is **loose**.
  - A **check** of a hook reads its line. Each tripwire of the line sets `attached` to true when
    the line is not loose, and `powered` to its own presence: an entity inside its block while it
    is not `disarmed`. Both hooks set `attached` the same way and `powered` to true when the
    line is not loose and a tripwire of the line is `powered`. Each set has the flags 2 for a
    tripwire and the flags 3 for a hook. A hook whose `powered` changed makes a neighbor request
    at P and at its attached block (RS-SRC-001). A powered hook schedules a tick at P with the
    delay 10, which runs the check again.
  - A placement of a hook, and a tick at a hook, run its check. A removal of a hook sets the
    tripwires of its line and the far hook to not `attached` and not `powered`, and makes the
    neighbor requests of a powered hook.
  - A placement or a removal of a tripwire finds the hooks at the ends of the runs of tripwire
    through its position in each horizontal direction and runs their checks.
  - The server reports the entities inside each tripwire as it does for pressure plates
    (RS-PLT-003); the host knows the tripwires of a section from the moment the section loads.
    A neighbor update at a tripwire whose presence became true runs the checks of its hooks. A
    tripwire whose presence became false schedules a tick at P with the delay 10, and the tick
    runs the checks when the presence is still false.
  - A shape update at a tripwire sets its connection toward the changed neighbor (`north`,
    `east`, `south` or `west`) to true when the neighbor is a tripwire or a hook. A hook pops
    when its attached block loses its full face.
  - The output signal of a hook is as in Table 1. The reach of a tripwire and a hook is 42
    blocks along the line (`block-simulation.md`, Table 5), so their items take the serial
    path of a partitioned step.
- **Conformance tests:** two hooks on facing walls 5 blocks apart with tripwire between them.
  Result: the hooks and the tripwire are `attached`. An entity on a tripwire. Result: both hooks
  are `powered` and a lamp beside a hook lights. The entity leaves. Result: the hooks release
  within 10 ticks and the lamp goes dark 4 ticks later.

Provenance: `documentation`. Kind: semantic. Confidence: medium.

### RS-MEC-008: Daylight detector

- **Inputs:** a placement of a `minecraft:daylight_detector`, a neighbor update at one, or a
  player use.
- **Behavior:** the server computes two values for each detector every 20 ticks: the **sky
  level**, the sky light at P (0..15), and the **sun power**, the sky level scaled by the cosine
  of the sun angle of the clock of the dimension, where the angle is pulled 1/5 of the way
  toward midnight before the cosine, and rounded, and not less than 0. The server reports
  both values to the detector with a neighbor request when they change. The detector sets
  `power` to the sun power, or to 15 minus the sky level when `inverted`, with the flags 3. A
  use changes `inverted` to the other value with the flags 2, and the detector recomputes
  `power`. The output signal is as in Table 1.
- **Conformance tests:** a detector with the sun power 9 and the sky level 15. Result: `power`
  is 9. Use it. Result: `inverted` is true and `power` is 0.

Provenance: `documentation` for the inputs; the sun angle is the celestial angle that the
documentation of the daylight cycle gives. Kind: semantic. Confidence: medium.

---

## 11. World rules

### RS-RUL-001: World rules of this domain

- **Behavior:** each rule is a value of the world. A server operator changes one rule without
  the others. A preset (`java`, `bedrock`) is a named set of values; it is not a mode. The
  server reads the values from its settings: `MCRS_REDSTONE_PRESET` selects the preset, and
  `MCRS_REDSTONE_QUASI_CONNECTIVITY`, `MCRS_REDSTONE_STICKY_PISTON_SHORT_PULSE`,
  `MCRS_REDSTONE_PISTONS_MOVE_BLOCK_ENTITIES`, `MCRS_REDSTONE_DUST_CONNECTS_TO_PISTONS` and
  `MCRS_REDSTONE_EXPERIMENTAL_WIRE` each set one rule on or off over the preset.

| Rule | Default (java) | bedrock | Effect |
|------|----------------|---------|--------|
| `mcrs:quasi_connectivity` | on | off | RS-PST-002, point 2 |
| `mcrs:sticky_piston_short_pulse` | on | off | RS-PST-003, `a` = 2 |
| `mcrs:pistons_move_block_entities` | off | on | RS-PST-004 |
| `mcrs:redstone_dust_connects_to_pistons` | off | on | RS-WIR-002 |
| `mcrs:experimental_wire` | off | off | RS-WIR-009; the setting stands in for the feature `minecraft:redstone_experiments` until the engine exposes feature flags |

- **Edge cases:** the client predicts no redstone, so each rule is server-only.

Provenance: `design`, the owner's decision in the parity notes of the redstone issues. Kind:
semantic. Confidence: high.

---

## 12. Intentional deviations

- **Rule:** RS-PST-007. **Reference:** a moving block completes its move in the block entity
  phase of the tick. **Reason:** this project completes it in a scheduled tick of the moving
  block, in the block tick phase of the same tick. The order inside the tick is the semantic
  policy of the domain. **Visible to the client:** no. **Bug reference:** none. **Test:** a
  differential test compares the states after the tick, not inside it.
- **Rule:** RS-SRC-005. **Reference:** one toggle record for each dimension. **Reason:** a
  partition keeps its own record, as the threaded instance of `block-simulation.md` does.
  **Visible to the client:** no. **Bug reference:** none. **Test:** none.

## 13. Unresolved

- Projectiles (RS-SRC-003) and item frames (RS-CMP-002): the block rules are stated, and no
  projectile or item frame entity exists in this version, so the server has nothing to report
  to the blocks yet.
- The client receives the record of a moving block (RS-PST-007) with the column data; whether
  the client animates the move from a record that arrives during the move is not checked.
- The update order of the experimental wire evaluator (RS-WIR-009) is semantic; the orientation
  of SIM-NBR-007 is not used.
- A player that stands on a plate in a column of another cell presses it only when the player's
  cell owns the plate: the players of a cell are not replicated to the other cells of the world
  (`cells.md`, "Ownership of players and entities").
- Blocks that need a system this version does not have: the explosion of TNT and the primed
  TNT entity; the detector rail and its minecarts; the sculk sensors and the vibration events;
  the lightning rod and lightning; the target block and its projectiles; the trapped chest and
  the count of players that look into a container; the lectern and its book; the jukebox and
  its record. Each of these is a Table 1 row or a mechanism once its system exists. The item
  logic of the dispenser, the dropper and the crafter (RS-MEC-005) belongs to the inventory
  domain.
