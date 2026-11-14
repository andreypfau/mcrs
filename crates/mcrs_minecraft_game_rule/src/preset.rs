//! World rule presets (`java`, `bedrock`) and typed unified game rule access per Issue #669.
//!
//! A preset is a named set of game rule values. It is not a mode:
//! applying a preset writes the values; a later change of one rule keeps the others.
//! The primary edition of the edition policy (#655) selects the preset that a new world starts with.

use mcrs_minecraft_resource::ResourceLocation;

use crate::{GameRule, GameRuleDefinition, GameRuleValueType, McrsGameRule};

/// Unified reference to either a standard vanilla `minecraft:` game rule or an `mcrs:` custom game rule.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum AnyGameRule {
    /// Vanilla Minecraft rule in `minecraft:` namespace.
    Vanilla(GameRule),
    /// Custom MCRS rule in `mcrs:` namespace for deliberate edition differences.
    Mcrs(McrsGameRule),
}

impl From<GameRule> for AnyGameRule {
    #[inline]
    fn from(rule: GameRule) -> Self {
        Self::Vanilla(rule)
    }
}

impl From<McrsGameRule> for AnyGameRule {
    #[inline]
    fn from(rule: McrsGameRule) -> Self {
        Self::Mcrs(rule)
    }
}

impl AnyGameRule {
    /// The namespaced resource location of this game rule.
    #[must_use]
    pub const fn location(self) -> ResourceLocation<'static> {
        match self {
            Self::Vanilla(r) => r.location(),
            Self::Mcrs(r) => r.location(),
        }
    }

    /// The definition of this rule (category, value type, bounds, features).
    #[must_use]
    pub const fn definition(self) -> &'static GameRuleDefinition {
        match self {
            Self::Vanilla(r) => r.definition(),
            Self::Mcrs(r) => r.definition(),
        }
    }

    /// Parses an `AnyGameRule` from a namespaced resource location.
    #[must_use]
    pub fn from_location(location: &ResourceLocation<'_>) -> Option<Self> {
        if location.namespace() == "minecraft" {
            GameRule::get(*location).map(Self::Vanilla)
        } else if location.namespace() == "mcrs" {
            McrsGameRule::get(*location).map(Self::Mcrs)
        } else {
            None
        }
    }
}

/// A named world rule preset selecting canonical defaults for an edition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GameRulePreset {
    /// Canonical Java Edition world defaults.
    #[default]
    Java,
    /// Canonical Bedrock Edition world defaults.
    Bedrock,
}

impl GameRulePreset {
    /// The string identifier of this preset (`"java"` or `"bedrock"`).
    #[inline]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Java => "java",
            Self::Bedrock => "bedrock",
        }
    }

    /// Parses a preset from its name. Case-insensitive.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("java") {
            Some(Self::Java)
        } else if s.eq_ignore_ascii_case("bedrock") {
            Some(Self::Bedrock)
        } else {
            None
        }
    }

    /// The value of a vanilla game rule under this preset.
    #[must_use]
    pub const fn value_of_vanilla(self, rule: GameRule) -> i32 {
        match self {
            Self::Java => match rule.definition().value {
                GameRuleValueType::Bool { default } => default as i32,
                GameRuleValueType::Int { default, .. } => default,
            },
            Self::Bedrock => match rule {
                // Bedrock default random tick speed is 1 (Java is 3)
                GameRule::RandomTickSpeed => 1,
                // Bedrock default spawn radius is 5 (Java is 10)
                GameRule::RespawnRadius => 5,
                // Bedrock players enter nether immediately in survival by default
                GameRule::PlayersNetherPortalDefaultDelay => 0,
                // Bedrock snow layers stack up to full height 8 by default
                GameRule::MaxSnowAccumulationHeight => 8,
                // For all other rules, defaults coincide
                _ => match rule.definition().value {
                    GameRuleValueType::Bool { default } => default as i32,
                    GameRuleValueType::Int { default, .. } => default,
                },
            },
        }
    }

    /// The value of an `mcrs:` game rule under this preset.
    #[must_use]
    pub const fn value_of_mcrs(self, rule: McrsGameRule) -> i32 {
        match self {
            Self::Java => match rule.definition().value {
                GameRuleValueType::Bool { default } => default as i32,
                GameRuleValueType::Int { default, .. } => default,
            },
            Self::Bedrock => match rule {
                // Redstone: Bedrock has no quasi-connectivity
                McrsGameRule::QuasiConnectivity => 0,
                // Redstone: sticky pistons never drop blocks on 1-tick pulse
                McrsGameRule::StickyPiston1Tick => 0,
                // Redstone: pistons push tile/block entities (chests, furnaces, hoppers)
                McrsGameRule::PistonPushBlockEntities => 1,
                // Redstone: synchronized tick timing
                McrsGameRule::RedstoneTiming => 1,
                // Combat: 1.8-style combat without attack charge scaling
                McrsGameRule::AttackChargeDamageScaling => 0,
                // Hunger: constant regeneration without saturation boost
                McrsGameRule::SaturationBoost => 0,
                // Hunger: damage does not cause exhaustion
                McrsGameRule::ExhaustionFromDamage => 0,
                // Hunger: players can eat food in Peaceful mode even at full hunger
                McrsGameRule::EatingInPeaceful => 1,
                // Damage: void damage begins 40 blocks below minimum world Y (-104 in standard worlds)
                McrsGameRule::VoidDepth => -104,
                // Damage: creative players are invulnerable to the void in Bedrock
                McrsGameRule::CreativeVoidDamage => 0,
                // Damage: creative players are invulnerable to /kill in Bedrock
                McrsGameRule::CreativeKillDamage => 0,
                // Animals: breeding reduction stays standard
                McrsGameRule::BabyGrowthPerFeeding => 10,
                // Animals: passenger can steer vehicles/boats in Bedrock
                McrsGameRule::MobPassengerControl => 1,
                // Workstations: linear capped anvil repair cost formula
                McrsGameRule::AnvilCostFormula => 1,
                // Explorer maps: locates nearest structure including already explored ones
                McrsGameRule::ExplorerMapPolicy => 1,
                // Snow: layers fall with gravity in Bedrock
                McrsGameRule::SnowLayerGravity => 1,
                // Snow: layers melt gradually one by one in Bedrock
                McrsGameRule::SnowLayerMelting => 1,
            },
        }
    }

    /// Returns the value of any game rule under this preset.
    #[inline]
    #[must_use]
    pub const fn value_of(self, rule: AnyGameRule) -> i32 {
        match rule {
            AnyGameRule::Vanilla(r) => self.value_of_vanilla(r),
            AnyGameRule::Mcrs(r) => self.value_of_mcrs(r),
        }
    }
}

