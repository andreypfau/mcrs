# Arm swing replication: design

Design note for server-side arm-swing state, cooldowns, item animations, and the `/swing` command.
Behaviour comes from `docs/spec/arm-swing.md` (SWG), `docs/spec/block-placing.md` (PLC), and
`docs/spec/command-swing.md` (CMD-SWG).

## 1. Classification of the state

| Value | Kind | Owner |
|-------|------|-------|
| `PlayerSwing(SwingTracker)` on a player | authoritative truth (`started_at`, `scaled_duration`) | `apply_player_swings` in `mcrs_minecraft_engine`; preserved across cell crossings and dimension changes |
| `UnloadedClient(u8)` on a player | authoritative truth (transient `SparseSet` component removed once loaded) | `on_player_loaded` and `tick_loaded_clients` in `mcrs_minecraft_engine`; preserved across cell crossings, reset on dimension change |
| `MobSwing(SwingTracker)` on a `Mob` entity | authoritative truth (`started_at`, `scaled_duration`) | `swing_mob` in `mcrs_minecraft_engine_physics`; required on `Mob` |
| `SwingPlayer` message | temporal fact (request to start a player swing, with `SwingRecipients` and `SwingEligibility`) | emitted by gameplay plugins (`vanilla_swing`, `vanilla_block_place`, `vanilla_inventory`), `player_order::swing`, or user plugins; consumed by `apply_player_swings` |
| `PendingPlayerSwings` / `MobChanges::swings` | per-tick replication outbox | drained into `TileEvents` once per tick (`publish_player_swings` / `take_mob_news` + `publish_motion`); `EntityCodec` and `MobTrackers` encode one `ClientboundSwingAnimation` `Bytes` buffer and share it across recipients |

## 2. Crate map

| Crate | Layer | Content |
|-------|-------|---------|
| `mcrs_minecraft_swing` (`no_std`) | 3, rules | `SwingModifiers`, `SwingTracker` |
| `mcrs_minecraft_item_stack` | 3, data | `SwingKind`, `SwingAnimation`, `AttackAnimation`, `InteractAnimation`, `PiercingWeapon` |
| `mcrs_minecraft_protocol_game` | 3, data | `ClientboundSwingAnimation`, `ClientboundAnimate` |
| `mcrs_minecraft_replication` | 3, data | `SwingRecipients`, `PlayerData::Swing`, and `MobSignal::Swing` encoded once and delivered without backlog replay |
| `mcrs_minecraft_engine` | 4 | `LoadedClientPlugin`, `UnloadedClient`, `PlayerSwingPlugin`, `SwingPlayer`, `SwingEligibility`, `PlayerOrder::Swing`, `MuxerStage` ordering (`views_out` -> `Entities` -> `changes_out`) |
| `mcrs_minecraft_engine_physics`, `mcrs_minecraft_engine_entity_motion_replication` | 4 | `MobSwing`, `MobSwingSpec`, `swing_mob`, `MobChanges::swings`, `take_mob_news` and `publish_motion` swing forwarding |
| `mcrs_minecraft_vanilla_swing`, `mcrs_minecraft_vanilla_block_place`, `mcrs_minecraft_vanilla_inventory`; planned `mcrs_minecraft_command_argument` & `mcrs_minecraft_command_vanilla` swing additions | 5 | `SwingPlugin` (`punch` and `stab`), placement swing (`PLC-PLA-007`), hand-drop swing; planned `/swing` command (`SwingEntity`, with `SwingAnimationArgument` in layer 3 `mcrs_minecraft_command_argument`) |
| `mcrs_minecraft_server` | 7 | `VanillaSwingPlugin` in `MinecraftServerPlugins` |

Reason for `mcrs_minecraft_swing`: bots, proxies, and dimension plugins share the cooldown
arithmetic and effect scaling without taking Bevy, tokio, item codecs, or engine registries.

## 3. Use cases

| User | Takes | Does not take |
|------|-------|---------------|
| proxy, replay viewer | `mcrs_minecraft_protocol_game`, `mcrs_minecraft_swing` | physics, Bevy, server engine |
| minigame server | `mcrs_minecraft_engine`; writes `SwingPlayer` directly or replaces `VanillaSwingPlugin` | vanilla `punch`/`stab` rules |
| headless bot | `mcrs_minecraft_swing` cooldown and `SwingModifiers` math | rendering, server engine |

## 4. Server tick

1. `TickSet::ClientInput`: `place_blocks` reads `InteractAnimation` before consuming the held item and writes `SwingPlayer`.
2. `TickSet::Simulate`: `tick_loaded_clients` (`LoadedClientTickSet`) advances unloaded players toward the 60-tick fallback; `SwingSimulateSet` (`process_swing_inputs`) handles `ServerboundPunch` and `PlayerAction::Stab`; `swing_on_hand_drop` runs after `SwingSimulateSet`.
3. `TickSet::Project`: `apply_player_swings` (gated on `on_message::<SwingPlayer>`) checks `SwingEligibility`, `BirthClock` cooldown (`SWG-STA-003`, `SWG-STA-004`, `SWG-STA-005`), and status-effect scaling (`SWG-STA-002`), then queues `PendingPlayerSwings`; `take_mob_news` and `publish_motion` forward `MobChanges::swings` into `TileEvents`.
4. `FixedPostUpdate`, `dim_outputs::receive`, and `MuxerStage`: `publish_player_swings` emits player `TileEvents`; `MuxerStage` buffers `Dim` and `Entities` calls and flushes view packets, entity packets (`ClientboundSwingAnimation`), and block updates (`BlockUpdate`) in order at each `Send`/`Flush` batch boundary (`SWG-DST-003`).
