# /give Command

- **Status**: Draft specification
- **Last updated**: 2026-11-15
- **Game version**: `26.4-snapshot-7` (client `protocol_version`: `1073742169`, `world_version`: `5127`)
- **Permission requirement**: `gamemasters` ([command-permissions.md](command-permissions.md), `PERM-CMD-001`)
- **Parser ids**: `brigadier:integer` (`3`), `minecraft:entity` (`6`), `minecraft:item_stack` (`14`) ([registries.json](../../assets/mcrs/reports/registries.json))
- **Clientbound packets**: `add_entity` (`1`), `commands` (`16`), `container_set_content` (`18`), `container_set_slot` (`20`) ([packets.json](../../assets/mcrs/reports/packets.json))
- **More clientbound packets**: `remove_entities` (`78`), `set_entity_data` (`101`), `sound` (`119`), `system_chat` (`124`), `take_item_entity` (`127`)

This document specifies wire behavior, client agreement, and execution semantics of `/give`.

---

## 0. Conventions

### 0.1 Terms

- **Operator**: A connected player whose permission level satisfies the `gamemasters` requirement (`PERM-CMD-001`).
- **Viewer**: A connected player client in the `play` state who observes packets or chat broadcasts.
- **Target**: Each online player entity selected by the `targets` argument.
- **Empty stack**: An item stack with item id `minecraft:air`, count `0`, or no non-empty item value ([item-stack.md](item-stack.md)).
- **Stack size**: Effective `minecraft:max_stack_size` (`1..99`), `1` when removed (`!minecraft:max_stack_size`), or `1` for `minecraft:air`.
- **Strict validation**: The checks of STK-VAL-001. STK-CMD-006 states their command error.
- **Display name**: The display name of the source or target, as defined in `command-execute.md`.
- **Item text**: The `chat.square_brackets` text component built for an item stack in feedback and errors (`CMD-GIVE-013`).
- **Result** and **success**: The values of a command run, as defined in `command-execute.md`.
- **Broadcast flag**: Source feedback and admin broadcast controls defined by command execution rules ([commands-packet.md](commands-packet.md), `CMDP-EXEC-004`).

### 0.2 Evidence

Primary evidence sources for this specification are:

- Registry reports in [registries.json](../../assets/mcrs/reports/registries.json) and packet reports in [packets.json](../../assets/mcrs/reports/packets.json).
- Client scenario [give_scenario.json](../../tools/mcrs_minecraft_probe/give_scenario.json) and console cases [give_cases.json](../../tools/mcrs_minecraft_probe/give_cases.json).
- [give-tree.json](../../tools/mcrs_minecraft_probe/traces/give-tree.json): `commands` packets before and after operator promotion.
- [give-client.json](../../tools/mcrs_minecraft_probe/traces/give-client.json): `system_chat` replies for 25 client commands and admin broadcast.
- [give-console.json](../../tools/mcrs_minecraft_probe/traces/give-console.json): console outputs and sound counts for 29 cases.
- [give-sound.json](../../tools/mcrs_minecraft_probe/traces/give-sound.json): decoded pickup sound packets for 17 cases.

### 0.3 Provenance and kinds

Every rule carries `Provenance`, `Kind`, and `Confidence` tags per [README.md](README.md).

- The labels of `registry.md`, section 0.3, apply.
- A reference to a shared rule inherits the provenance and confidence of that rule.
- **Kind**: `wire`, `client agreement`, `semantic`, `layout`, `deviation`, or `unspecified`.

### 0.4 ID prefixes

- `CMD-GIVE-001` .. `CMD-GIVE-013`: `/give` command syntax, validation, execution, sound, and text rules.
- `D6`: Intentional deviation shared with [item-entity.md](item-entity.md).
- `U1` .. `U7`: Open questions in Section 7.

---

## 1. Command Syntax and Permissions

### `CMD-GIVE-001` Command tree and syntax

The command uses this syntax:

```text
give <targets> <item> [<count>]
```

In the `commands` (`16`) packet, the server publishes one subtree under root for operators:

| Node | Type | Flags | Executable | Parser (`id`) | Wire properties |
| --- | --- | --- | --- | --- | --- |
| `give` | literal | `33` (`0x21`) | no | none | none |
| `targets` | argument | `2` (`0x02`) | no | `minecraft:entity` (`6`) | `2` (`players_only = true`, `single = false`) |
| `item` | argument | `6` (`0x06`) | yes | `minecraft:item_stack` (`14`) | none |
| `count` | argument | `6` (`0x06`) | yes | `brigadier:integer` (`3`) | `flags = 1` (`has_min = true`), `min = 1` |

