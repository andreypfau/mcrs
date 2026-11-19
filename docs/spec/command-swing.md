# The `swing` command: behavioural specification

| Field | Value |
|-------|-------|
| Version | see `assets/minecraft/version.json` |
| Permission requirement | `gamemasters` (`command-permissions.md`, `PERM-CMD-001`) |
| Parser ids | `minecraft:entity` 6, `minecraft:time` 43, `minecraft:swing_animation` 60 |

Source of the parser ids: `assets/mcrs/reports/registries.json`, registry
`minecraft:command_argument_type`.

This document states the tree, the arguments, the behaviour, the feedback, and the errors of
`/swing`. It also states one intentional deviation. It does not prescribe an architecture.

Out of scope: cooldown mechanics after `punch` or item use (`arm-swing.md`).

---

## 0. Conventions

### 0.1 Terms

- **Node**, **literal**, **argument**, **executable**, **source**, **cursor**, **position**,
  **error**, **suggestion**: as `command-parser.md` defines them.
- **Operator**, **system message**: as `commands-packet.md` defines them.
- **Display name**, **result**, and **success**: as `command-execute.md`, section 0.1 and
  `CMD-EX-040`, define them.
- **Broadcast flag**: the command feedback flag that notifies other online operators
  (`commands-packet.md`, `CMDP-EXEC-004`).
- **Actor**, **hand**, **animation type**, **base duration**, **effective duration**, **cooldown window**,
  **swing packet**, **self-included swing**, **viewer**, **recipient**: as `arm-swing.md`,
  section 0.1, defines them.
- **Living entity**: a player, a mob, or an armor stand. Item entities and other objects are outside
  this class.
- **Default parameters**: hand `main`, animation type `whack`, and duration `6` ticks.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The rule kinds follow `docs/spec/README.md`,
section "Semantics, not accidents".

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CMD-SWG` | the `swing` command, its tree, its arguments, its behaviour, and its errors |
| `CMD-SWG-DEV` | intentional deviations from vanilla |
| `CMD-SWG-E` | verification experiments |

---

## 1. The tree

### CMD-SWG-001: Tree and permission

```
swing                                               require command_level gamemasters; executable
└── <targets: minecraft:entity>                     executable; entities, any amount (wire 0x00)
    ├── mainhand                                    executable
    │   └── <animation: minecraft:swing_animation>  executable; properties none
    │       └── <duration: minecraft:time>          executable; min 1 tick
    └── offhand                                     executable
        └── <animation: minecraft:swing_animation>  executable; properties none
            └── <duration: minecraft:time>          executable; min 1 tick
```

- The root node `swing` requires permission level `gamemasters` (`command-permissions.md`,
  `PERM-CMD-001`).
- Only `swing` sets the `restricted` bit (`0x20`) on operator trees (`commands-packet.md`,
  `CMDP-PKT-001`). Every child node clears that bit.
- `<targets>` uses parser `minecraft:entity` (`6`) with the wire byte `0x00` (`single` false and
  `players_only` false).
- Child literals under `<targets>` appear in order `mainhand`, then `offhand`.
- Under both literals, `<animation>` uses parser `minecraft:swing_animation` (`60`) with no
  properties and no custom suggestion type.
- `<duration>` uses parser `minecraft:time` (`43`) with the 4-byte wire `Int` minimum `1`
  (`commands-packet.md`, `CMDP-ARG-001`; `command-time.md`, `CMD-TIME-003`).
- A non-operator or a permission-level-`1` operator sees no `swing` subtree. If that player sends
  `swing`, the parse fails with `command.unknown.command` and `command.context.here`
  (`commands-packet.md`, `CMDP-EXEC-003`). A permission-level-`2` (`gamemasters`) operator receives
  the `swing` subtree and can run `/swing`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`;
`tools/mcrs_minecraft_probe/traces/command_swing.json`: `join_alice`, `join_bob`, `join_carl_op1`,
`swing_op1_carl_fails`, `join_dan_op2`, `swing_op2_dan_succeeds`, `deop_alice`,
`swing_deopped_alice_fails`, `reop_alice`). Kind: wire, client agreement. Confidence: high.

---

## 2. Argument types

### CMD-SWG-002: `minecraft:swing_animation`

