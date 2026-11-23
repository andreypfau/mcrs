# The hearts of the health bar in the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states how the vanilla client draws the health of the local player as hearts in the
HUD. It states the source values of the hearts, the game modes that show them, the sprites, the
positions, the order of the draw, the blink after a change of the health and the movement of the
hearts. Section 8 states the food bar beside the hearts. The server side of `set_health` is in
`entity-damage.md`, DMG-NET-005, and `hunger.md`, HNG-NET-001. The death screen is in
`player-death.md`, PLD-SCR-001 to PLD-SCR-004. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Local player**, **frame**, **tick**: as `client-camera.md`, section 0.1, defines them. A tick
  of the client is a **client tick**.
- **HUD**, **HUD game mode**: as `client-hotbar.md`, section 0.1, defines them.
- **In a world**, **screen**: as `client-widget.md`, section 0.1, defines them.
- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them. The origin is the top
  left corner of the GUI. `y` increases down.
- **GUI sprite**: as `client-gui-sprite.md`, section 0.1, defines it. All the sprites of this
  document have the scaling `stretch` and a target of 9 × 9 GUI units.
- **Hidden HUD**: the state that the key F1 turns on and off. While it is on, the HUD draws none of
  its parts.
- **Level loading screen**: as `client-title-screen.md`, section 0.1, defines it.
- **Heart**: one place of 9 × 9 GUI units in the health bar. A heart has an index `i` from 0.
- **Health bar**: the set of all hearts.
- **Health value**: the health of the local player as a number of type `f32` (CHH-SRC-001).
- **Health** `h`: the smallest integer that is not less than the health value.
- **Maximum health** `m`: the value of the attribute `max_health` of the local player
  (CHH-SRC-002).
- **Absorption** `A`: the smallest integer that is not less than the absorption value of the local
  player (CHH-SRC-003).
- **Old health** `D`: the health that the blink layer shows (CHH-BLK-002).
- **Health limit** `M`: the largest of `m`, `D` and `h`, as an `f32`.
- **Health hearts**: the hearts with the index `0` to `C − 1`, with `C = ceil(M / 2)`.
- **Absorption hearts**: the hearts with the index `C` to `C + B − 1`, with `B = ceil(A / 2)`.
- **Hardcore flag**: the hardcore flag of the `login` packet (CHH-SRC-004).
- **Fill kind**: one of normal, poisoned, withered, frozen and absorbing. It selects the sprites of
  a fill (CHH-SPR-002).
- **Heart kind**: the fill kind of the health hearts in a frame. It is normal, poisoned, withered or
  frozen (CHH-SPR-001).
- **Hurt cooldown** `K`: an integer counter of the local player (CHH-BLK-001).
- **HUD tick count** `T`: an integer counter of the HUD (CHH-BLK-002).
- **Blink end** `E`: the value of `T` at which the blink stops (CHH-BLK-002).
- **Blink phase**: a frame in which the blink forms of the sprites show (CHH-BLK-003).
- **Container**: the sprite of an empty heart.
- **Fill**: a sprite that the HUD draws over the container of a heart.
- **Full** and **half**: the two shapes of a fill. A full fill stands for 2 points, a half fill
  for 1 point.
- **Food icon**: one of the ten places of 9 × 9 GUI units of the food bar (section 8).

### 0.2 Provenance, kind and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents".

The observer of each rule is a player in normal play who sees the HUD at a known GUI scale.