/// In-memory storage for active game rule values across both `minecraft:` and `mcrs:` namespaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRuleValues {
    vanilla: [i32; GameRule::ALL.len()],
    mcrs: [i32; McrsGameRule::ALL.len()],
}

impl Default for GameRuleValues {
    #[inline]
    fn default() -> Self {
        Self::from_preset(GameRulePreset::default())
    }
}

impl GameRuleValues {
    /// Creates a game rule map initialized with the values of a given preset.
    #[must_use]
    pub fn from_preset(preset: GameRulePreset) -> Self {
        let mut vanilla = [0i32; GameRule::ALL.len()];
        let mut mcrs = [0i32; McrsGameRule::ALL.len()];

        for (slot, &rule) in vanilla.iter_mut().zip(GameRule::ALL) {
            *slot = preset.value_of_vanilla(rule);
        }

        for (slot, &rule) in mcrs.iter_mut().zip(McrsGameRule::ALL) {
            *slot = preset.value_of_mcrs(rule);
        }

        Self { vanilla, mcrs }
    }

    /// Overwrites the current game rule values with the specified preset.
    pub fn apply_preset(&mut self, preset: GameRulePreset) {
        for (slot, &rule) in self.vanilla.iter_mut().zip(GameRule::ALL) {
            *slot = preset.value_of_vanilla(rule);
        }
        for (slot, &rule) in self.mcrs.iter_mut().zip(McrsGameRule::ALL) {
            *slot = preset.value_of_mcrs(rule);
        }
    }

    /// Returns the integer value of a vanilla game rule.
    #[inline]
    #[must_use]
    pub fn get_vanilla(&self, rule: GameRule) -> i32 {
        let idx = rule.id().number() as usize;
        self.vanilla.get(idx).copied().unwrap_or_default()
    }

    /// Sets the value of a vanilla game rule.
    #[inline]
    pub fn set_vanilla(&mut self, rule: GameRule, value: i32) {
        let idx = rule.id().number() as usize;
        if let Some(slot) = self.vanilla.get_mut(idx) {
            *slot = value;
        }
    }

    /// Returns the integer value of an `mcrs:` custom game rule.
    #[inline]
    #[must_use]
    pub fn get_mcrs(&self, rule: McrsGameRule) -> i32 {
        let idx = rule.id().number() as usize;
        self.mcrs.get(idx).copied().unwrap_or_default()
    }

    /// Sets the value of an `mcrs:` custom game rule.
    #[inline]
    pub fn set_mcrs(&mut self, rule: McrsGameRule, value: i32) {
        let idx = rule.id().number() as usize;
        if let Some(slot) = self.mcrs.get_mut(idx) {
            *slot = value;
        }
    }

    /// Returns the value of any game rule.
    #[inline]
    #[must_use]
    pub fn get(&self, rule: AnyGameRule) -> i32 {
        match rule {
            AnyGameRule::Vanilla(r) => self.get_vanilla(r),
            AnyGameRule::Mcrs(r) => self.get_mcrs(r),
        }
    }

    /// Sets the value of any game rule.
    #[inline]
    pub fn set(&mut self, rule: AnyGameRule, value: i32) {
        match rule {
            AnyGameRule::Vanilla(r) => self.set_vanilla(r, value),
            AnyGameRule::Mcrs(r) => self.set_mcrs(r, value),
        }
    }

    /// Returns whether a boolean game rule is enabled (non-zero).
    #[inline]
    #[must_use]
    pub fn is_on(&self, rule: AnyGameRule) -> bool {
        self.get(rule) != 0
    }
}
