# Capture procedure: `loot.bin`

The game writes this file. Nobody edits it by hand. The layout and the generator are in the
section "Loot dumps" of `tools/vanilla-oracle/README.md`.

## Capture

```sh
cargo run -p mcrs_minecraft_update -- recapture loot
```

The task loads the vanilla data pack as a server does and runs no server. Two runs write the same
bytes. A reader asserts `world_version` against `assets/minecraft/version.json`, reads every case
and ends at the last byte.

## The world of each case

- Every position is loaded and holds the biome of the case: `minecraft:plains`, except in the
  cases that name another biome.
- There is no structure anywhere. An exploration map finds no structure, and the game gives an
  empty stack for it. A roll records the empty stack as `minecraft:air` with count 0. A fill
  puts nothing in a slot for it.
- The origin is (0.5, 64.5, 0.5). The luck is 0. No case has an entity.

## Cases

The order in the file is the order of this list.

| Kind | Cases | Seeds | Context |
| - | - | - | - |
| fill, 27 slots | each table under `minecraft:chests/`, ids ascending | 42; for `ancient_city`, `buried_treasure`, `shipwreck_map`, `stronghold_library` and `village/village_weaponsmith`: 1, 42, 0x7FFF_FFFF_0000_0001 | `minecraft:chest`, origin |
| fill, 27 slots | `chests/abandoned_mineshaft` in `minecraft:sulfur_caves`; `chests/abandoned_camp_common_chest` in `minecraft:bamboo_jungle` and in `minecraft:cherry_grove` | 1, 42, 0x7FFF_FFFF_0000_0001 | the same |
| fill, 5 slots | `chests/end_city_treasure`: more stacks than slots | the same three | the same |
| roll, inline table | one item entry with one modifier: `set_count` (uniform, binomial, a count of 40 that the roll divides), `set_damage`, `enchant_randomly` (a book, a sword), `enchant_with_levels` (uniform, constant); a weighted pool with an empty entry and quality | 1, 42, -7 | `minecraft:chest`, origin |
| roll, inline table | `apply_bonus` with each formula, the tool has Fortune III | 1, 42, -7 | `minecraft:block`: origin, `minecraft:stone`, an iron pickaxe |
| roll, inline table | `explosion_decay` after `set_count` 8 | 1, 42, -7 | `minecraft:block` with the explosion radius 4 |
| condition, 16 tests | `random_chance`; `random_chance_with_enchanted_bonus` with no attacker; `table_bonus` with a plain tool and with Fortune III; `survives_explosion`; `inverted`, `all_of` and `any_of` over `random_chance` | 1, 42, -7 | as the condition needs |
| int provider, 16 samples | constant, `uniform`, `binomial`, `weighted_list`, `add` of two `uniform` | 1, 42, -7 | `minecraft:chest`, origin |
| float provider, 16 samples | constant, `uniform`, `weighted_list` | 1, 42, -7 | the same |

167 cases, 82 814 bytes.

## What the fixture cannot pin

- A context with an entity: mob loot, archaeology, gifts, equipment, the attacker of
  `random_chance_with_enchanted_bonus`.
- An exploration map that finds a structure.
- A roll from a random sequence (the seed 0).
- A luck other than 0.
- A table under `minecraft:blocks/` or a data pack other than the vanilla one.

A larger corpus for the exhaustive tier comes from the same task with more tables and seeds.