When `<count>` is absent, the command uses `count = 1`.
The exact item suggestions and their source are unresolved (U5).

- **Provenance**: `black_box_confirmed` (`give-tree.json`, `give-client.json`, `single-default`, and `registries.json`).
- **Kind**: `wire`, `client agreement`.
- **Confidence**: high.

---

### `CMD-GIVE-002` Operator permission gate

The `give` root node sets the `restricted` bit (`0x20`) on operator trees.
A player who cannot use `give` receives a tree without that subtree.
After the player gains permission, the server sends a tree that includes it.

If a non-operator sends `chat_command` with `give ...`, the server rejects execution as a parse error.
The server sends two red `system_chat` packets: `command.unknown.command` and `command.context.here` (`reply_count: 2`).
This permission check also runs when a client sends raw commands with stale local trees.

- **Provenance**: `black_box_confirmed` (`give-tree.json`; `give-client.json`, `no-permission`). Also `source_suggested` (inherited) ([command-permissions.md](command-permissions.md), `PERM-CMD-001`).
- **Kind**: `wire`, `client agreement`, `semantic`.
- **Confidence**: high for the captured cases; medium for inherited or general rules.

---

## 2. Argument Parse, Validation, and Precedence

### `CMD-GIVE-003` Target resolution

The `targets` node uses the `players_only` (`0x02`) multi-target `minecraft:entity` parser ([argument-entity.md](argument-entity.md)).
A selector able to match non-player entities, such as `@e`, fails parse with `argument.player.entities` and `command.context.here`.
The selectors resolve online players under ARG-ENT-014 and ARG-ENT-018.
World-limited selectors search the source dimension.

Dead players whose respawn screen stays open (`immediate_respawn = false`) stay online and match `@a` or explicit name.
If zero online players match `targets`, runtime fails with `argument.entity.notfound.player` and no `command.context.here`.
Console execution with `@s` has no player source and fails with `argument.entity.notfound.player`, not `permissions.requires.player`.

- **Provenance**: `black_box_confirmed` (`give-client.json`: `non-player-selector`, `offline-before-limit`). Also `black_box_confirmed` (`give-console.json`: `multiple`, `cross_dimension`, `argument_errors`, `console_precedence`, `dead_target`).
- **Kind**: `semantic`, `wire`.
- **Confidence**: high.

---

### `CMD-GIVE-004` Item syntax, empty brackets, and strict validation

The `<item>` argument accepts an item identifier and an optional component patch (STK-CMD-001).
An identifier without a namespace uses `minecraft` (STK-CMD-002).
Empty brackets (`stone[]` or `air[]`) parse as an empty component patch equal to the bare item id.
Parse checks reject unknown ids (`argument.item.id.invalid`) and unknown component keys (`arguments.item.component.unknown`).
Parse checks also reject invalid values such as `damage=-1` or `max_stack_size=100` (`arguments.item.component.malformed`).

After target lookup succeeds, the server validates the stack (STK-VAL-001 and STK-CMD-006).
Incompatible pairs like `diamond_sword[max_stack_size=16]` fail at runtime with `arguments.item.malformed` (`"Item cannot be both damageable and stackable"`).
Runtime strict errors send one `system_chat` packet with no `command.context.here`.

- **Provenance**: `black_box_confirmed` (`give-client.json`: `malformed-component`, `empty-brackets`, `unknown-item`, `malformed-strict`). Also `black_box_confirmed` (`give-console.json`: `empty_brackets`, `invalid_component`, `console_precedence`) and `source_suggested` (inherited) ([item-stack.md](item-stack.md), `STK-CMD-001..004`).
- **Kind**: `semantic`, `wire`.
- **Confidence**: high for the captured cases; medium for inherited or general rules.

---

### `CMD-GIVE-005` Count bounds and per-target limit

Parse checks enforce `count >= 1` (`argument.integer.low`, with `command.context.here`).
Let `S` denote the stack size (`1..99` from `minecraft:max_stack_size`, or `1` when `!minecraft:max_stack_size` removes it).
When the item id is `minecraft:air`, `S` stays `1` even when a `max_stack_size` patch appears.

