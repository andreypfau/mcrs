# Arm swing replication: design

Planned design note for server-side arm-swing state, cooldowns, item animations, and the `/swing` command.
At this revision, only the `mcrs_minecraft_protocol_game` wire codecs and the probe harness exist; the runtime crates, Bevy types, and systems in sections 1 to 4 are proposed for follow-up PRs.
Behaviour comes from `docs/spec/arm-swing.md` (SWG), `docs/spec/block-placing.md` (PLC), and
`docs/spec/command-swing.md` (CMD-SWG).

## 1. Classification of the state (planned)

| Value | Kind | Owner |
|-------|------|-------|
| `PlayerSwing(SwingTracker)` on a player | authoritative truth (`started_at`, `scaled_duration`) | `apply_player_swings` and `swing::swing_player` in `mcrs_minecraft_engine` |
| `LoadedClient` on a player | authoritative truth (`loaded`, `ticks_since_spawn`) | `on_player_loaded` and `tick_loaded_clients` in `mcrs_minecraft_engine`; preserved across cell crossings, reset on dimension change |
| `MobSwingStates` (`EntityId -> SwingTracker`) | authoritative truth per living mob row | `swing_mob` and `end_mob_changes` in `mcrs_minecraft_engine_physics` |
| `SwingPlayer` message | temporal fact (request to start a player swing) | emitted by gameplay plugins (`vanilla_swing`, `vanilla_block_place`, `vanilla_inventory`) or user plugins; consumed by `apply_player_swings` |
| `PendingPlayerSwings` / `MobChanges::swings` | per-tick replication outbox | drained into `TileEvents` once per tick; `EntityCodec` encodes one `ClientboundSwingAnimation` `Bytes` buffer and shares it across observers |

## 2. Crate map

| Crate | Layer | Content |
|-------|-------|---------|
| `mcrs_minecraft_swing` (`no_std`, planned) | 3, rules | `SwingHand`, `SwingKind`, `SwingSpec`, `SwingDelivery`, `SpectatorRule`, `SwingModifiers`, `ActiveSwing`, `SwingTracker`, `LoadedClient`, `EffectStep` |
| `mcrs_minecraft_item_stack` (`no_std`, planned) | 3, data | `SwingAnimation`, `AttackAnimation`, `InteractAnimation`, `PiercingWeapon` |
| `mcrs_minecraft_protocol_game` | 3, data | `ClientboundSwingAnimation`, `ClientboundAnimate` |
| `mcrs_minecraft_replication` (planned swing variants) | 3, data | `PlayerData::Swing` and `MobSignal::Swing` encoded once and delivered to tracking observers without backlog replay |
| `mcrs_minecraft_engine` (planned swing APIs) | 4 | `PlayerSwingPlugin`, `SwingPlayer` message, `PlayerOrder::Swing`, `MuxerStage` ordering (`views_out` -> `Entities` -> `changes_out`) |
| `mcrs_minecraft_engine_physics`, `mcrs_minecraft_engine_mob_replication` (planned swing APIs) | 4 | `MobSwingStates`, `swing_mob`, `publish_swings` |
| `mcrs_minecraft_vanilla_swing` (planned); `mcrs_minecraft_vanilla_block_place`, `mcrs_minecraft_vanilla_inventory`, `mcrs_minecraft_command_vanilla` (planned swing additions) | 5 | `SwingPlugin` (`punch` and `stab`), placement swing (`PLC-PLA-007`), hand-drop swing, `/swing` command (`SwingAnimationArgument`, `SwingEntity`) |
| `mcrs_minecraft_server` | 7 | `VanillaSwingPlugin` (planned) in `MinecraftServerPlugins` |

Reason for `mcrs_minecraft_swing`: bots, proxies, item stacks, command arguments, and dimension
plugins share the swing types, cooldown arithmetic, and effect scaling without taking Bevy, tokio,
or engine registries.

## 3. Use cases

| User | Takes | Does not take |
|------|-------|---------------|
| proxy, replay viewer | `mcrs_minecraft_protocol_game`, `mcrs_minecraft_swing` (planned) | physics, Bevy, server engine |
| minigame server (planned) | `mcrs_minecraft_engine`; writes `SwingPlayer` directly or replaces `VanillaSwingPlugin` | vanilla `punch`/`stab` rules |
| headless bot | `mcrs_minecraft_swing` (planned) cooldown and `SwingModifiers` math | rendering, server engine |

## 4. Server tick (planned)

1. `TickSet::ClientInput`: `place_blocks` reads `InteractAnimation` before consuming the held item and writes `SwingPlayer`.
2. `TickSet::Simulate`: `tick_loaded_clients` advances unloaded players toward the 60-tick fallback; `SwingSimulateSet` (`process_swing_inputs`) handles `ServerboundPunch` and `PlayerAction::Stab` (emitting any stab `sound` packet via `OutboundPlayerPacket` when a stab starts); `swing_on_hand_drop` runs after `SwingSimulateSet`.
3. `TickSet::Project`: `apply_player_swings` (gated on `on_message::<SwingPlayer>`) checks `SpectatorRule`, `BirthClock` cooldown (`SWG-STA-005`, `SWG-STA-004`), and status-effect scaling (`SWG-STA-002`), then queues outbound packets before `tick_player_effects_system` decays active effects.
4. `FixedPostUpdate`, `dim_outputs::receive`, and `MuxerStage`: `publish_player_swings` and `publish_swings` emit `TileEvents`; `dim_outputs::receive` flushes direct dimension `OutboundPlayerPacket`s (including stab `sound` from `SwingSimulateSet`) before queuing `MuxerCall::Entities`; then `MuxerStage` flushes view packets, entity packets (`ClientboundSwingAnimation`), and block updates (`BlockUpdate`) in order within each `Send` step (`SWG-DST-003`).