- **Properties:** none on the wire (`commands-packet.md`, `CMDP-ARG-001`).
- **Behaviour:**
  1. The server reads an unquoted string token with `CMD-RD-006` (`command-parser.md`).
  2. The server matches that token with exact case against this table:

| Keyword | Animation type (`arm-swing.md`, `section 0.1`) | Wire value |
|---------|------------------------------------------------|------------|
| `none` | `none` | `0` |
| `whack` | `whack` | `1` |
| `stab` | `stab` | `2` |

  3. If no keyword matches, the server stops the cursor at the token end. It then fails with
     `argument.swing_animation.invalid` and that token.
- **Suggest:**
  - The candidate list in order is `none`, `stab`, `whack`.
  - Both server replies (`commands-packet.md`, `CMDP-SUG-002`) and client lists apply `CMD-SUG-008`
    (`command-parser.md`). None of the keywords contain `.`, `_`, or `/`, so `CMD-SUG-008` matches
    lower-case starts (`S` matches `stab`).
- **Edge cases:**
  - Input `WHACK` reads the token `WHACK` and fails after `WHACK` with the argument `WHACK`.
  - Input `@` reads the empty token `""` (`CMD-RD-006`). The parse fails before `@` with the argument `""`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`;
`tools/mcrs_minecraft_probe/traces/command_swing.json`: `swing_mainhand_stab`,
`swing_mainhand_none_15`, `swing_invalid_animation`, `swing_uppercase_animation`,
`swing_bad_token_at`, `suggest_anim_all`, `suggest_anim_s`, `suggest_anim_upper_s`). Kind: wire,
client agreement. Confidence: high.

### CMD-SWG-003: Duration argument

- `<duration>` parses with `minecraft:time` (`command-time.md`, `CMD-TIME-003`) and minimum `1`.
- Duration units resolve to integer ticks, for example `2s` resolves to `40` ticks.
- Values below `1` tick, such as `0`, fail with `argument.time.tick_count_too_low` and arguments
  `[1, 0]`.

Provenance: `black_box_confirmed` (`tools/mcrs_minecraft_probe/traces/command_swing.json`:
`swing_bob_time_2s`, `swing_duration_zero_fails`). Kind: client agreement, semantic. Confidence:
high.

---

## 3. Targets and defaults

### CMD-SWG-004: Omitted arguments and item suppression

- When the command omits `<targets>`, it targets only the entity of the source.
- If the source has no entity, for example the console, it fails with `permissions.requires.entity`.
- When the command omits optional parameters, each absent field uses its default (`section 0.1`):
  - the omitted hand uses `main`;
  - the omitted animation type uses `whack`;
  - the omitted duration uses `6` ticks.
- The command ignores held item components `minecraft:attack_animation` and
  `minecraft:interact_animation`.

Provenance: `black_box_confirmed` (`tools/mcrs_minecraft_probe/traces/command_swing.json`:
`swing_bare_alice`, `swing_mainhand_omits_anim`, `swing_offhand`,
`swing_mainhand_stab`, `swing_console_bare_fails`). Kind: semantic, client agreement. Confidence:
high.

### CMD-SWG-005: Target selection and entity filter

- First, the server resolves `<targets>` with `minecraft:entity` (`argument-entity.md`, `ARG-ENT-013`).
- If zero entities match, it fails with `argument.entity.notfound.entity` (`ARG-ENT-016`). The run
  returns success false and value `0`.
- Next, the server filters those entities in order and keeps only **living entities** (`section 0.1`).
  Items (`minecraft:item`) and other entities outside that class drop out of the list.
- Kept targets can be players in `creative`, `survival`, `adventure`, or `spectator` mode, mobs, and
  armor stands.
- If zero **living entities** remain, the command fails with `commands.swing.failed.notliving` and
  returns success false and value `0`.

Provenance: `black_box_confirmed` (`tools/mcrs_minecraft_probe/traces/command_swing.json`:
`swing_multiple_targets`, `swing_item_only_fails`, `swing_notfound_fails`, `swing_console_pig`,
`swing_spectator_self`). Kind: semantic, wire. Confidence: high.

---

## 4. Execution, cooldown, and delivery

### CMD-SWG-006: Swing trigger and delivery

- Let **base duration** `B` be the resolved duration in ticks (`<duration>` or `6`). The wire field
  `duration` sends unscaled `B` (`SWG-CMP-003`). Status effects (`haste`, `mining_fatigue`,
  `conduit_power`) scale **effective duration** `D` via `SWG-STA-002` (`arm-swing.md`).
- For each target `E` that `CMD-SWG-005` keeps, in order:
  1. The server checks the cooldown state of `E` (`SWG-STA-005`) and the same-tick limit
     (`SWG-STA-004`, `CMD-SWG-DEV-001`).
  2. When neither blocks `E`, `E` starts one swing on that tick (`CMD-SWG-DEV-001`). The server
     computes `D` with `SWG-STA-002` and starts a **cooldown window** on ticks `T + 1` to
     `T + floor(D / 2)` (`SWG-STA-005`). It then emits a **self-included swing** with wire duration
     `B` (`SWG-PKT-001`, `SWG-CMP-003`, `SWG-DST-001`).
- **Cooldown properties:**
  - Even animation `none` starts that **cooldown window** on ticks `T + 1` to `T + floor(D / 2)`.
  - While the window lasts, later `punch` or `/swing` calls emit no `swing_animation`.
  - When an active cooldown on `E` skips `swing_animation`, `E` still counts toward `N` in
    `CMD-SWG-007`.
- **Delivery:**
  - Every **viewer** of `E` in range within the same dimension receives `swing_animation`.
  - When `E` is an online player, `E` also receives the **swing packet** even in `spectator` mode,
    unless `E` is past corpse removal (`SWG-DST-001`; in that state zero recipients receive a packet,
    yet `E` still counts toward `N` in `CMD-SWG-007`).
  - Viewers in other dimensions receive no packet, even when the source stands there.

Provenance: `black_box_confirmed` (`tools/mcrs_minecraft_probe/traces/command_swing.json`:
`swing_mainhand_none_15`, `punch_whack_positive_control`, `swing_none_blocks_later_punch`,
`swing_bob_time_2s`, `swing_cooldown_blocks_later_punch`, `punch_cooldown_blocks_later_swing`,
`swing_while_blocked_reports_success`, `swing_while_blocked_score`, `freeze_pig_cmd_cd`,
`try_pig_cmd_t1`..`try_pig_cmd_t4`, `cmd_h2_blocked_at_t3`, `cmd_h2_unlocked_at_t4`,
`cmd_c2_blocked_at_t3`, `cmd_c2_unlocked_at_t4`, `cmd_f4_blocked_at_t8`, `cmd_f4_unlocked_at_t9`,
`swing_haste_default`, `swing_haste_explicit`, `swing_fatigue_explicit`, `swing_conduit_explicit`,
`swing_spectator_self`, `swing_cross_dimension_target`, `wait_dead_bob_removal`, `swing_dead_bob_past_removal`,
`swing_dead_bob_score`) for cooldown, effect scaling, and delivery. The exact tick measurements
cover pig targets (`SWG-STA-003`). The mcrs timing for players follows `SWG-STA-005`.
Kind: `semantic, wire`. Confidence: `high`.

Same-tick per-actor repeat suppression follows `SWG-STA-004` (`CMD-SWG-DEV-001`). Provenance:
`design`. Kind: `deviation`. Confidence: `high`.

---

## 5. Feedback and result

### CMD-SWG-007: Feedback and return value

- Let `N` (`N >= 1`) be the count of targets that `CMD-SWG-005` keeps.
- The run completes with **success** true and **result** value `N` (`CMD-EX-040`).
- Success feedback sets the **broadcast flag** (`commands-packet.md`, `CMDP-EXEC-004`):
  - When `N == 1` for target `E`: the server sends `commands.swing.success.single` with the **display
    name** of `E`.
  - When `N > 1`: the server sends `commands.swing.success.multiple` with the integer `[N]`.
- When `send_command_feedback` is false, the server skips chat replies to the source and operators.
  Execution, the packet rules in `CMD-SWG-006` (emitting **swing packets** only when not blocked by
  cooldown or same-tick suppression), and **result** `N` still occur.
- Error exits (`permissions.requires.entity`, `argument.entity.notfound.entity`,
  `commands.swing.failed.notliving`) send red text to the source with the **broadcast flag** clear.
  Those exits return **success** false and **result** `0`.

Provenance: `black_box_confirmed` (`tools/mcrs_minecraft_probe/traces/command_swing.json`:
`swing_bare_alice`, `swing_multiple_targets`, `swing_item_only_fails`, `swing_notfound_fails`,
`swing_while_blocked_reports_success`, `swing_while_blocked_score`, `swing_execute_store_result`,
`swing_feedback_false`, `swing_feedback_false_score`, `swing_same_tick_execute`,
`wait_dead_bob_removal`, `swing_dead_bob_past_removal`, `swing_dead_bob_score`). Kind: wire, semantic. Confidence: high.

---

## 6. Intentional deviations

### CMD-SWG-DEV-001: Same-tick repeat suppression aligns with actor rules

- **Rule:** `CMD-SWG-006` (by reference to `arm-swing.md`, `SWG-STA-004` and `SWG-DEV-002`).
- **Reference:** when two `/swing` calls hit one **actor** inside one tick, vanilla emits two
  `swing_animation` packets and returns `1` for each call (`setup_same_tick_execute`,
  `swing_same_tick_execute` in `tools/mcrs_minecraft_probe/traces/command_swing.json`).
- **Reason:** an owner choice on `#906` (`SWG-DEV-002`) keeps one uniform tick limit across all swing
  sources. Two swings on one tick produce no extra client animation effect.