The inclusive limit is `100 * S` items per target, compared against `count` per individual target.
For two targets, `/give @a stone[max_stack_size=1] 60` succeeds, and `/give @s stone[max_stack_size=4] 400` succeeds.
If `count > 100 * S`, execution fails at runtime with `commands.give.failed.toomanyitems` (`(100 * S, item_text)`).
On limit failure, no inventory changes, no item entities spawn, no sounds play, `#result = 0`, and `#success = 0`.

- **Provenance**: `black_box_confirmed` (`give-client.json`: `zero`, `over-limit-tool`, `over-limit-patched`, `toomanyitems-epic`, `air-over-limit`). Also `black_box_confirmed` (`give-console.json`: `limit`, `per_target_limit`, `limit_inclusive`, `removed_max_stack_size`, `modified_air`, `console_precedence`).
- **Kind**: `semantic`, `wire`.
- **Confidence**: high for the captured cases; medium for inherited or general rules.

---

### `CMD-GIVE-006` Error classification and evaluation precedence

Parse errors follow `CMDP-EXEC-003` and runtime errors follow `CMD-ERR-001` and `CMDP-EXEC-004` ([command-parser.md](command-parser.md), [commands-packet.md](commands-packet.md)):

1. **Left-to-right parse**: Runs first; stops at the earliest syntax, unknown-item, invalid-component-value, or `count < 1` error.
   Client receives two `system_chat` packets (`error` + `command.context.here`); console prints `<--[HERE]`.
2. **Target resolution**: Runs first at runtime; fails with `argument.entity.notfound.player` before strict checks or count limits.
3. **Strict stack validation**: Runs before the `100 * S` limit; `diamond_sword[max_stack_size=16] 1601` fails with `arguments.item.malformed`.
4. **Per-target count limit**: Fails with `commands.give.failed.toomanyitems` when `count > 100 * S`.

All runtime errors (steps 2 to 4) send one red `system_chat` packet with no `command.context.here`.
Runtime errors also omit `<--[HERE]` from console output.

- **Provenance**: `black_box_confirmed` (`give-client.json`: `zero`, `malformed-component`, `unknown-item`, `malformed-strict`, `offline-before-limit`, `over-limit-patched`). Also `black_box_confirmed` (`give-console.json`: `error_precedence`, `invalid_component`, `console_precedence`).
- **Kind**: `wire`, `semantic`.
- **Confidence**: high.

---

## 3. Inventory Mutation, Entities, and Client State

### `CMD-GIVE-007` Behavior for `minecraft:air`

Any stack with item id `minecraft:air` (bare `air`, `air[]`, or patched `air[custom_name=...]`, `air[max_stack_size=N]`) acts as an empty stack.
Parse checks (`CMD-GIVE-004`) and the `100` limit (`CMD-GIVE-005`) still run on modified and bare `minecraft:air`.

When `count <= 100`, execution skips inventory mutation, spawns no item entity, and emits no pickup sound.
The command still succeeds (`#result = target_count`, `#success = 1`) and outputs `[Air]` (`CMD-GIVE-013`).

- **Provenance**: `black_box_confirmed` (`give-client.json`: `air`, `empty-brackets`, `modified-air`, `air-over-limit`). Also `black_box_confirmed` (`give-console.json`: `air`, `empty_brackets`, `modified_air`).
- **Kind**: `semantic`, `wire`.
- **Confidence**: high.

---

### `CMD-GIVE-008` Greedy split, insertion, overflow, and target modes

For each target in order, `/give` splits a non-empty `count` greedily into parts `k = min(remaining, S)`.
Each part enters the inventory under INV-INS-001. That rule sets the slot order.
It includes the selected slot, the off-hand slot and the main slots.
A compatible stack with room takes items before an empty main slot:

- **Complete insertion (`inserted == k`)**: The server spawns one `count: 1` display item entity (`IEN-SPN-004`) and emits one pickup sound (`CMD-GIVE-010`).
- **Partial or zero insertion (`0 <= inserted < k`)**: The leftover `k - inserted` items spawn on the ground (`IEN-SPN-004`). No pickup sound plays for that part.

Suppose `hotbar.0` holds `63` stone and all other slots hold `64` dirt.
Then `give GiveProbeA stone 100` fills `hotbar.0` to `64`, drops two ground entities (`63` and `36`), and plays `0` sounds.
Suppose `hotbar.0` is empty and all other slots hold `64` dirt.
Then `give GiveProbeA stone 100` fills `hotbar.0` to `64`, plays `1` sound, spawns a `count: 1` display entity, and drops a `36` ground entity.

