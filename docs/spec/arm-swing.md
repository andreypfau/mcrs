# Arm swing replication: behavioural specification

Version: see `assets/minecraft/version.json`. Wire packet IDs: `assets/mcrs/reports/packets.json`. Item
component IDs: `assets/mcrs/reports/registries.json`.

This document states when a server starts an arm swing and updates the cooldown. It also states the
format of `swing_animation` and the set of recipients.

Out of scope:
- entity trackers (`entity-tracking.md`);
- limb render on a client (`client-player-render.md`);
- mob combat (`mob-combat-goals.md`);
- the `/swing` command (`command-swing.md`);
- inventory menu drops and container close drops (`item-entity.md`, `IEN-SPN-010`);
- structure-dependent throws (`ender_eye`) and bed animations;
- block updates (`block-placing.md`, `client-block-breaking.md`).

## 0. Conventions

### 0.1 Terms

- **Actor**: a player, a mob, or an armor stand whose limb animates on screen.
- **Viewer**: a connected player who tracks **actor** under `entity-tracking.md`. A player never
  appears in the viewer set of that player.
- **Recipient**: a **viewer** of **actor**, or also **actor** itself in a **self-included swing**.
- **Loaded client**: a connected player after `player_loaded` or `60` server ticks after join or
  respawn (`block-placing.md`, `PLC-REQ-005`; `movement-checks.md`, `MVC-PKT-012`). Death clears this
  state until respawn load ends (`player-death.md`, `PLD-DTH-003`).
- **Hand**: a swing slot on the wire, either `main` (`0`) or `off` (`1`).
- **Animation type**: an arm motion kind on the wire, either `none` (`0`), `whack` (`1`), or
  `stab` (`2`).
- **Swing packet**: the clientbound `minecraft:swing_animation` (`123`) packet.
- **Self-included swing**: a swing that goes to every **viewer** of **actor** and also to **actor**.
- **Observer-only swing**: a swing that goes only to each **viewer** of **actor**.
- **Trigger**: a packet action or server command that tries to start an arm swing.
- **Base duration**: the raw integer `duration` read from an item component or the fallback value.
- **Effective duration**: the tick duration computed at swing start after status effects scale the
  **base duration**.
- **Cooldown window**: the ticks after a swing starts when the server ignores new swing **triggers**
  for **actor**.

### 0.2 Provenance and kinds

Rule tags follow `registry.md` and `docs/spec/README.md` (`black_box_confirmed` or `design`; kind
`wire`, `semantic`, or `deviation`; confidence `high`). Rules rely on experiments against a
vanilla server of the version in `assets/minecraft/version.json` and on
`tools/mcrs_minecraft_probe/traces/arm_swing.json`.

### 0.3 ID prefixes

- `SWG-PKT-*`: packets on the wire and separate animation packets.
- `SWG-CMP-*`: components and fallback values on items.
- `SWG-STA-*`: shared cooldown across hands and status effect scale.
- `SWG-SRC-*`: actions from a client and their preconditions.
- `SWG-DST-*`: recipients, visibility range, and tick order.
- `SWG-DEV-*`: intentional deviations from vanilla.
- `SWG-E*`: verification experiments.

### 0.4 Observable tick boundaries

A swing **trigger** in tick `T` is subject to cooldown windows that cover `T`
(`SWG-STA-003`, `SWG-STA-005`) and to the same-tick limit (`SWG-STA-004`). When a block interaction
starts a swing, its recipients see the swing packet before the associated block update
(`SWG-DST-003`).

### 0.5 Artefacts cited

- `assets/mcrs/reports/packets.json`
- `assets/mcrs/reports/registries.json`
- `tools/mcrs_minecraft_probe/scenarios/arm_swing.json`
- `tools/mcrs_minecraft_probe/traces/arm_swing.json`

---

## 1. Wire packets

### SWG-PKT-001: clientbound `minecraft:swing_animation` (`123`)

The **swing packet** (`play`, clientbound ID `123`) sends four `VarInt` fields in order:

| Field | Wire type | Values |
|-------|-----------|--------|
| `entity_id` | `VarInt` | Protocol entity ID of **actor** |
| `hand` | `VarInt` | `0` = `main`, `1` = `off` |
| `type` | `VarInt` | `0` = `none`, `1` = `whack`, `2` = `stab` |
| `duration` | `VarInt` | Non-negative **base duration** in ticks |

Once a swing starts, the server must send `swing_animation` even when `type` is `0` (`none`) or `duration` is `0`.
The `main_arm` field in `client_information` must not swap the wire `hand`. Every **recipient** of
one swing event must receive identical payload bytes.

Provenance: `black_box_confirmed`. Kind: `wire`. Confidence: `high`.
Experiment: `SWG-E1`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`punch_empty_shared_observers`, `punch_left_arm`, `punch_attack_none`, `punch_attack_zero`,
`use_item_snowball_offhand`, `interact_pig_carrot_includes_self`).

### SWG-PKT-002: serverbound `minecraft:punch` (`46`)

`minecraft:punch` (`play`, serverbound ID `46`) has zero payload bytes. The server must close the
connection with a disconnect error if the payload has any byte. No serverbound `swing` packet
exists.

Provenance: `black_box_confirmed`. Kind: `wire`. Confidence: `high`.
Experiment: `SWG-E1`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`punch_empty_shared_observers`, `punch_extra_byte_disconnects`).

### SWG-PKT-003: clientbound `minecraft:animate` (`2`) is separate

