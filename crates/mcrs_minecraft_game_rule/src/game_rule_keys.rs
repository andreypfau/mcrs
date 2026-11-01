// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registries::static_registry! {
    pub enum GameRule;
    AdvanceTime = "minecraft:advance_time",
    AdvanceWeather = "minecraft:advance_weather",
    AllowEnteringNetherUsingPortals = "minecraft:allow_entering_nether_using_portals",
    BlockDrops = "minecraft:block_drops",
    BlockExplosionDropDecay = "minecraft:block_explosion_drop_decay",
    CommandBlocksWork = "minecraft:command_blocks_work",
    CommandBlockOutput = "minecraft:command_block_output",
    DrowningDamage = "minecraft:drowning_damage",
    ElytraMovementCheck = "minecraft:elytra_movement_check",
    EnderPearlsVanishOnDeath = "minecraft:ender_pearls_vanish_on_death",
    EntityDrops = "minecraft:entity_drops",
    FallDamage = "minecraft:fall_damage",
    FireDamage = "minecraft:fire_damage",
    FireSpreadRadiusAroundPlayer = "minecraft:fire_spread_radius_around_player",
    ForgiveDeadPlayers = "minecraft:forgive_dead_players",
    FreezeDamage = "minecraft:freeze_damage",
    GlobalSoundEvents = "minecraft:global_sound_events",
    ImmediateRespawn = "minecraft:immediate_respawn",
    KeepInventory = "minecraft:keep_inventory",
    LavaSourceConversion = "minecraft:lava_source_conversion",
    LimitedCrafting = "minecraft:limited_crafting",
    LocatorBar = "minecraft:locator_bar",
    LogAdminCommands = "minecraft:log_admin_commands",
    MaxBlockModifications = "minecraft:max_block_modifications",
    MaxCommandForks = "minecraft:max_command_forks",
    MaxCommandSequenceLength = "minecraft:max_command_sequence_length",
    MaxEntityCramming = "minecraft:max_entity_cramming",
    MaxMinecartSpeed = "minecraft:max_minecart_speed",
    MaxSnowAccumulationHeight = "minecraft:max_snow_accumulation_height",
    MobDrops = "minecraft:mob_drops",
    MobExplosionDropDecay = "minecraft:mob_explosion_drop_decay",
    MobGriefing = "minecraft:mob_griefing",
    NaturalHealthRegeneration = "minecraft:natural_health_regeneration",
    PlayerMovementCheck = "minecraft:player_movement_check",
    PlayersNetherPortalCreativeDelay = "minecraft:players_nether_portal_creative_delay",
    PlayersNetherPortalDefaultDelay = "minecraft:players_nether_portal_default_delay",
    PlayersSleepingPercentage = "minecraft:players_sleeping_percentage",
    ProjectilesCanBreakBlocks = "minecraft:projectiles_can_break_blocks",
    Pvp = "minecraft:pvp",
    Raids = "minecraft:raids",
    RandomTickSpeed = "minecraft:random_tick_speed",
    ReducedDebugInfo = "minecraft:reduced_debug_info",
    RespawnRadius = "minecraft:respawn_radius",
    SendCommandFeedback = "minecraft:send_command_feedback",
    ShowAdvancementMessages = "minecraft:show_advancement_messages",
    ShowDeathMessages = "minecraft:show_death_messages",
    SpawnerBlocksWork = "minecraft:spawner_blocks_work",
    SpawnMobs = "minecraft:spawn_mobs",
    SpawnMonsters = "minecraft:spawn_monsters",
    SpawnPatrols = "minecraft:spawn_patrols",
    SpawnPhantoms = "minecraft:spawn_phantoms",
    SpawnWanderingTraders = "minecraft:spawn_wandering_traders",
    SpawnWardens = "minecraft:spawn_wardens",
    SpectatorsGenerateChunks = "minecraft:spectators_generate_chunks",
    SpreadVines = "minecraft:spread_vines",
    TntExplodes = "minecraft:tnt_explodes",
    TntExplosionDropDecay = "minecraft:tnt_explosion_drop_decay",
    UniversalAnger = "minecraft:universal_anger",
    WaterSourceConversion = "minecraft:water_source_conversion",
}

use crate::{GameRuleCategory, GameRuleDefinition, GameRuleValueType};
use mcrs_minecraft_resource::rl;