Mixed multi-target commands evaluate each target separately with no rollback (`mixed_multi_target`).
The captured spectator target receives items and overflow drops (`spectator_target`).
The captured dead target receives items into an empty inventory (`dead_target`).
The full-inventory case for a dead target is unresolved (U6).

**Intentional deviation (`D6`)**: For a creative target with no room, vanilla destroys the items.
The viewers see a display item as for a success.
Under `D6` in [item-entity.md](item-entity.md), `mcrs` uses the uniform overflow rule and drops uninserted items.
See `Q6` in [item-entity.md](item-entity.md) for the link between `D6` and `IEN-PCK-006`.

- **Provenance**: `black_box_confirmed` for the cited captures (`give-console.json`: `split`, `partial_survival`, `full_survival`, `greedy_split_partial`, `greedy_split_one_slot`, `mixed_multi_target`, `spectator_target`, `dead_target`; `give-sound.json`). The creative overflow rule is `design` (D6). The shared rules keep their own provenance.
- **Kind**: `semantic`.
- **Confidence**: high for the captured cases and D6.

---

### `CMD-GIVE-009` Client inventory sync, statistics, and advancements

Targets receive inventory updates under INV-SYN-003 through INV-SYN-008 in [inventory.md](inventory.md).
Item entities follow the rules in `item-entity.md`.
In particular, a display item does not send `take_item_entity` (IEN-SPN-004).
A later pickup of a drop follows the rules of section 5 in that document.

The capture reads `minecraft.picked_up:minecraft.stone` before and after a give of five `stone`.
The value stays at 0.
A `cobblestone` inserted by `give` triggers the `Stone Age` advancement.
The capture does not decide every statistic or advancement (U7).

- **Provenance**: `black_box_confirmed` (`give-console.json`: `stats_and_advancement`). Also `source_suggested` (inherited) ([inventory.md](inventory.md), `INV-SYN-003..008`; [item-entity.md](item-entity.md), `IEN-SPN-004`).
- **Kind**: `wire`, `semantic`.
- **Confidence**: high.

---

## 4. Sound, Output, Feedback, and Text Format

### `CMD-GIVE-010` Pickup sound broadcast

For each part where `inserted == k` on a non-air item, the server sends `sound` (`119`) with:

- `sound_id = 654` (`minecraft:entity.item.pickup`, VarInt `655` on the wire),
- `source = 7` (`players`),
- fixed-point `x, y, z` for the target position: multiply each coordinate by 8 and truncate toward zero,
- `volume = 0.2` (`f32`),
- variable `pitch` (`f32`), identical for all viewers of that event.

Recipients are connected players in the **same dimension** as the target.
Euclidean distance from the sound position to the viewer must be strictly below `16.0` blocks (`distance < 16.0`).
In traces, a viewer at `dx = 14.0` receives the packet (`sound_in_range`).
A viewer at exact `dx = 16.0` or in `minecraft:the_nether` receives no packet (`sound_at_boundary`, `sound_other_dimension`).
A command that inserts `m` full parts emits `m` `sound` packets per viewer (`split`: `2` sounds).

- **Provenance**: `black_box_confirmed` (`give-sound.json`, 17 cases; `give-console.json`: `split`, `greedy_split_partial`, `greedy_split_one_slot`, `sound_in_range`, `sound_at_boundary`, `sound_other_dimension`, `sound_fraction_positive` and `sound_fraction_negative`). The packet and sound ids come from the registry and packet reports.
- **Kind**: `wire`, `semantic`.
- **Confidence**: high for the captured cases; medium for the general position rule.

---

### `CMD-GIVE-011` Result and success output values

A command that fails during target resolution, strict validation or the limit check returns result 0 and success 0.
A parse error prevents the command run. It does not write the targets of `execute store`.
When `/give` succeeds, the result is the resolved target count (`N >= 1`). The success is 1.
This holds when inventories are full or the item is `minecraft:air`.
It also holds for the captured dead and spectator targets.

- **Provenance**: `black_box_confirmed` (`give-console.json`: `default`, `multiple`, `limit`, `per_target_limit`, `limit_inclusive`, `air`, `invalid_component`, `partial_survival`, `full_survival`, `cross_dimension`, `mixed_multi_target`, `dead_target` and `parse_error_store`).
- **Kind**: `semantic`.
- **Confidence**: high.

---

### `CMD-GIVE-012` Success feedback and admin broadcast