`minecraft:animate` (`play`, clientbound ID `2`) encodes `(entity_id: VarInt, action: Unsigned Byte)`. For
example, an enchanted weapon hit sends `animate` with action `2` (magic crit) and no
`swing_animation`. The server must not send `animate` for arm swings.

Provenance: `black_box_confirmed`. Kind: `wire`. Confidence: `high`.
Experiment: `SWG-E8`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`attack_pig_sharpness_no_swing_emits_animate`).

---

## 2. Animation components

### SWG-CMP-001: component fields and fallback value

Item components `minecraft:attack_animation` (ID `40`) and `minecraft:interact_animation` (ID
`41`) carry `type` and `duration` (`registry-fields.md`, `REG-FLD-548`; `item-stack.md`). When a
slot is empty or lacks the component, the server must use `whack` (`1`) with duration `6`.

Provenance: `black_box_confirmed`. Kind: `wire`. Confidence: `high`.
Experiment: `SWG-E1`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`punch_empty_shared_observers`, `punch_copper_spear`, `interact_pig_saddle_observers_only`).

### SWG-CMP-002: component selection by action

Each client action reads only its own component and target **hand**:

1. For `punch` (`46`) and `player_action` `stab` (`8`), the server must read
   `minecraft:attack_animation` on `main`. It must ignore `minecraft:interact_animation` and `off`.
2. For `use_item` (`67`), `use_item_on` (`66`), and `interact` (`26`), the server must read
   `minecraft:interact_animation` from the **hand** in the packet. It must ignore
   `minecraft:attack_animation` and the opposite **hand**.
3. For `player_action` `drop_item` (`5`) and `drop_all_items` (`4`), the server must ignore both
   components and send `(main, whack, 6)`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E1`, `SWG-E7`; `tools/mcrs_minecraft_probe/traces/arm_swing.json` (`punch_copper_spear`,
`punch_ignores_interact_animation`, `punch_ignores_offhand`, `stab_spear`,
`use_item_snowball_offhand`, `use_item_on_place_stone_offhand`, `interact_pig_carrot_includes_self`,
`drop_item_nonempty`, `drop_all_items_nonempty`).

### SWG-CMP-003: wire `duration` sends unscaled base duration

The field `duration` in `swing_animation` must carry the unscaled **base duration** from
`SWG-CMP-002`. Status effects on **actor** must not scale the wire field.

Provenance: `black_box_confirmed`. Kind: `wire`. Confidence: `high`.
Experiment: `SWG-E6`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`punch_fatigue_unscaled_and_blocks_reswing`, `punch_haste_overrides_fatigue_sequence`).

---

## 3. Shared cooldown window

### SWG-STA-001: one cooldown window across both hands

Each **actor** has one **cooldown window** shared across `main` and `off` hands. An active window
from `off` must block a **trigger** on `main`, and the reverse also applies.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E6`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`offhand_slow_blocks_mainhand_punch`).

### SWG-STA-002: effective duration under status effects

At the start of a swing, the server must compute the **effective duration** `D` from the
**base duration** `B`:

1. When **actor** has `minecraft:haste` or `minecraft:conduit_power`, `A` is the largest
   `0`-based amplifier among those effects. Then `D = max(0, B - (1 + A))`, and any active
   `minecraft:mining_fatigue` has no effect.
2. Else when **actor** has `minecraft:mining_fatigue` with amplifier `A`, `D = B + (1 + A) * 2`.
3. Else `D = B`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E6`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`punch_fatigue_unscaled_and_blocks_reswing`, `punch_haste_overrides_fatigue_sequence`,
`punch_haste2_shortens_b10`, `punch_fatigue1_blocks_at_200ms`, `punch_fatigue3_blocks_at_300ms`,
`pig_haste0_fatigue4_blocked_at_t2`, `pig_haste0_fatigue4_unlocked_at_t3`,
`punch_baseline_b10_blocked_at_220ms`,
`punch_conduit2_shortens_b10`, `punch_conduit0_overrides_fatigue4_b6`, `pig_conduit0_b6_unlocked_at_t3`,
`punch_haste6_conduit6_b20_max`, `punch_haste10_conduit2_b24`, `punch_haste2_conduit10_fatigue4_b24`,
`punch_haste20_b6_clamp_unlocked`, `pig_fatigue1_blocked_at_t5`, `pig_fatigue1_unlocked_at_t6`,
`pig_haste10_conduit2_unlocked_at_t8`).

### SWG-STA-003: observed cross-tick cooldown window on a pig

When a pig starts a swing at tick `T` with **effective duration** `D >= 0`, it sets a
**cooldown window** fixed at tick `T`. That window covers ticks `T + 1` to `T + floor(D / 2)`.
The server must ignore **triggers** in those ticks and emit no `swing_animation`. Adding dig-speed
status effects after tick `T` must not shorten that window.

A **trigger** can start a new swing from tick `T + 1 + floor(D / 2)` onward. Examples: `D = 1`
unlocks tick `T + 1`; `D = 2` unlocks tick `T + 2`; `D = 5` unlocks tick `T + 3`. `D = 6` or `7`
unlocks tick `T + 4`, and `D = 12` unlocks tick `T + 7`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E6`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`d1_unlocked_at_t1`,
`d2_blocked_at_t1`, `d2_unlocked_at_t2`, `d5_blocked_at_t2`, `d5_unlocked_at_t3`,
`d6_blocked_at_t3`, `d6_unlocked_at_t4`, `d7_blocked_at_t3`, `d7_unlocked_at_t4`,
`d12_blocked_at_t6`, `d12_unlocked_at_t7`, `midwindow_blocked_at_t2`,
`midwindow_blocked_at_t5`, `midwindow_unlocked_at_t6`).