The sprite names and the sprite sizes are in the client jar, under
`assets/minecraft/textures/gui/sprites/hud/`. Each other rule that comes from the reference keeps
the label `source_suggested`.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CHH-SRC` | the source values of the hearts |
| `CHH-SHOW` | when the HUD draws the hearts |
| `CHH-CNT` | the number of hearts and of rows |
| `CHH-POS` | the position of each heart |
| `CHH-SPR` | the heart kind and the sprite names |
| `CHH-DRAW` | the layers of a heart and the order of the draw |
| `CHH-BLK` | the blink after a change of the health |
| `CHH-MOV` | the shake at low health and the regeneration wave |
| `CHH-FOOD` | the food bar |
| `CHH-DEV` | intentional deviations |

### 0.4 Scope

In scope:

- the hearts of the local player in the HUD;
- the food bar: its source values, its position, its sprites and its shake.

Out of scope:

- the hearts of a vehicle (the sprites `hud/heart/vehicle_*`);
- the armor bar, the air bubbles and the experience bar. Their positions depend on the hearts and
  on the food bar;
- the hearts in the player list;
- the hotbar (`client-hotbar.md`);
- the death screen (`player-death.md`, PLD-SCR-004).

### 0.5 Constants

| Value | Data | Rule |
|-------|------|------|
| Size of each heart sprite and food sprite | 9 × 9 GUI units | CHH-POS-001, CHH-FOOD-002 |
| Hearts in a row | 10 | CHH-POS-001 |
| Step from one heart to the next in a row | 8 GUI units | CHH-POS-001 |
| `x` of the heart 0 | `floor(W / 2) − 91` | CHH-POS-001 |
| `y` of the row 0 | `H − 39` | CHH-POS-001 |
| Row height | `max(12 − R, 3)` | CHH-CNT-002 |
| Freeze threshold of the frozen kind | 140 ticks frozen | CHH-SPR-001 |
| Blink length after a decrease, after an increase | 20, 10 client ticks | CHH-BLK-002 |
| Blink period | 3 client ticks | CHH-BLK-003 |
| Refresh delay of the old health | 1000 ms | CHH-BLK-002 |
| Hurt cooldown after a decrease, after an increase | 20, 10 | CHH-BLK-001 |
| Shake threshold | `h + A ≤ 4` | CHH-MOV-001 |
| Regeneration wave period | `ceil(M + 5)` client ticks | CHH-MOV-002 |
| Regeneration wave lift | 2 GUI units | CHH-MOV-002 |
| `x` of the food icon `j` | `floor(W / 2) + 91 − 8 · j − 9` | CHH-FOOD-002 |

---

## 1. Source values

### CHH-SRC-001: The health value

- **Inputs:** a `set_health` packet (play clientbound 106); a `set_entity_data` packet (play
  clientbound 101) for the entity id of the local player with the index 9.
- **Behavior:**
  1. A new local player has the health value 20 until the first of these packets.
  2. The client sets the health value to the first field of `set_health`, an `f32`.
  3. The client sets the health value to the `f32` value of the index 9 of `set_entity_data`.
  4. The last packet of the two kinds gives the health value.
- **Postconditions:**
  - The health `h` is the health value rounded up. The health value 0.5 gives `h = 1`, and the
    health value 19.1 gives `h = 20`.
  - A vanilla server sends a health value from 0 to the maximum health. When the health value is
    more than `m`, the result is unspecified. The vanilla client limits the value of a `set_health`
    to the maximum health that it knows at that time.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CHH-SRC-002: The maximum health

- **Inputs:** the attribute instance `max_health` of the local player.
- **Behavior:**
  1. The maximum health `m` is the value of the instance, as `attributes.md`, ATR-VAL-001, computes
     it.
  2. Before the first `update_attributes` for the local player, the instance has the base value 20
     and no modifier (`attributes.md`, ATR-CLI-001).
  3. Each `update_attributes` for the local player changes the instance as `attributes.md`,
     ATR-CLI-002, states.
- **Postconditions:** the HUD reads `m` in each frame. A change shows in the next frame.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CHH-SRC-003: The absorption

- **Inputs:** a `set_entity_data` packet for the entity id of the local player with the index 17.
- **Behavior:**
  1. The index 17 of a player is an `f32`, the absorption value. Its default is 0.
  2. The absorption `A` is the absorption value rounded up.
- **Postconditions:** the effect `absorption` gives 4 points for each level (`status-effects.md`).
  Thus the level 1 gives `A = 4` and two absorption hearts.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

### CHH-SRC-004: The hardcore flag

- **Inputs:** the `login` packet (play clientbound 50).
- **Behavior:**
  1. The hardcore flag is the Boolean field of the `login` packet that follows the entity id.
  2. A `respawn` packet does not change the hardcore flag. The flag stays until the next `login`
     packet.
- **Postconditions:** the death screen reads the same flag (`player-death.md`, PLD-SCR-004).

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

### CHH-SRC-005: The effects and the freeze state

- **Inputs:** `update_mob_effect` (play clientbound 135) and `remove_mob_effect` (play clientbound
  79) for the local player; the index 7 of `set_entity_data` for the local player.
- **Behavior:**
  1. The local player has an effect from the `update_mob_effect` of that effect until the
     `remove_mob_effect` of that effect. The client does not remove an effect when its duration
     ends.
  2. The rules of this document read the effects `poison`, `wither`, `regeneration` and `hunger`.
  3. The local player is fully frozen when the index 7 is 140 or more (`client-entities.md`,
     CENTS-DATA-003, step 4).
- **Postconditions:** a new local player after a `respawn` packet has no effect until the next
  `update_mob_effect`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 2. When the hearts show

### CHH-SHOW-001: The game modes with hearts

- **Inputs:** a frame in a world.
- **Behavior:** the HUD draws the health bar when all these conditions are true:
  1. The HUD game mode is survival or adventure.
  2. The HUD is not hidden.
  3. The level loading screen does not show.
- **Postconditions:**
  - In creative mode and in spectator mode, the HUD draws no hearts and no food bar. It draws
    nothing in their place.
  - An open screen does not stop the draw. The screen draws over the HUD. Thus the hearts show
    under the chat screen and under the death screen.
  - A change of the game mode shows in the first frame after the change.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 3. Number and position

### CHH-CNT-001: The number of hearts

- **Inputs:** the health limit `M` and the absorption `A`.
- **Behavior:**
  1. The number of health hearts is `C = ceil(M / 2)`.
  2. The number of absorption hearts is `B = ceil(A / 2)`.
  3. The health bar has `C + B` hearts, with the index `0` to `C + B − 1`.
- **Postconditions:**
  - The maximum health 20 gives 10 health hearts. The maximum health 21 gives 11 health hearts.
  - The maximum health 1024 gives 512 health hearts.
  - Each health heart has a container, also when the health is less than `M`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CHH-CNT-002: The rows and the row height

- **Inputs:** the health limit `M` and the absorption `A`.
- **Behavior:**
  1. The row count is `R = ceil((M + A) / 20)`. The client computes the division as `f32`.
  2. The row height is `Y = max(12 − R, 3)` GUI units.
- **Postconditions:**
  - The row height shrinks as the row count increases:

| `R` | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 or more |
|-----|---|---|---|---|---|---|---|---|-----------|
| `Y` | 11 | 10 | 9 | 8 | 7 | 6 | 5 | 4 | 3 |

  - A row height of less than 9 makes the rows overlap.
- **Edge cases:** `R` can be less than the number of rows that the hearts use. Example: `M = 19`
  and `A = 1` give `R = 1` and `Y = 11`, but 11 hearts. The heart 10 is on the row 1, 11 GUI
  units above the row 0.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CHH-POS-001: The position of a heart

- **Inputs:** a heart with the index `i`, the row height `Y`, the shake `s` (CHH-MOV-001) and the
  lift `w` (CHH-MOV-002) of the heart.
- **Behavior:**
  1. The row of the heart is `r = floor(i / 10)`. The column is `c = i mod 10`.
  2. The left edge of the heart is `x = floor(W / 2) − 91 + 8 · c`.
  3. The top edge of the heart is `y = H − 39 − r · Y + s − w`.
  4. The heart is 9 × 9 GUI units.
- **Postconditions:**
  - The left edge of the heart 0 is the left edge of the hotbar. The hotbar is 182 GUI units wide,
    from `x = floor(W / 2) − 91`, with its top at `y = H − 22`.
  - The row 0 is 17 GUI units above the top of the hotbar. The row 0 is at the bottom, and the
    next rows go up.
  - Two hearts that are next to each other in a row overlap by one column of GUI units.
  - Example: a GUI of 480 × 270 GUI units puts the heart 0 at `(149, 231)` and the heart 9 at
    `(221, 231)`. With `R = 2`, the heart 10 is at `(149, 221)`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 4. Sprites

### CHH-SPR-001: The heart kind

- **Inputs:** the effects and the freeze state of the local player (CHH-SRC-005).
- **Behavior:** the first true condition gives the heart kind:
  1. The local player has the effect `poison`: poisoned.
  2. The local player has the effect `wither`: withered.
  3. The local player is fully frozen: frozen.
  4. Else: normal.
- **Postconditions:** one heart kind applies to all the health hearts of a frame. CHH-DRAW-001,
  layer 2, states the kind of the fill of an absorption heart.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CHH-SPR-002: The sprite names

- **Inputs:** a layer of a heart: the container or a fill; for a fill, the fill kind and the
  shape; the hardcore flag; the blink form.
- **Behavior:** the name of the GUI sprite is `minecraft:hud/heart/` and then these parts, in this
  order, with no separator other than the `_` of each part:
  1. the prefix of the fill kind: none for normal; `poisoned_`, `withered_`, `frozen_` or
     `absorbing_` for the others. A container has no prefix;
  2. `hardcore_` when the hardcore flag is true;
  3. the body: `container` for a container, `full` or `half` for a fill;
  4. `_blinking` for the blink form.
- **Exception:** the container puts `_hardcore` after the body, not `hardcore_` before it.
- **Postconditions:** the table lists each name. Each name exists in the vanilla pack.

| Layer | Not hardcore | Hardcore |
|-------|--------------|----------|
| Container | `container`, `container_blinking` | `container_hardcore`, `container_hardcore_blinking` |
| Normal | `full`, `half`, `full_blinking`, `half_blinking` | `hardcore_full`, `hardcore_half`, `hardcore_full_blinking`, `hardcore_half_blinking` |
| Poisoned | `poisoned_full`, `poisoned_half`, `poisoned_full_blinking`, `poisoned_half_blinking` | `poisoned_hardcore_full`, `poisoned_hardcore_half`, `poisoned_hardcore_full_blinking`, `poisoned_hardcore_half_blinking` |
| Withered | `withered_full`, `withered_half`, `withered_full_blinking`, `withered_half_blinking` | `withered_hardcore_full`, `withered_hardcore_half`, `withered_hardcore_full_blinking`, `withered_hardcore_half_blinking` |
| Frozen | `frozen_full`, `frozen_half`, `frozen_full_blinking`, `frozen_half_blinking` | `frozen_hardcore_full`, `frozen_hardcore_half`, `frozen_hardcore_full_blinking`, `frozen_hardcore_half_blinking` |
| Absorbing | `absorbing_full`, `absorbing_half` | `absorbing_hardcore_full`, `absorbing_hardcore_half` |

- **Edge cases:** the vanilla pack also has the four sprites `absorbing_*_blinking`. The health
  bar never draws them (CHH-DRAW-001, layer 2).

Provenance: `source_suggested_black_box_confirmed` for the names, from the client jar. Kind:
layout. Confidence: high.

---

## 5. The draw

### CHH-DRAW-001: The layers of a heart

- **Inputs:** a heart with the index `i`; `C`, `h`, `D` and `A`; the heart kind; the hardcore flag;
  the blink phase.
- **Behavior:** the HUD draws these layers at the position of the heart, in this order. Let
  `n = 2 · i`.
  1. The container. It has the blink form in a blink phase.
  2. For an absorption heart only: let `a = 2 · (i − C)`. When `a < A`, draw a fill of
     the fill kind absorbing, or of the fill kind withered when the heart kind is withered. The
     shape is half when `a + 1 = A`, else full. This fill never has the blink form.
  3. In a blink phase only: when `n < D`, draw a fill of the heart kind in the blink form. The
     shape is half when `n + 1 = D`, else full.
  4. When `n < h`, draw a fill of the heart kind. The shape is half when `n + 1 = h`, else full.
     This fill does not have the blink form.
- **Postconditions:**
  - The health 13 with `M = 20` and no blink shows the hearts 0 to 5 full, the heart 6 half and
    the hearts 7 to 9 as containers.
  - Only the last fill can be half. The hearts before it are full.
  - Layers 3 and 4 do not draw on an absorption heart, because `h` and `D` are not more than `M`.
  - The absorption hearts keep the fill kind absorbing under the effect `poison` and in the frozen
    state.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CHH-DRAW-002: The order of the hearts

- **Inputs:** the hearts of a frame.
- **Behavior:**
  1. The HUD draws the hearts from the index `C + B − 1` down to the index 0.
  2. The HUD draws all the layers of one heart before the next heart.
- **Postconditions:**
  - Where two hearts overlap, the heart with the lower index is on top.
  - In a row, the right column of each heart covers the left column of the heart to its right.
  - Where rows overlap (CHH-CNT-002), a lower row covers the row above it.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 6. The blink

### CHH-BLK-001: The hurt cooldown

- **Inputs:** `damage_event` (play clientbound 25), `set_health` and the index 9 of
  `set_entity_data` for the local player.
- **Behavior:**
  1. A new local player has `K = 0`.
  2. A `damage_event` with the entity id of the local player sets `K = 20`.
  3. A packet of CHH-SRC-001 that sets a health value lower than the old health value sets
     `K = 20`.
  4. A packet of CHH-SRC-001 that sets a health value higher than the old health value sets
     `K = 10`.
  5. A packet that keeps the health value does not change `K`.
  6. The first packet of CHH-SRC-001 after the client creates the local player does not change
     `K`. The client creates a local player at a `login` packet and at a `respawn` packet.
  7. In each client tick of the local player, `K` decreases by 1 while it is more than 0.
- **Postconditions:** the health value at the join and after a respawn does not start a blink.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Steps 3 and 4 for the index 9
are a deviation (CHH-DEV-001).

### CHH-BLK-002: The start of a blink and the old health

- **Inputs:** the health `h` in each client tick; the hurt cooldown `K`.
- **Behavior:** the HUD keeps the HUD tick count `T`, the blink end `E`, the old health `D`, the
  health `L` of the last client tick and a refresh time `t`. At the start, all are 0. In each client
  tick, while the game is not paused, the HUD does these steps:
  1. Increase `T` by 1.
  2. When `h < L` and `K > 0`, set `E = T + 20` and set `t` to the current time.
  3. When `h > L` and `K > 0`, set `E = T + 10` and set `t` to the current time.
  4. When more than 1000 ms passed since `t`, set `D = h` and set `t` to the current time.
  5. Set `L = h`.
- **Postconditions:**
  - The time of steps 2 to 4 is real time in milliseconds, not a count of ticks.
  - In the client tick of a change, `D` keeps the health of the last refresh. Thus the blink
    layer shows the health before the change.
  - `D` changes no earlier than 1000 ms after the last start of a blink.
  - Only a singleplayer game pauses. A paused game stops the blink, the shake and the
    regeneration wave.
  - `L` does not restart for a new local player.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CHH-BLK-003: The blink phase

- **Inputs:** `T` and `E` in a frame.
- **Behavior:** let `q = E − T`. The frame is in a blink phase when `q > 0` and `floor(q / 3)` is
  odd.
- **Postconditions:**
  - The value of `q` stays the same in each frame of one client tick.
  - A blink of 20 client ticks has the blink phase at `q` = 17 to 15, 11 to 9 and 5 to 3. It
    has three flashes of 3 client ticks each. The first flash starts 3 client ticks after the
    change.
  - A blink of 10 client ticks has the blink phase at `q` = 10, 9 and 5 to 3. Its first flash
    starts in the client tick of the change.
  - A new change during a blink starts the blink again from its first client tick.
  - Example: a hit lowers the health from 20 to 13 with `M = 20`. In a blink phase, the
    containers have the blink form, the hearts 7 to 9 show a full fill in the blink form, and
    the heart 6 shows the normal half fill over the full fill in the blink form.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 7. The movement of the hearts

### CHH-MOV-001: The shake at low health

- **Inputs:** the health `h` and the absorption `A`.
- **Behavior:**
  1. When `h + A ≤ 4`, each heart of the health bar gets a shake `s` of 0 or 1 GUI unit down.
     Else `s = 0` for each heart.
  2. The HUD chooses `s` for each heart separately, with the chance 1/2 for each value.
  3. The HUD chooses again in each client tick. All frames of one client tick show the same
     choice.
- **Postconditions:**
  - The absorption hearts shake with the health hearts.
  - The health 0 shakes the containers, for example under the death screen.
  - The shake changes 20 times in each second at the normal tick rate, not in each frame.
- **Edge cases:** the vanilla client chooses from a random sequence with the seed
  `T · 312871`. The same sequence also gives the shake of the food bar. Which heart gets which
  value is unspecified.

Provenance: `source_suggested`. Kind: unspecified. Confidence: medium. Reason: a player sees the
shake, but not the choice for each heart.

### CHH-MOV-002: The regeneration wave

- **Inputs:** the effect `regeneration` of the local player; `T`; the health limit `M`.
- **Behavior:**
  1. When the local player does not have the effect `regeneration`, no heart has a lift.
  2. Else, let `k = T mod ceil(M + 5)`. When `k < C`, the heart `k` has the lift `w = 2` GUI
     units up. Each other heart has `w = 0`.
- **Postconditions:**
  - An absorption heart never has a lift.
  - With `M = 20`, the period is 25 client ticks. The hearts 0 to 9 rise one after another in 10
    client ticks. Then no heart rises for 15 client ticks.
  - The level of the effect does not change the wave.
  - A heart can have a shake and a lift in the same frame.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 8. The food bar

### CHH-FOOD-001: The source values and the visibility of the food bar

- **Inputs:** `set_health`; the frame conditions of CHH-SHOW-001; the vehicle of the local player.
- **Behavior:**
  1. The food level `f` is the second field of `set_health`, a VarInt.
  2. The saturation is the third field of `set_health`, an `f32`.
  3. A new local player has the food level 20 and the saturation 5 until the first `set_health`.
  4. The HUD draws the food bar when it draws the health bar (CHH-SHOW-001).
  5. The HUD does not draw the food bar while the local player rides a living entity whose health
     the HUD shows. The hearts of a vehicle are out of scope.
- **Postconditions:** the effect `hunger` comes from CHH-SRC-005.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CHH-FOOD-002: The layout and the sprites of the food bar

- **Inputs:** the food level `f`; the effect `hunger`; the shake `u` (CHH-FOOD-003).
- **Behavior:**
  1. The food bar has ten food icons, with the index `j` from 0 to 9.
  2. The left edge of the food icon `j` is `x = floor(W / 2) + 91 − 8 · j − 9`. Its top edge is
     `y = H − 39 + u`. The food icon is 9 × 9 GUI units.
  3. The HUD draws the food icons from `j = 0` to `j = 9`. For each food icon, it draws the empty
     sprite, then a full sprite when `2 · j + 1 < f`, then a half sprite when `2 · j + 1 = f`.
  4. Without the effect `hunger`, the sprites are `minecraft:hud/food_empty`,
     `minecraft:hud/food_full` and `minecraft:hud/food_half`.
  5. With the effect `hunger`, the sprites are `minecraft:hud/food_empty_hunger`,
     `minecraft:hud/food_full_hunger` and `minecraft:hud/food_half_hunger`.
- **Postconditions:**
  - The food icon 0 is at the right end. Its right edge is the right edge of the hotbar.
  - The food bar is on the same line as the row 0 of the hearts.
  - The food level 20 shows ten full icons. The food level 7 shows the icons 0 to 2 full, the icon
    3 half and the icons 4 to 9 empty.
  - The food icon `j` covers the left column of the food icon `j − 1`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CHH-FOOD-003: The shake of the food bar

- **Inputs:** the saturation, the food level `f` and `T`.
- **Behavior:**
  1. When the saturation is 0 or less and `T mod (3 · f + 1) = 0`, each food icon gets a shake
     `u` of −1, 0 or 1 GUI unit. Else `u = 0`.
  2. The HUD chooses `u` for each food icon separately, with the chance 1/3 for each value.
  3. All frames of one client tick show the same choice.
- **Postconditions:**
  - The food level 0 with no saturation shakes the food bar in each client tick.
  - The food level 20 with no saturation shakes it in one client tick of each 61.
- **Edge cases:** the vanilla client uses the random sequence of CHH-MOV-001. Which icon gets which
  value is unspecified.

Provenance: `source_suggested`. Kind: unspecified. Confidence: medium. Reason: a player sees the
shake, but not the choice for each icon.

---

## 9. Known vanilla quirks

None.

---

## 10. Intentional deviations

### CHH-DEV-001: The index 9 sets the hurt cooldown

- **Rule:** CHH-BLK-001, steps 3 and 4.
- **Reference:** the vanilla client sets the hurt cooldown only for a `set_health` and a
  `damage_event`. A change of the index 9 of `set_entity_data` sets the health value and keeps the
  hurt cooldown. When the index 9 arrives before the `set_health` of the same change, the
  `set_health` keeps the health value. Then a heal starts no blink.
- **Reason:** the blink then depends on the order of two packets of the server. A player cannot see
  that order. With the rule, each heal shows a blink of 10 client ticks, and each loss shows a
  blink of 20 client ticks.
- **Visible to the client:** yes, as a blink of the containers after a heal, when the server sends
  the index 9 first.
- **Bug reference:** none.
- **Test:** a differential test of the HUD ignores the blink after a heal when the index 9 of that
  heal arrived before its `set_health`.

---

## 11. Unresolved

None.