The command reports one success message with the broadcast flag set.
For one target, the translation is `commands.give.success.single`.
Its arguments are the requested count, item text and target display name, in that order.
For more than one target, the translation is `commands.give.success.multiple`.
Its arguments are the requested count, item text and number of targets, in that order.

Delivery to the source and other operators follows CMDP-EXEC-004.
In particular, player feedback depends on `send_command_feedback`.
The target display name follows the shared text rules.

- **Provenance**: `black_box_confirmed` (`give-client.json`: `single-default`, `multiple-targets`, `broadcast`). Also `source_suggested` (inherited) ([commands-packet.md](commands-packet.md), `CMDP-EXEC-004`).
- **Kind**: `wire`, `client agreement`.
- **Confidence**: high for the captured cases; medium for inherited or general rules.

---

### `CMD-GIVE-013` `Item text` chat component format

Both success and count-limit messages use the item text. Its observable content is:

- The item name appears in brackets. A custom name or item name replaces the default name.
- A custom name has an italic parent style. An explicit style on the name can override it.
- For a non-air item, the effective rarity sets the bracket colour. It is white for common,
  yellow for uncommon, aqua for rare, and light purple for epic.
- When the player hovers over a non-air item, the client shows the item and its component patch.
  The item template and text form follow TXT-HOV-002 and `text-component.md`.
- The captured `air` messages show the air name without a colour or item hover event.

The capture records the text structure for each listed example. It does not require one
particular arrangement of text nodes when the same visible content and hover data result.

- **Provenance**: `black_box_confirmed` (`give-client.json`: `single-default`, `rarity-uncommon`, `rarity-rare`, `rarity-epic`, `custom-name`, `custom-name-styled`, `item-name`, `lore-mixed`).
  Also `black_box_confirmed` (`give-client.json`: `tooltip-hidden`, `removed-component`, `empty-brackets`, `modified-air`, `air-over-limit`, `toomanyitems-epic`).
  Also `source_suggested` (inherited) ([item-stack.md](item-stack.md), `STK-SHP-010..014`; [text-component.md](text-component.md), `TXT-HOV-001..003`, `TXT-NBT-002`).
- **Kind**: `semantic`.
- **Confidence**: high for the captured cases; medium for inherited or general rules.

---

## 5. Intentional Deviations

- **`D6` (Creative overflow, shared with [item-entity.md](item-entity.md))**: For a creative target with no room, vanilla destroys the items and shows a display item.
  Under `D6`, `mcrs` uses the uniform survival overflow rule so uninserted parts drop on the ground.
  See `Q6` in [item-entity.md](item-entity.md) for the interaction between `D6` and `IEN-PCK-006`.

---

## 6. Experiments and Conformance

The saved captures cover 25 client steps and 29 console cases. All passed on `26.4-snapshot-7`.
The experiment needs Java 25 and the verified server jar of that version.
Provision the jar with `tools/vanilla-server/run.sh fetch`. The runner uses a marked test world
under `target/vanilla-server/26.4-snapshot-7`. It refuses an existing unmarked world.
The runner clears the test players' inventories and removes item entities in that test world.
Run the experiment with:

```bash
MINECRAFT_EULA=true python3 tools/mcrs_minecraft_probe/give_record.py
```

Key files in the probe suite are:

- [give_record.py](../../tools/mcrs_minecraft_probe/give_record.py): isolated probe runner that produces `target/give-results.json` and `target/give-trace.jsonl`.
- [give_client.py](../../tools/mcrs_minecraft_probe/give_client.py): minimal `1073742169` client that captures `commands`, `system_chat`, and `sound` packets.
- [give_tree.py](../../tools/mcrs_minecraft_probe/give_tree.py): decoder for `commands` (`16`) trees recorded in the trace.

---

## 7. Unresolved

- **`U1` (Untested item and component pairs)**: Data-pack feature flags and rarer malformed component syntax errors outside `give-client.json` stay unprobed.
- **U2**: The captures do not decide the pitch range or distribution of the pickup sound.
- **U3**: The command form of `air` for other commands is open in `item-stack.md`, U3.
- **U4**: `item-entity.md`, Q6, asks whether creative pickup and `give` use one overflow policy.
  D6 and IEN-PCK-006 apply until the owner decides. A later pickup can destroy a dropped item.
- **U5**: Confirm which item and component suggestions need the server.
- **U6**: Confirm overflow for a dead target with a full inventory.
- **U7**: Confirm the statistic and advancement effects for items and cases outside the capture.