### SWG-STA-004: at most one swing per actor in one tick

When a swing starts at tick `T`, the server must ignore later **triggers** for **actor** in tick `T`
(`SWG-DEV-002`).

Provenance: `design` (owner decision on `#906`). Kind: `deviation`. Confidence: `high`.

### SWG-STA-005: cross-tick cooldown window in mcrs

Mcrs applies the cooldown window in `SWG-STA-003` to every **actor**. For a player it uses the
player's simulation ticks. Switching the held slot after the swing starts preserves that window.
The player observations in `SWG-E6` record suppression and recovery. The exact vanilla player
punch boundaries remain open in `SWG-U3`.

Provenance: `design`. Kind: `semantic`. Confidence: `high`.

---

## 4. Swing sources and preconditions

### SWG-SRC-001: serverbound `minecraft:punch` (`46`)

`punch` starts an **observer-only swing** with `main` and `minecraft:attack_animation` when no
active **cooldown window** (`SWG-STA-005`) or same-tick limit (`SWG-STA-004`) blocks **actor**, and
all of these conditions hold for **actor**:

- **actor** is alive;
- **actor** is a **loaded client**;
- **actor** is not in `spectator` mode.

Players with `invisibility` and players in `adventure`, `survival`, or `creative` mode can **trigger**
`punch`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E4`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`punch_empty_shared_observers`, `punch_invisibility`, `punch_adventure`, `punch_survival`).

### SWG-SRC-002: ignored `punch` (`46`) preconditions

The server must ignore `punch` when any of these conditions holds for **actor** (`SWG-DEV-003`):

- **actor** is not a **loaded client**;
- **actor** is dead;
- **actor** is in `spectator` mode.

Provenance: `design` (owner decision on `#906`). Kind: `deviation`. Confidence: `high`.

### SWG-SRC-003: serverbound `minecraft:player_action`

Subject to the **cooldown window** (`SWG-STA-005`) and same-tick (`SWG-STA-004`) gating on **actor**,
`player_action` starts a swing only for these status codes:

1. `drop_item` (`5`) and `drop_all_items` (`4`): non-spectator players whose `main` hand is not
   empty start an **observer-only swing** with `(main, whack, 6)`. An empty `main` hand and
   `spectator` mode emit no `swing_animation`.
2. `stab` (`8`): outside `spectator` mode, when `main` holds an item with
   `minecraft:piercing_weapon`, `stab` starts an **observer-only swing** with
   `(main, attack_animation)`. That swing starts even on the tick when **actor** equips that item.
   Items without that component and `spectator` mode emit no `swing_animation`.
3. `start_destroy_block`, `abort_destroy_block`, `stop_destroy_block`, `change_destroy_direction`,
   `release_use_item`, and `swap_item_with_offhand` must emit no `swing_animation`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E7`; `tools/mcrs_minecraft_probe/traces/arm_swing.json` (`drop_item_nonempty`,
`drop_all_items_nonempty`, `drop_item_empty`, `spectator_sender_drop_no_swing`,
`stab_immediate_with_spear`, `stab_offhand_no_swing`, `stab_spear`, `stab_non_spear`,
`stab_spectator_no_swing`, `start_destroy_no_swing`, `change_destroy_no_swing`,
`abort_destroy_no_swing`, `stop_destroy_no_swing`, `use_item_eat_start_no_swing`,
`swap_offhand_no_swing`).

### SWG-SRC-004: serverbound `minecraft:attack`

`attack` alone must emit no `swing_animation`. Vanilla clients send a companion `punch` packet when a
swing plays. An enchanted hit still sends `animate` (`2`), not `swing_animation`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E8`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`attack_pig_sharpness_no_swing_emits_animate`).

### SWG-SRC-005: serverbound `minecraft:use_item` (`67`)

Subject to the **cooldown window** (`SWG-STA-005`) and same-tick (`SWG-STA-004`) gating on **actor**,
`use_item` starts a swing for the **hand** in the packet with `minecraft:interact_animation` when the
item use finishes an instant action:

1. An instant projectile throw (`snowball`), a `fishing_rod` cast or reel, and a blank `map`
   transform start an **observer-only swing**.
2. The start of continuous use (`apple`, `bow`, `shield`), an empty hand, and `spectator` mode must
   emit no `swing_animation`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E7`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`use_item_snowball_offhand`, `use_item_empty_map_observers_only`, `use_item_fishing_rod_cast`,
`use_item_fishing_rod_reel`, `use_item_empty_no_swing`, `use_item_eat_start_no_swing`,
`use_item_bow_no_swing`, `use_item_shield_no_swing`, `use_item_spectator_no_swing`).

### SWG-SRC-006: serverbound `minecraft:use_item_on` (`66`)

Subject to the **cooldown window** (`SWG-STA-005`) and same-tick (`SWG-STA-004`) gating on **actor**,
`use_item_on` starts a swing for the **hand** in the packet with `minecraft:interact_animation` when
the block or item use succeeds:

1. A client-predicted block placement (`stone`), interactive block use (`lever`, `chest`), or item use
   (`flint_and_steel`) starts an **observer-only swing**. On a `lever` or a `chest`, an inert or
   empty `main` hand activates the block and swings, whereas an inert `off` hand emits no swing.
2. A server-driven block use (`bone_meal`) starts a **self-included swing**.
3. Refused actions (beyond reach, inside **actor**, or an empty `survival` hand on a
   non-interactive block) emit no `swing_animation`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E7`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`use_item_on_place_stone_offhand`, `use_item_on_lever_main`, `use_item_on_lever_off_ignored`,