- **Visible to the client:** yes, because **recipients** receive one `swing_animation` packet instead
  of two on same-tick repeats. Multi-tick behaviour and feedback stay identical.
- **Bug reference:** none.
- **Test:** for two `/swing` calls on one **actor** in one tick with no prior cooldown, a
  differential test requires `1` `swing_animation` packet per **recipient** on `mcrs` and `2` on
  vanilla, while checking identical chat replies and scores.

---

## 7. Known vanilla quirks

None. Section 6 states the single deviation, and this command keeps no vanilla accident.

---

## 8. Experiments

All experiments below ran in `tools/mcrs_minecraft_probe/scenarios/command_swing.json` and produced
`tools/mcrs_minecraft_probe/traces/command_swing.json`.

- **CMD-SWG-E1** (tree, permissions, suggestions, defaults, targets, and errors):
  1. Set peaceful world rules (`prepare_peaceful`).
  2. Connect `Alice` and `Bob` (`join_alice`, `join_bob`).
  3. Run the arena console setup (`prepare_arena`).
  4. Join level-`1` operator `Carl`, run `swing`, and disconnect `Carl`
     (`join_carl_op1`, `swing_op1_carl_fails`, `leave_carl_op1`).
  5. Join level-`2` operator `Dan`, run `swing Dan mainhand`, and disconnect `Dan`
     (`join_dan_op2`, `swing_op2_dan_succeeds`, `leave_dan_op2`).
  6. De-op `Alice`, send `swing`, and re-op `Alice` (`deop_alice`, `swing_deopped_alice_fails`,
     `reop_alice`).
  7. Request suggestions for prefix `""` (`suggest_anim_all`).
  8. Request suggestions for prefix `s` (`suggest_anim_s`).
  9. Request suggestions for prefix `S` (`suggest_anim_upper_s`).
  10. Run bare `swing` (`swing_bare_alice`).
  11. Run `swing Alice mainhand` (`swing_mainhand_omits_anim`).
  12. Run `swing Alice offhand` (`swing_offhand`).
  13. Run `swing Alice mainhand stab` (`swing_mainhand_stab`).
  14. Run `swing Alice mainhand none 15` (`swing_mainhand_none_15`).
  15. Run `swing Bob offhand stab 2s` (`swing_bob_time_2s`).
  16. Run `swing @e[distance=..5]` (`swing_multiple_targets`).
  17. Run a console `swing` on a pig (`swing_console_pig`).
  18. Run a bare console `swing` (`swing_console_bare_fails`).
  19. Run `swing @e[type=minecraft:item]` (`swing_item_only_fails`).
  20. Run `swing @a[name=nobody]` (`swing_notfound_fails`).
  21. Run `swing Alice mainhand bad_anim` (`swing_invalid_animation`).
  22. Run `swing Alice mainhand WHACK` (`swing_uppercase_animation`).
  23. Run `swing Alice mainhand @` (`swing_bad_token_at`).
  24. Run `swing Alice mainhand stab 0` (`swing_duration_zero_fails`).
  - Result (`join_alice`, `join_bob`, `join_carl_op1`, `swing_op1_carl_fails`, `join_dan_op2`,
    `swing_op2_dan_succeeds`, `deop_alice`, `swing_deopped_alice_fails`, `reop_alice`): level `2`
    (`Dan`) sees `restricted: true` `swing` and succeeds; level `1` (`Carl`) and de-opped `Alice`
    receive no `swing` node and fail with `command.unknown.command`.
  - Result (`suggest_anim_all`, `suggest_anim_s`, `suggest_anim_upper_s`): candidates return
    `["none", "stab", "whack"]` for `""` and `["stab"]` for `s` and `S`.
  - Result (`swing_bare_alice`, `swing_mainhand_omits_anim`, `swing_offhand`, `swing_mainhand_stab`,
    `swing_mainhand_none_15`, `swing_bob_time_2s`, `swing_console_pig`): omitted parameters fall back to
    defaults (`main`, `whack`, `6`) and ignore held item components; `swing_bob_time_2s` resolves
    explicitly to `(off, stab, 40)` (`CMD-SWG-003`).
  - Result (`swing_multiple_targets`, `swing_item_only_fails`, `swing_notfound_fails`,
    `swing_console_bare_fails`): `@e[distance=..5]` hits `4` targets and `@e[type=minecraft:item]` fails
    with `commands.swing.failed.notliving`. Empty selectors and bare console fail with exact errors.
  - Result (`swing_invalid_animation`, `swing_uppercase_animation`, `swing_bad_token_at`,
    `swing_duration_zero_fails`): invalid tokens fail at the documented cursor position and argument.
