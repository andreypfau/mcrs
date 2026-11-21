//! Custom MCRS-namespaced game rules per Issue #669.
//!
//! Provides typed game rules for deliberate differences between Java Edition and Bedrock Edition.
//! Each rule cites its parity list entry and defaults to the canonical Java behavior.

use crate::{GameRuleCategory, GameRuleDefinition, GameRuleValueType};
use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

mcrs_minecraft_registries::static_registry! {
    pub enum McrsGameRule;
    QuasiConnectivity = "mcrs:quasi_connectivity",
    StickyPiston1Tick = "mcrs:sticky_piston_1_tick",
    PistonPushBlockEntities = "mcrs:piston_push_block_entities",
    RedstoneTiming = "mcrs:redstone_timing",
    AttackChargeDamageScaling = "mcrs:attack_charge_damage_scaling",
    SaturationBoost = "mcrs:saturation_boost",
    ExhaustionFromDamage = "mcrs:exhaustion_from_damage",
    EatingInPeaceful = "mcrs:eating_in_peaceful",
    VoidDepth = "mcrs:void_depth",
    CreativeVoidDamage = "mcrs:creative_void_damage",
    CreativeKillDamage = "mcrs:creative_kill_damage",
    BabyGrowthPerFeeding = "mcrs:baby_growth_per_feeding",
    MobPassengerControl = "mcrs:mob_passenger_control",
    ExplorerMapPolicy = "mcrs:explorer_map_policy",
    AnvilCostFormula = "mcrs:anvil_cost_formula",
    SnowLayerGravity = "mcrs:snow_layer_gravity",
    SnowLayerMelting = "mcrs:snow_layer_melting",
}

pub const MCRS_GAME_RULE: RegistryKey<McrsGameRule> = RegistryKey::new(rl!("mcrs:game_rule"));

impl Registered for McrsGameRule {
    const REGISTRY: RegistryKey<Self> = MCRS_GAME_RULE;
}

impl McrsGameRule {
    /// Returns the category, value type, and requirements for this game rule.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per custom game rule"
    )]
    #[must_use]
    pub const fn definition(self) -> &'static GameRuleDefinition {
        match self {
            Self::QuasiConnectivity => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::StickyPiston1Tick => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::PistonPushBlockEntities => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::RedstoneTiming => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Int {
                    default: 0,
                    min: 0,
                    max: 1,
                },
                required_features: &[],
            },
            Self::AttackChargeDamageScaling => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::SaturationBoost => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::ExhaustionFromDamage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::EatingInPeaceful => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::VoidDepth => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Int {
                    default: -64,
                    min: -2048,
                    max: 2048,
                },
                required_features: &[],
            },
            Self::CreativeVoidDamage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::CreativeKillDamage => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Bool { default: true },
                required_features: &[],
            },
            Self::BabyGrowthPerFeeding => &GameRuleDefinition {
                category: GameRuleCategory::Mobs,
                value: GameRuleValueType::Int {
                    default: 10,
                    min: 0,
                    max: 100,
                },
                required_features: &[],
            },
            Self::MobPassengerControl => &GameRuleDefinition {
                category: GameRuleCategory::Mobs,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::ExplorerMapPolicy => &GameRuleDefinition {
                category: GameRuleCategory::Misc,
                value: GameRuleValueType::Int {
                    default: 0,
                    min: 0,
                    max: 1,
                },
                required_features: &[],
            },
            Self::AnvilCostFormula => &GameRuleDefinition {
                category: GameRuleCategory::Player,
                value: GameRuleValueType::Int {
                    default: 0,
                    min: 0,
                    max: 1,
                },
                required_features: &[],
            },
            Self::SnowLayerGravity => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
            Self::SnowLayerMelting => &GameRuleDefinition {
                category: GameRuleCategory::Updates,
                value: GameRuleValueType::Bool { default: false },
                required_features: &[],
            },
        }
    }

    /// Cites the specification, parity list entry, or reference issue for this deliberate edition difference.
    #[must_use]
    pub const fn parity_citation(self) -> &'static str {
        match self {
            Self::QuasiConnectivity => "PARITY-REDSTONE-QC: Java quasi-connectivity powered through air block above (#184)",
            Self::StickyPiston1Tick => "PARITY-REDSTONE-STICKY-DROP: 1-tick pulse causes sticky pistons to spit block (#190)",
            Self::PistonPushBlockEntities => "PARITY-PISTON-BLOCK-ENTITIES: immovable by pistons in Java, movable in Bedrock (#195)",
            Self::RedstoneTiming => "PARITY-REDSTONE-TIMING: 0=instantaneous edge repeaters (Java), 1=synchronized ticks (Bedrock) (#460)",
            Self::AttackChargeDamageScaling => "PARITY-COMBAT-ATTACK-COOLDOWN: 1.9+ weapon charge cooldown scaling (#121)",
            Self::SaturationBoost => "PARITY-HUNGER-SATURATION-BOOST: fast health regeneration while saturation > 0 (#261)",
            Self::ExhaustionFromDamage => "PARITY-HUNGER-DAMAGE-EXHAUSTION: damage increases exhaustion in Java (#262)",
            Self::EatingInPeaceful => "PARITY-HUNGER-PEACEFUL-EAT: allowed in Bedrock even with full hunger (#234)",
            Self::VoidDepth => "PARITY-DAMAGE-VOID-DEPTH: void damage Y start (-64 Java, -104 Bedrock) (#241)",
            Self::CreativeVoidDamage => "PARITY-DAMAGE-CREATIVE-VOID: creative players damaged by void in Java, immune in Bedrock (#212)",
            Self::CreativeKillDamage => "PARITY-DAMAGE-CREATIVE-KILL: /kill command removes creative players (#212)",
            Self::BabyGrowthPerFeeding => "PARITY-ANIMALS-FEED-GROWTH: percentage age reduction per breeding feed (#225)",
            Self::MobPassengerControl => "PARITY-ANIMALS-PASSENGER-STEER: passenger steering in boats/vehicles (#228)",
            Self::ExplorerMapPolicy => "PARITY-EXPLORER-MAP-SEARCH: 0=unexplored only (Java), 1=nearest any (Bedrock) (#100)",
            Self::AnvilCostFormula => "PARITY-WORKSTATION-ANVIL-COST: 0=exponential prior work penalty capped at 40 (Java), 1=linear capped (Bedrock) (#188)",
            Self::SnowLayerGravity => "PARITY-SNOW-GRAVITY: snow layers fall with gravity in Bedrock (#184)",
            Self::SnowLayerMelting => "PARITY-SNOW-GRADUAL-MELTING: gradual layer melting in Bedrock vs complete removal in Java (#184)",
        }
    }
}