`use_item_on_chest_main`, `use_item_on_chest_off_ignored`, `use_item_on_flint_and_steel`,
`use_item_on_in_reach_control`, `use_item_on_out_of_reach_no_swing`,
`use_item_on_inside_hitbox_no_swing`, `use_item_on_refused_survival_empty`,
`use_item_on_lever_survival_empty`, `use_item_on_bone_meal_includes_self`).

### SWG-SRC-007: packet count on `use_item_on` (`66`)

When an accepted `use_item_on` action starts a swing (`SWG-STA-005`, `SWG-STA-004`), the server
must send one `swing_animation` packet to each **recipient** (`SWG-DEV-001`). If `SWG-STA-005` or
`SWG-STA-004` blocks a new swing, `use_item_on` still takes effect while emitting `0` swing packets.

Provenance: `design`. Kind: `deviation`. Confidence: `high`.

### SWG-SRC-008: serverbound `minecraft:interact` (`26`)

Subject to the **cooldown window** (`SWG-STA-005`) and same-tick (`SWG-STA-004`) gating on **actor**,
`interact` starts a swing for the **hand** in the packet with `minecraft:interact_animation` when the
entity interaction succeeds:

1. A server-driven mob feed or shear (`carrot` on `pig`, `shears` on `sheep`) starts a
   **self-included swing**.
2. Equipment placement (`saddle` on `pig`) starts an **observer-only swing**.
3. An item with no interaction effect on the target entity must emit no `swing_animation`.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E7`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`interact_pig_carrot_includes_self`, `interact_sheep_shears_includes_self`,
`interact_pig_inert_no_swing`, `interact_pig_saddle_observers_only`).

---

## 5. Recipients, range, and tick order

### SWG-DST-001: active trackers and self delivery

An **observer-only swing** goes to every **viewer** of **actor** in the same dimension
(`entity-tracking.md`, `TRK-RNG-001`, `TRK-RNG-002`). For a **self-included swing**, the server
must send the packet to every **viewer** of **actor** and, when **actor** is an online player, also to
**actor**. Connected **viewers** in `spectator` mode must receive `swing_animation`.

A player receives no `swing_animation` when any of these conditions holds:

- the player stands in another dimension;
- the player stands outside tracker range;
- **actor** is past corpse removal.

Once a **viewer** enters range again and receives spawn packets for **actor**, new swings reach that
**viewer**.

Provenance: `black_box_confirmed`. Kind: `semantic`. Confidence: `high`.
Experiment: `SWG-E5`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`spectator_observer_receives_punch`, `punch_charlie_in_nether`, `punch_charlie_out_of_range`,
`punch_charlie_back_in_range`, `dead_sender_punch_after_removal`).

### SWG-DST-002: no metadata field or spawn catch-up

`swing_animation` is purely transient. The server must not send `set_entity_data` for swing state,
and must not send `swing_animation` on spawn (`entity-tracking.md`, `TRK-SPN-001`).

Provenance: `black_box_confirmed`. Kind: `wire`. Confidence: `high`.
Experiment: `SWG-E1`, `SWG-E5`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`punch_empty_shared_observers`, `punch_left_arm`, `punch_charlie_back_in_range`).

### SWG-DST-003: intra-tick packet order

Each recipient that receives both packets for a `stab` sees `sound` before `swing_animation`.
For drops and placements, recipients see `swing_animation` before the action's `set_equipment`,
`block_update`, or `section_blocks_update` (`block-placing.md`, `PLC-PLA-003`).

Provenance: `black_box_confirmed`. Kind: `wire`. Confidence: `high`.
Experiment: `SWG-E2`, `SWG-E3`; `tools/mcrs_minecraft_probe/traces/arm_swing.json`
(`drop_item_nonempty`, `stab_spear`, `use_item_on_place_stone_offhand`,
`use_item_on_two_placements_order`).

---

## 6. Intentional deviations

### SWG-DEV-001: single packet per accepted `use_item_on`

- **Rule:** `SWG-SRC-007`.
- **Reference:** vanilla sends two identical `swing_animation` packets in one tick for one
  `use_item_on` (`block-placing.md`, `PLC-PLA-007`). Trace steps: `use_item_on_place_stone_offhand`,
  `use_item_on_lever_main`, `use_item_on_bone_meal_includes_self`.
- **Reason:** a client needs one packet to animate; one packet saves bandwidth with identical visuals.
- **Visible to the client:** yes, because raw packet count is `1` instead of `2` in the same tick.
  The visual limb motion does not change.
- **Bug reference:** none.
- **Test:** when `use_item_on` starts a swing (not blocked by cooldown or `SWG-STA-004`), a
  differential test checks `1` `swing_animation` packet per eligible **recipient** on `mcrs` and
  `2` on vanilla; when blocked by cooldown or `SWG-STA-004`, it checks `0` on `mcrs`.

### SWG-DEV-002: at most one swing per actor in one tick

- **Rule:** `SWG-STA-004`.
- **Reference:** under `tick freeze` with `duration = 60`, `punch_burst_same_tick` emits `2`
  swings per observer whereas the follow-up `punch_after_burst_blocked_while_frozen` emits `0`
  (`setup_burst_same_tick`, `punch_burst_same_tick`, `punch_after_burst_blocked_while_frozen` in
  `tools/mcrs_minecraft_probe/traces/arm_swing.json`). What separates the burst from the blocked
  follow-up in vanilla (`tick` identity, packet batching, or partial ticking under `tick freeze`)
  remains unresolved (`SWG-U1`).
- **Reason:** normal clients send one **trigger** per tick; extra **triggers** must not bypass
  cooldown (owner decision on `#906`).