impl GameRule {
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per game rule the game ships"
    )]
    pub const fn definition(self) -> &'static GameRuleDefinition {
        match self {
            Self::AdvanceTime => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::AdvanceWeather => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::AllowEnteringNetherUsingPortals => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::BlockDrops => &GameRuleDefinition {
                category: GameRuleCategory::Drops,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::BlockExplosionDropDecay => &GameRuleDefinition {
                category: GameRuleCategory::Drops,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::CommandBlockOutput => &GameRuleDefinition {
                category: GameRuleCategory::Chat,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::CommandBlocksWork => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::DrowningDamage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::ElytraMovementCheck => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::EnderPearlsVanishOnDeath => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::EntityDrops => &GameRuleDefinition {
                category: GameRuleCategory::Drops,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::FallDamage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::FireDamage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::FireSpreadRadiusAroundPlayer => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Int {
                    default: 128,
                    min: -1,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::ForgiveDeadPlayers => &GameRuleDefinition {
                category: GameRuleCategory::Mobs,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::FreezeDamage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::GlobalSoundEvents => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::ImmediateRespawn => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::KeepInventory => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::LavaSourceConversion => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::LimitedCrafting => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::LocatorBar => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::LogAdminCommands => &GameRuleDefinition {
                category: GameRuleCategory::Chat,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::MaxBlockModifications => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Int {
                    default: 32768,
                    min: 1,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::MaxCommandForks => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Int {
                    default: 65536,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::MaxCommandSequenceLength => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Int {
                    default: 65536,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::MaxEntityCramming => &GameRuleDefinition {
                category: GameRuleCategory::Mobs,
                value: GameRuleValueType::Int {
                    default: 24,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::MaxMinecartSpeed => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Int {
                    default: 8,
                    min: 1,
                    max: 1000,
                },
                required_features: &[rl!("minecraft:minecart_improvements")],
            },
            Self::MaxSnowAccumulationHeight => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Int {
                    default: 1,
                    min: 0,
                    max: 8,
                },
                required_features: &[],
            },
            Self::MobDrops => &GameRuleDefinition {
                category: GameRuleCategory::Drops,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::MobExplosionDropDecay => &GameRuleDefinition {
                category: GameRuleCategory::Drops,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::MobGriefing => &GameRuleDefinition {
                category: GameRuleCategory::Mobs,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::NaturalHealthRegeneration => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::PlayerMovementCheck => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::PlayersNetherPortalCreativeDelay => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Int {
                    default: 0,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::PlayersNetherPortalDefaultDelay => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Int {
                    default: 80,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::PlayersSleepingPercentage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Int {
                    default: 100,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::ProjectilesCanBreakBlocks => &GameRuleDefinition {
                category: GameRuleCategory::Drops,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::Pvp => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::Raids => &GameRuleDefinition {
                category: GameRuleCategory::Mobs,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::RandomTickSpeed => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Int {
                    default: 3,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::ReducedDebugInfo => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::RespawnRadius => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Int {
                    default: 10,
                    min: 0,
                    max: 2147483647,
                },
                required_features: &[],
            },
            Self::SendCommandFeedback => &GameRuleDefinition {
                category: GameRuleCategory::Chat,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::ShowAdvancementMessages => &GameRuleDefinition {
                category: GameRuleCategory::Chat,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::ShowDeathMessages => &GameRuleDefinition {
                category: GameRuleCategory::Chat,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpawnMobs => &GameRuleDefinition {
                category: GameRuleCategory::Spawning,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpawnMonsters => &GameRuleDefinition {
                category: GameRuleCategory::Spawning,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpawnPatrols => &GameRuleDefinition {
                category: GameRuleCategory::Spawning,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpawnPhantoms => &GameRuleDefinition {
                category: GameRuleCategory::Spawning,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpawnWanderingTraders => &GameRuleDefinition {
                category: GameRuleCategory::Spawning,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpawnWardens => &GameRuleDefinition {
                category: GameRuleCategory::Spawning,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpawnerBlocksWork => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpectatorsGenerateChunks => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SpreadVines => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::TntExplodes => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::TntExplosionDropDecay => &GameRuleDefinition {
                category: GameRuleCategory::Drops,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::UniversalAnger => &GameRuleDefinition {
                category: GameRuleCategory::Mobs,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::WaterSourceConversion => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
        }
    }
}