- **CMD-SWG-E2** (cooldowns, status effects, spectator, feedback, scoreboard, and cross-dimension):
  1. Equip `Alice` with a `whack` `6` stick and record an unlocked positive-control punch
     (`equip_whack_control_alice`, `punch_whack_positive_control`).
  2. Run the `none 40` and `punch` sequence (`swing_none_blocks_later_punch`).
  3. Run the slow `/swing` and `punch` sequence (`swing_cooldown_blocks_later_punch`).
  4. Freeze ticks (`equip_slow_stick_alice`) and send `punch`. Wait for Bob to receive
     Alice's `swing_animation`, run `tick step 2`, and use `time query gametime` to verify
     that the frozen world's game clock advanced by two ticks.
     Then run `/swing` with `execute store result` and read the score before unfreezing
     (`punch_cooldown_blocks_later_swing`).
  5. Reset `swing_res` (`99`), start a slow `punch` cooldown on `Alice`
     (`equip_very_slow_stick_alice`, `punch_very_slow_for_blocked_cmd`).
  6. Run `/swing` while that cooldown lasts (`swing_while_blocked_reports_success`).
  7. Query the scoreboard result of that blocked call (`swing_while_blocked_score`).
  8. Freeze ticks with `swing @e[type=minecraft:pig,limit=1] mainhand stab 6` (`freeze_pig_cmd_cd`).
  9. Probe the unscaled frozen pig across ticks `T + 1` to `T + 4`
     (`try_pig_cmd_t1`..`try_pig_cmd_t4`).
  10. Start `B = 10` on the pig with `haste 2` (`cmd_haste2_pig_d10`).
  11. Probe `haste 2` on the pig at `T + 3` (`cmd_h2_blocked_at_t3`).
  12. Probe `haste 2` on the pig at `T + 4` (`cmd_h2_unlocked_at_t4`).
  13. Start `B = 10` on the pig with `conduit_power 2` (`cmd_conduit2_pig_d10`).
  14. Probe `conduit_power 2` on the pig at `T + 3` (`cmd_c2_blocked_at_t3`).
  15. Probe `conduit_power 2` on the pig at `T + 4` (`cmd_c2_unlocked_at_t4`).
  16. Start `B = 6` on the pig with `mining_fatigue 4` (`cmd_fatigue4_pig_d6`).
  17. Probe `mining_fatigue 4` on the pig at `T + 8` (`cmd_f4_blocked_at_t8`).
  18. Probe `mining_fatigue 4` on the pig at `T + 9` (`cmd_f4_unlocked_at_t9`).
  19. Unfreeze server ticks (`unfreeze_pig_cmd_cd`).
  20. Apply `haste 3` to `Alice` (`setup_effects_haste`).
  21. Run `swing Alice` under `haste 3` (`swing_haste_default`).
  22. Run `swing Alice mainhand stab 20` under `haste 3` (`swing_haste_explicit`).
  23. Apply `mining_fatigue 2` to `Alice` (`setup_effects_fatigue`).
  24. Run `swing Alice mainhand stab 20` under `mining_fatigue 2` (`swing_fatigue_explicit`).
  25. Apply `conduit_power 0` to `Alice` (`setup_effects_conduit`).
  26. Run `swing Alice mainhand stab 20` under `conduit_power 0` (`swing_conduit_explicit`).
  27. Run `execute at Alice store result score Alice swing_res run swing @e[distance=..5] mainhand`
      followed by `swing @e[type=minecraft:item] mainhand`, reading `swing_res` after each
      (`swing_execute_store_result`).
  28. Disable `send_command_feedback` (`feedback_off`).
  29. Run `/swing` under `execute store result` with feedback off while observing both `Alice` and
      `Bob` chat, then read the score (`swing_feedback_false`, `swing_feedback_false_score`).
  30. Set `Alice` to `spectator` mode (`alice_spectator`).
  31. Run `swing` as `spectator` `Alice` (`swing_spectator_self`).
  32. Reset both scores (`99`) and execute two same-tick `/swing Alice` calls storing each player's
      result (`setup_same_tick_execute`, `swing_same_tick_execute`).
  33. Teleport `Bob` to `minecraft:the_nether` (`move_bob_nether`).
  34. Run `swing Bob` from Overworld `Alice` (`swing_cross_dimension_target`).
  35. Return `Bob` to the Overworld, wait past post-teleport invulnerability (`3500` ms), kill
      `Bob`, and wait for Alice to receive Bob's `remove_entities` before running
      `execute store result score Alice swing_res run swing Bob` (`return_bob_overworld`,
      `wait_bob_return_invuln`, `wait_dead_bob_removal`,
      `swing_dead_bob_past_removal`, `swing_dead_bob_score`).
  - Result (`punch_whack_positive_control`, `swing_none_blocks_later_punch`,
    `swing_cooldown_blocks_later_punch`, `punch_cooldown_blocks_later_swing`,
    `swing_while_blocked_reports_success`, `swing_while_blocked_score`,
    `swing_execute_store_result`): unlocked `whack` emits a swing, `none` sets cooldown, and both
    sources share one cooldown window. After the recorded punch and two world ticks, the offhand
    `/swing` produces zero additional swings. Blocked `/swing` still updates
    `99 -> 1` (`Alice has 1 [swing_res]`) and sends
    `commands.swing.success.single`; `swing_execute_store_result` records `4` for
    `@e[distance=..5]` (excluding the item entity) and `0` for `@e[type=minecraft:item]`.
  - Result (`try_pig_cmd_t1`..`try_pig_cmd_t4`): the unscaled frozen pig unlocks at tick `T + 4`.
  - Result (`cmd_h2_blocked_at_t3`..`cmd_f4_unlocked_at_t9`): `haste 2` and `conduit_power 2` unlock
    `B = 10` at `T + 4`. `mining_fatigue 4` blocks `B = 6` at `T + 8` and unlocks it at `T + 9`.
  - Result (`swing_haste_default`, `swing_haste_explicit`, `swing_fatigue_explicit`,
    `swing_conduit_explicit`): wire `duration` in `swing_animation` stays unscaled (`6` and `20`).
  - Result (`swing_feedback_false`, `swing_feedback_false_score`, `swing_spectator_self`,
    `swing_same_tick_execute`, `swing_cross_dimension_target`, `swing_dead_bob_past_removal`,
    `swing_dead_bob_score`): `feedback` off silences chat for both `Alice` and `Bob`, emits the
    swing, and returns `1`. `spectator` `Alice` gets her packet, `swing_same_tick_execute` stores
    `1` for both invocations with two swings per recipient on vanilla, and Nether `Bob` receives
    cross-dimension `/swing`. Past corpse removal, dead `Bob` emits `0` swing packets (`swings: []`)
    while still returning `Alice has 1 [swing_res]`.

---

## 9. Unresolved

None.