- **Visible to the client:** no in normal play; yes when a client sends two swing **triggers** in the
  same tick.
- **Bug reference:** none.
- **Test:** a differential test sends at most one swing **trigger** per tick for **actor**.

### SWG-DEV-003: ignored `punch` before load, in `spectator`, or while dead

- **Rule:** `SWG-SRC-002`.
- **Reference:** vanilla emits `swing_animation` on `punch` in `spectator` mode and while
  the dead player's corpse remains tracked. Trace steps: `spectator_sender_punch`,
  `dead_sender_punch_before_removal`.
- **Reason:** unloaded players, corpses, and `spectator` players must not animate for nearby players
  (owner decision on `#906`).
- **Visible to the client:** yes, when a client sends `punch` under one of these conditions:
  - the client is not a **loaded client**;
  - **actor** is dead;
  - **actor** is in `spectator` mode.
- **Bug reference:** none.
- **Test:** a differential test skips `punch` before a player becomes a **loaded client**, while dead,
  and in `spectator` mode.

---

## 7. Known vanilla quirks

None. This specification keeps no vanilla accident (`SWG-DEV-001`, `SWG-DEV-002`, `SWG-DEV-003`).

---

## 8. Experiments

All experiments below ran in `tools/mcrs_minecraft_probe/scenarios/arm_swing.json` and produced
`tools/mcrs_minecraft_probe/traces/arm_swing.json`.

- **SWG-E1** (wire format and components):
  1. Set peaceful world rules (`prepare_peaceful`) and connect `Bob` (`join_bob`).
  2. Connect `Alice` with `player_loaded` deferred (`join_alice_deferred`).
  3. Send `player_loaded` from `Alice` (`send_player_loaded_alice`).
  4. Connect `Charlie` (`join_charlie`).
  5. Send `punch` from `Alice` with an empty main hand (`punch_empty_shared_observers`).
  6. Send `client_information` from `Alice` with `main_arm` set to `left` (`set_left_arm_alice`).
  7. Send `punch` from `Alice` with a `left` main arm (`punch_left_arm`).
  8. Freeze server ticks with a `duration = 60` weapon (`setup_burst_same_tick`), send a two-punch
     burst (`punch_burst_same_tick`), and send one follow-up punch while frozen
     (`punch_after_burst_blocked_while_frozen`).
  9. Send `punch` with `attack_animation` set to `none` (`punch_attack_none`).
  10. Send `punch` with `duration` `0` (`punch_attack_zero`).
  11. Send `punch` with a `copper_spear` in `main` (`punch_copper_spear`).
  12. Send `punch` with `interact_animation` only (`punch_ignores_interact_animation`).
  13. Send `punch` with a `copper_spear` in `off` (`punch_ignores_offhand`).
  14. Connect `Dave` (`join_dave`).
  15. Send `punch` from `Dave` with one extra payload byte (`punch_extra_byte_disconnects`).
  - Result (`punch_empty_shared_observers`): `Bob` and `Charlie` receive four-`VarInt`
    `swing_animation` packets (`swing_bytes`: `canonical: true`, `shared_across_recipients: true`)
    and zero `set_entity_data` packets.
  - Result (`punch_burst_same_tick`, `punch_after_burst_blocked_while_frozen`): while ticks remain
    frozen, `punch_burst_same_tick` emits `2` swings per observer whereas the next frozen punch
    emits `0` (distinguishing the mechanism is left to `SWG-U1`).
  - Result (`punch_left_arm`): `hand` stays `main`.
  - Result (`punch_attack_none`, `punch_attack_zero`, `punch_copper_spear`,
    `punch_ignores_interact_animation`, `punch_ignores_offhand`): `none` and `duration = 0` transmit.
    Component selection follows `SWG-CMP-002`.
  - Result (`punch_extra_byte_disconnects`): `Dave` disconnects with a packet-size error.
- **SWG-E2** (`drop_item` packet order):
  1. Equip `dirt` in the main hand of `Alice` (`setup_drop_item`).
  2. Send `drop_item` from `Alice` with `packet_order` active (`drop_item_nonempty`).
  - Result (`drop_item_nonempty`): `Bob` and `Charlie` receive `swing_animation` (`main`, `whack`,
    `6`) before `set_equipment`.
- **SWG-E3** (`stab` and `use_item_on` packet order):
  1. Equip `wooden_spear` in the main hand of `Alice` (`equip_stab_spear`).
  2. Send `stab` from `Alice` with `packet_order` active (`stab_spear`).
  3. Send `use_item_on` with an off-hand `stone` block (`use_item_on_place_stone_offhand`).
  4. Send two `use_item_on` block placements in sequence while world time is frozen
     (`use_item_on_two_placements_order`).
  - Result (`stab_spear`): `Bob` and `Charlie` receive `sound` before `swing_animation`.
  - Result (`use_item_on_place_stone_offhand`, `use_item_on_two_placements_order`): `Bob` and
    `Charlie` receive `swing_animation` before `block_update` and a single
    `section_blocks_update` for the paired placements under `tick freeze`.
