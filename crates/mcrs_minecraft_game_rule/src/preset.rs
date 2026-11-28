use mcrs_minecraft_resource::ResourceLocation;

use crate::{GameRule, GameRuleDefinition, GameRuleValueType, InvalidGameRuleValue, McrsGameRule};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum AnyGameRule {
    Vanilla(GameRule),
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
    #[must_use]
    pub const fn location(self) -> ResourceLocation<'static> {
        match self {
            Self::Vanilla(r) => r.location(),
            Self::Mcrs(r) => r.location(),
        }
    }

    #[must_use]
    pub const fn definition(self) -> &'static GameRuleDefinition {
        match self {
            Self::Vanilla(r) => r.definition(),
            Self::Mcrs(r) => r.definition(),
        }
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GameRulePreset {
    #[default]
    Java,
    Bedrock,
}

impl GameRulePreset {
    #[inline]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Java => "java",
            Self::Bedrock => "bedrock",
        }
    }

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

    #[must_use]
    pub const fn value_of_vanilla(self, rule: GameRule) -> i32 {
        match self {
            Self::Java => match rule.definition().value {
                GameRuleValueType::Bool { default } => default as i32,
                GameRuleValueType::Int { default, .. } => default,
            },
            Self::Bedrock => match rule {
                GameRule::RandomTickSpeed => 1,
                GameRule::RespawnRadius => 5,
                GameRule::PlayersNetherPortalDefaultDelay => 0,
                GameRule::MaxSnowAccumulationHeight => 8,
                _ => match rule.definition().value {
                    GameRuleValueType::Bool { default } => default as i32,
                    GameRuleValueType::Int { default, .. } => default,
                },
            },
        }
    }

    #[must_use]
    pub const fn value_of_mcrs(self, rule: McrsGameRule) -> i32 {
        match self {
            Self::Java => match rule.definition().value {
                GameRuleValueType::Bool { default } => default as i32,
                GameRuleValueType::Int { default, .. } => default,
            },
            Self::Bedrock => match rule {
                McrsGameRule::QuasiConnectivity => 0,
                McrsGameRule::StickyPiston1Tick => 0,
                McrsGameRule::PistonPushBlockEntities => 1,
                McrsGameRule::RedstoneTiming => 1,
                McrsGameRule::AttackChargeDamageScaling => 0,
                McrsGameRule::SaturationBoost => 0,
                McrsGameRule::ExhaustionFromDamage => 0,
                McrsGameRule::EatingInPeaceful => 1,
                McrsGameRule::VoidDepth => -104,
                McrsGameRule::CreativeVoidDamage => 0,
                McrsGameRule::CreativeKillDamage => 0,
                McrsGameRule::BabyGrowthPerFeeding => 10,
                McrsGameRule::MobPassengerControl => 1,
                McrsGameRule::AnvilCostFormula => 1,
                McrsGameRule::ExplorerMapPolicy => 1,
                McrsGameRule::SnowLayerGravity => 1,
                McrsGameRule::SnowLayerMelting => 1,
            },
        }
    }

    #[inline]
    #[must_use]
    pub const fn value_of(self, rule: AnyGameRule) -> i32 {
        match rule {
            AnyGameRule::Vanilla(r) => self.value_of_vanilla(r),
            AnyGameRule::Mcrs(r) => self.value_of_mcrs(r),
        }
    }
}

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

    pub fn apply_preset(&mut self, preset: GameRulePreset) {
        for (slot, &rule) in self.vanilla.iter_mut().zip(GameRule::ALL) {
            *slot = preset.value_of_vanilla(rule);
        }
        for (slot, &rule) in self.mcrs.iter_mut().zip(McrsGameRule::ALL) {
            *slot = preset.value_of_mcrs(rule);
        }
    }

    #[inline]
    #[must_use]
    pub fn get_vanilla(&self, rule: GameRule) -> i32 {
        let idx = rule.id().number() as usize;
        self.vanilla.get(idx).copied().unwrap_or_default()
    }

    #[inline]
    pub fn set_vanilla(&mut self, rule: GameRule, value: i32) -> Result<(), InvalidGameRuleValue> {
        if !rule.definition().value.accepts(value) {
            return Err(InvalidGameRuleValue {
                rule: rule.into(),
                value,
            });
        }
        let idx = rule.id().number() as usize;
        if let Some(slot) = self.vanilla.get_mut(idx) {
            *slot = value;
        }
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn get_mcrs(&self, rule: McrsGameRule) -> i32 {
        let idx = rule.id().number() as usize;
        self.mcrs.get(idx).copied().unwrap_or_default()
    }

    #[inline]
    pub fn set_mcrs(&mut self, rule: McrsGameRule, value: i32) -> Result<(), InvalidGameRuleValue> {
        if !rule.definition().value.accepts(value) {
            return Err(InvalidGameRuleValue {
                rule: rule.into(),
                value,
            });
        }
        let idx = rule.id().number() as usize;
        if let Some(slot) = self.mcrs.get_mut(idx) {
            *slot = value;
        }
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn get(&self, rule: AnyGameRule) -> i32 {
        match rule {
            AnyGameRule::Vanilla(r) => self.get_vanilla(r),
            AnyGameRule::Mcrs(r) => self.get_mcrs(r),
        }
    }

    #[inline]
    pub fn set(&mut self, rule: AnyGameRule, value: i32) -> Result<(), InvalidGameRuleValue> {
        match rule {
            AnyGameRule::Vanilla(r) => self.set_vanilla(r, value),
            AnyGameRule::Mcrs(r) => self.set_mcrs(r, value),
        }
    }

    #[inline]
    #[must_use]
    pub fn is_on(&self, rule: AnyGameRule) -> bool {
        self.get(rule) != 0
    }
}