- **SWG-E4** (`punch` preconditions):
  1. Apply `invisibility` to `Alice` (`setup_normal_preconditions_extras`).
  2. Send `punch` from `Alice` (`punch_invisibility`).
  3. Set `Alice` to `adventure` mode (`setup_adventure_punch`).
  4. Send `punch` from `Alice` (`punch_adventure`).
  5. Set `Alice` to `survival` mode (`setup_survival_punch`).
  6. Send `punch` from `Alice` (`punch_survival`).
  - Result (`punch_invisibility`, `punch_adventure`, `punch_survival`): `Bob` and `Charlie` receive
    `swing_animation` (`main`, `whack`, `6`) in each state.
- **SWG-E5** (tracker boundary and dimensions):
  1. Set `Bob` to `spectator` mode (`set_spectator_observer`).
  2. Send `punch` from `Alice` (`spectator_observer_receives_punch`).
  3. Equip `Alice` with a `duration = 600` stick, teleport `Charlie` out of tracker range, and
     wait for `Charlie` to receive Alice's `remove_entities` (`move_charlie_out_of_range`).
  4. Send `punch` from `Alice` (`punch_charlie_out_of_range`).
  5. Move `Charlie` back within range. Wait until he accepts the return `player_position` and his
     Luck duration decreases by at least six ticks. Send position synchronization, then wait for
     `Charlie` to receive Alice's `add_entity`. Send another punch to check that the cooldown still holds
     (`move_charlie_back_in_range`).
  6. After the tracking check, measure at least 301 additional player ticks from Alice's
     decreasing Luck duration. Clear `Alice`'s main hand and send a fresh
     `punch` (`clear_slow_charlie_equipment`, `punch_charlie_back_in_range`).
  7. Teleport `Charlie` to `minecraft:the_nether` (`move_charlie_to_nether`).
  8. Send `punch` from `Alice` (`punch_charlie_in_nether`).
  9. Send `punch` from `Alice` `150` ms after `kill Alice` (`dead_sender_punch_before_removal`).
  10. Send `punch` from dead `Alice` after `1200` ms (`dead_sender_punch_after_removal`).
  - Result (`spectator_observer_receives_punch`): `Bob` receives swings in `spectator` mode.
  - Result (`punch_charlie_out_of_range`, `move_charlie_back_in_range`,
    `punch_charlie_back_in_range`, `punch_charlie_in_nether`): `Charlie` sees zero swings while out
    of range or in `minecraft:the_nether`, and receives zero catch-up swings upon returning while
    Alice's cooldown remains active. Neither observer receives a swing from the blocked punch
    after Charlie starts tracking Alice. The fresh punch delivers to both observers.
  - Result (`dead_sender_punch_before_removal`, `dead_sender_punch_after_removal`): `Bob` gets a
    vanilla swing before corpse removal (`SWG-DEV-003`) and zero swings after removal.
- **SWG-E6** (cooldown and status effect formulas):
  1. Start a slow main-hand punch (`duration = 60`), attempt blocked repeats before and after slot
     swapping, and send the recovery punch inside the same sequence between half and full duration
     (`slow_swing_blocks_reswing_and_slot_switch`).
  2. Run the off-hand `snowball` then main-hand `punch` sequence (`offhand_slow_blocks_mainhand_punch`).
  3. Run the `mining_fatigue` sequence (`punch_fatigue_unscaled_and_blocks_reswing`).
  4. Run the `haste` override sequence (`punch_haste_overrides_fatigue_sequence`).
  5. Run the `haste 2` sequence (`punch_haste2_shortens_b10`).
  6. Run the `mining_fatigue 1` step (`punch_fatigue1_blocks_at_200ms`).
  7. Run the `mining_fatigue 3` step (`punch_fatigue3_blocks_at_300ms`).
  8. Run the wall-clock player observation with `haste 0` and `mining_fatigue 4`
     (`punch_haste0_overrides_fatigue4`).
  9. Run the `B = 10` baseline blocked check at `220` ms (`punch_baseline_b10_blocked_at_220ms`),
     the `conduit_power 2` check on `B = 10` at `220` ms (`punch_conduit2_shortens_b10`), and the
     `conduit_power 0` + `mining_fatigue 4` override check on `B = 6`
     (`punch_conduit0_overrides_fatigue4_b6`).
  10. Run the `haste 6` and `conduit_power 6` sequence (`punch_haste6_conduit6_b20_max`) and the
      reversed `haste 10` and `conduit_power 2` sequence on `B = 24` (`punch_haste10_conduit2_b24`).
  11. Run the `haste 2`, `conduit_power 10`, and `mining_fatigue 4` sequence
      (`punch_haste2_conduit10_fatigue4_b24`).
  12. Run the `haste 20` on `B = 6` sequence (`punch_haste20_b6_clamp_unlocked`).
  13. Freeze ticks and start a `D = 5` swing on a `pig` (`freeze_pig_exact_ticks_d5`).
  14. Probe the frozen `pig` for `D = 5` at `T + 2` and `T + 3` (`d5_blocked_at_t2`,
      `d5_unlocked_at_t3`).
  15. Probe the frozen `pig` for `D = 6` at `T + 3` and `T + 4` (`freeze_pig_d6`,
      `d6_blocked_at_t3`, `d6_unlocked_at_t4`).
  16. Probe the frozen `pig` for `D = 7` at `T + 3` and `T + 4` (`freeze_pig_d7`,
      `d7_blocked_at_t3`, `d7_unlocked_at_t4`).
  17. Probe the frozen `pig` for `D = 12` at `T + 6` and `T + 7` (`freeze_pig_d12`,
      `d12_blocked_at_t6`, `d12_unlocked_at_t7`).
  18. Probe the frozen `pig` for `D = 1` at `T + 1` (`freeze_pig_d1`, `d1_unlocked_at_t1`).
  19. Probe the frozen `pig` for `D = 2` at `T + 1` and `T + 2` (`freeze_pig_d2`,
      `d2_blocked_at_t1`, `d2_unlocked_at_t2`).
  20. Probe the frozen `pig` for `B = 6` with `conduit_power 0` and `mining_fatigue 4` (`D = 5`)
      at `T + 3` (`freeze_pig_conduit0_b6`, `pig_conduit0_b6_unlocked_at_t3`).
  21. Start a `D = 10` swing on the frozen `pig` without effects (`freeze_pig_midwindow_noeffect`),
      apply `haste 20` at `T + 1` (`midwindow_step1_add_haste20`), and probe at `T + 2`,
      `T + 5`, and `T + 6` (`midwindow_blocked_at_t2`, `midwindow_blocked_at_t5`,
      `midwindow_unlocked_at_t6`).
  22. With `mining_fatigue 1` and `B = 6`, send one player `punch` at frozen-world offsets
      `0`, `5`, and `6`. After each punch, wait for the acknowledgement of a following
      block-use packet on the same connection before advancing the world
      (`punch_fatigue1_world_t0` through `punch_fatigue1_world_t6`).
  23. Start a swing on the frozen pig with `mining_fatigue 1` and `B = 6`. Verify the world
      time advances by five ticks, attempt a swing, advance one more tick, and try again
      (`freeze_pig_fatigue1_b6` through `pig_fatigue1_unlocked_at_t6`).
  24. Start a swing on the frozen pig with `haste 0`, `mining_fatigue 4`, and `B = 6`.
      Verify the two-tick and one-tick advances before probing at `T + 2` and `T + 3`
      (`freeze_pig_haste0_fatigue4_b6` through `pig_haste0_fatigue4_unlocked_at_t3`).
  25. Start a swing on the frozen pig with `haste 10`, `conduit_power 2`, and `B = 24`.
      Verify the world time advances by eight ticks before attempting the next swing
      (`freeze_pig_haste10_conduit2_b24` through `pig_haste10_conduit2_unlocked_at_t8`).
  - Result (`slow_swing_blocks_reswing_and_slot_switch`, `offhand_slow_blocks_mainhand_punch`): slot
    swaps keep cooldown (`duration = 6` recovery punch succeeds only after half duration while before
    full `duration = 60`), and both hands share one window.
  - Result (`punch_fatigue_unscaled_and_blocks_reswing`..`punch_haste20_b6_clamp_unlocked`): wire
    `duration` stays unscaled. Mixed effects pick the largest dig-speed amplifier (`max(6, 6)` in
    `punch_haste6_conduit6_b20_max` blocks the `250` ms `stab` attempt and unlocks only the `none`
    attempt at `430` ms; `max(10, 2)` in `punch_haste10_conduit2_b24` unlocks `none` at `420` ms
    rather than delaying to `T + 11` for `conduit_power 2`), and `haste 20` on `B = 6` unlocks below
    `100` ms without underflow.
  - Result (`d5_blocked_at_t2`..`midwindow_unlocked_at_t6`): `D = 1` unlocks on `T + 1`, and larger
    `D` values unlock on `T + 1 + floor(D / 2)`; `D = 5` under `conduit_power 0` unlocks at `T + 3`
    where unscaled `B = 6` (`d6_blocked_at_t3`) still blocks, and adding `haste 20` at `T + 1` does
    not shorten a `D = 10` window started at `T`.
  - Result (`pig_fatigue1_blocked_at_t5`, `pig_fatigue1_unlocked_at_t6`): observers see zero
    packets at `T + 5` and one each at `T + 6`. This matches `D = 10`.
  - Result (`pig_haste10_conduit2_unlocked_at_t8`): observers receive the second swing at `T + 8`.
    Selecting `conduit_power 2` over the larger `haste 10` would block the attempt until `T + 11`.
  - Result (`pig_haste0_fatigue4_blocked_at_t2`, `pig_haste0_fatigue4_unlocked_at_t3`):
    observers see zero packets at `T + 2` and one each at `T + 3`, matching `D = 5`.
  - Result (`punch_fatigue1_world_t0`, `punch_fatigue1_world_t5`, `punch_fatigue1_world_t6`):
    nearby observers receive a swing at each world offset. World stepping therefore fails to
    isolate the same boundaries for player punches (`SWG-U3`).
- **SWG-E7** (`player_action`, `use_item`, `use_item_on`, and `interact`):
  1. Send `drop_item` and `drop_all_items` with non-empty, empty, and `spectator` `main` hands
     (`drop_item_nonempty`, `drop_all_items_nonempty`, `drop_item_empty`,
     `spectator_sender_drop_no_swing`).
  2. Send `swap_item_with_offhand` and the four block-destroy `player_action` codes
     (`swap_offhand_no_swing`, `start_destroy_no_swing`, `change_destroy_no_swing`,
     `abort_destroy_no_swing`, `stop_destroy_no_swing`).
  3. Freeze ticks (`freeze_before_immediate_stab`), verify the world clock is frozen, equip
     a `wooden_spear`, send `stab`, and wait for Bob to receive Alice's swing
     (`stab_immediate_with_spear`). Unfreeze
     (`unfreeze_after_immediate_stab`), and send `stab` on an off-hand spear, stick, and `spectator`
     spear (`stab_offhand_no_swing`, `stab_spear`, `stab_non_spear`, `stab_spectator_no_swing`).
  4. Run the `use_item` steps (`use_item_snowball_offhand`..`use_item_spectator_no_swing`), including
     verifying `foodLevel < 20` (`check_eat_food_level`) before starting and releasing an `apple`
     eat sequence (`use_item_eat_start_no_swing`).
  5. Send `use_item_on` for block placement, `lever`, `chest`, `flint_and_steel`, `bone_meal`,
     near-reach positive control, refused empty/reach/hitbox requests, and empty-hand survival `lever`
     (`use_item_on_place_stone_offhand`..`use_item_on_in_reach_control`..
     `use_item_on_refused_survival_empty`, `use_item_on_lever_survival_empty`).
  6. Send `interact` with `carrot`, inert item, `saddle`, and `shears`
     (`interact_pig_carrot_includes_self`..`interact_sheep_shears_includes_self`).
  - Result (`drop_item_nonempty`, `drop_all_items_nonempty`, `drop_item_empty`,
    `spectator_sender_drop_no_swing`): drops emit a swing from a non-empty main hand outside
    `spectator` mode.
  - Result (`stab_immediate_with_spear`, `stab_offhand_no_swing`, `stab_spear`, `stab_non_spear`,
    `stab_spectator_no_swing`): `stab` swings immediately inside the same frozen equip tick only
    with `piercing_weapon` in `main` outside `spectator` mode.
  - Result (`use_item_fishing_rod_cast`, `use_item_fishing_rod_reel`,
    `use_item_empty_map_observers_only`, `check_eat_food_level`,
    `use_item_eat_start_no_swing`..`use_item_spectator_no_swing`): instant uses emit **observer-only
    swings**. The apple eat sequence confirms `foodLevel < 20` beforehand, emits
    `set_entity_data` and `sound` packets during continuous use without completing (`foodLevel` stays
    below `20`), and emits zero swings across start, continuous eat, and `release_use_item`.
    Continuous use and block breaks emit zero swings.
  - Result (`use_item_on_lever_main`, `use_item_on_lever_off_ignored`,
    `use_item_on_lever_survival_empty`, `use_item_on_chest_main`, `use_item_on_chest_off_ignored`,
    `use_item_on_flint_and_steel`, `use_item_on_bone_meal_includes_self`): a passive item on a
    `lever` or a `chest` swings only on `main` (including an empty `main` hand on a `lever` in
    `survival`). `flint_and_steel` emits an **observer-only swing**, and `bone_meal` emits a
    **self-included swing**.
  - Result (`use_item_on_in_reach_control`, `use_item_on_out_of_reach_no_swing`,
    `use_item_on_inside_hitbox_no_swing`, `use_item_on_refused_survival_empty`): the prepared reach
    control emits a swing while out-of-reach (`[20, 99, 0]` prepared block), inside-hitbox, and
    empty-hand survival requests emit zero swings.
  - Result (`interact_pig_carrot_includes_self`, `interact_sheep_shears_includes_self`,
    `interact_pig_inert_no_swing`, `interact_pig_saddle_observers_only`): `carrot` and `shears` emit
    **self-included swings**. `saddle` emits an **observer-only swing**, and inert items emit zero
    swings.
- **SWG-E8** (`attack` and `animate`):
  1. Summon a `pig` and equip `Alice` with a `sharpness` `5` `diamond_sword`
     (`setup_pig_for_combat`).
  2. Send `attack` from `Alice` against that `pig` (`attack_pig_sharpness_no_swing_emits_animate`).
  - Result (`attack_pig_sharpness_no_swing_emits_animate`): `Alice`, `Bob`, and `Charlie` receive
    `animate` (`2`) for `minecraft:pig` and zero `swing_animation` packets.

---

## 9. Unresolved

- **SWG-U1** (vanilla cooldown-bypass trigger mechanism under `tick freeze`):
  `punch_burst_same_tick` emitted `2` swings while `punch_after_burst_blocked_while_frozen` emitted
  `0` with ticks frozen (`SWG-DEV-002`). Whether vanilla gated the follow-up on server tick count,
  packet batching within one tick handler invocation, or partial ticking while frozen is not yet
  separated; experiment: record `time query gametime` around same-tick batched vs separated punches
  with and without `tick freeze`.
- **SWG-U2** (vanilla `punch` before client load): does an unloaded client cause
  `swing_animation` when it punches? The deferred-join trace does not establish the state
  of the 60-tick fallback when the server handles the packet. The mcrs gate is specified
  in `SWG-SRC-002` and `SWG-DEV-003`.
- **SWG-U3** (exact vanilla player punch cooldown boundaries): what player tick
  processes each isolated punch while world time is frozen? The packet acknowledgement bounds
  input processing in world time. The world clock and player cooldown can advance differently
  under `tick freeze`. This matters for the `mining_fatigue 1` boundary at `T + 5` and `T + 6`.
