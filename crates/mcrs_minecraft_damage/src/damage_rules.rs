use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_entity::damage_type_tag_keys;
use mcrs_minecraft_registries::{Registries, Tags};
use mcrs_minecraft_resource::TagKey;

use crate::damage_types;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageTypeFlags {
    pub bypasses_armor: bool,
    pub bypasses_resistance: bool,
    pub bypasses_invulnerability: bool,
    pub bypasses_effects: bool,
    pub bypasses_cooldown: bool,
    pub is_fire: bool,
    pub is_fall: bool,
    pub is_drowning: bool,
    pub is_freezing: bool,
    pub no_knockback: bool,
}

impl DamageTypeFlags {
    #[must_use]
    pub const fn builtin_for(damage_type: u32) -> Self {
        let mut flags = Self {
            bypasses_armor: false,
            bypasses_resistance: false,
            bypasses_invulnerability: false,
            bypasses_effects: false,
            bypasses_cooldown: false,
            is_fire: false,
            is_fall: false,
            is_drowning: false,
            is_freezing: false,
            no_knockback: false,
        };
        match damage_type {
            damage_types::OUT_OF_WORLD | damage_types::GENERIC_KILL => {
                flags.bypasses_armor = true;
                flags.bypasses_resistance = true;
                flags.bypasses_invulnerability = true;
                flags.bypasses_effects = true;
                flags.bypasses_cooldown = true;
            }
            damage_types::FALL => {
                flags.bypasses_armor = true;
                flags.is_fall = true;
                flags.no_knockback = true;
            }
            damage_types::DROWN => {
                flags.bypasses_armor = true;
                flags.is_drowning = true;
                flags.no_knockback = true;
            }
            damage_types::FREEZE => {
                flags.bypasses_armor = true;
                flags.is_freezing = true;
                flags.no_knockback = true;
            }
            damage_types::IN_FIRE
            | damage_types::ON_FIRE
            | damage_types::LAVA
            | damage_types::HOT_FLOOR
            | damage_types::CAMPFIRE
            | damage_types::FIREBALL
            | damage_types::UNATTRIBUTED_FIREBALL => {
                flags.is_fire = true;
            }
            damage_types::IN_WALL | damage_types::STARVE => {
                flags.bypasses_armor = true;
                flags.no_knockback = true;
            }
            damage_types::OUTSIDE_BORDER => {
                flags.bypasses_armor = true;
                flags.bypasses_cooldown = true;
                flags.no_knockback = true;
            }
            damage_types::GENERIC | damage_types::FLY_INTO_WALL => {
                flags.bypasses_armor = true;
            }
            _ => {}
        }
        flags
    }

    fn merge(&mut self, other: Self) {
        self.bypasses_armor |= other.bypasses_armor;
        self.bypasses_resistance |= other.bypasses_resistance;
        self.bypasses_invulnerability |= other.bypasses_invulnerability;
        self.bypasses_effects |= other.bypasses_effects;
        self.bypasses_cooldown |= other.bypasses_cooldown;
        self.is_fire |= other.is_fire;
        self.is_fall |= other.is_fall;
        self.is_drowning |= other.is_drowning;
        self.is_freezing |= other.is_freezing;
        self.no_knockback |= other.no_knockback;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageTypeRules {
    flags: Vec<DamageTypeFlags>,
    exhaustion: Vec<f32>,
}

impl DamageTypeRules {
    #[must_use]
    pub fn from_registries(registries: &Registries) -> Self {
        let Some(entries) = registries.entries::<DamageType>() else {
            return Self::builtin();
        };
        let mut rules = Self {
            flags: vec![DamageTypeFlags::default(); entries.len()],
            exhaustion: (0..entries.len())
                .map(|id| Self::builtin_exhaustion(id as u32))
                .collect(),
        };
        if let Some(registry) = registries.registry::<DamageType>() {
            for (at, value) in registry.values().iter().enumerate() {
                if let Some(exhaustion) = rules.exhaustion.get_mut(at) {
                    *exhaustion = value.exhaustion;
                }
            }
        }
        if let Some(tags) = registries.tags::<DamageType>() {
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_ARMOR, |flags| {
                flags.bypasses_armor = true;
            });
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_RESISTANCE, |flags| {
                flags.bypasses_resistance = true
            });
            rules.mark(
                &tags,
                damage_type_tag_keys::BYPASSES_INVULNERABILITY,
                |flags| flags.bypasses_invulnerability = true,
            );
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_EFFECTS, |flags| {
                flags.bypasses_effects = true
            });
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_COOLDOWN, |flags| {
                flags.bypasses_cooldown = true
            });
            rules.mark(&tags, damage_type_tag_keys::IS_FIRE, |flags| {
                flags.is_fire = true;
            });
            rules.mark(&tags, damage_type_tag_keys::IS_FALL, |flags| {
                flags.is_fall = true;
            });
            rules.mark(&tags, damage_type_tag_keys::IS_DROWNING, |flags| {
                flags.is_drowning = true;
            });
            rules.mark(&tags, damage_type_tag_keys::IS_FREEZING, |flags| {
                flags.is_freezing = true;
            });
            rules.mark(&tags, damage_type_tag_keys::NO_KNOCKBACK, |flags| {
                flags.no_knockback = true;
            });
        }
        for (id, flags) in rules.flags.iter_mut().enumerate() {
            flags.merge(DamageTypeFlags::builtin_for(id as u32));
        }
        for id in [damage_types::OUT_OF_WORLD, damage_types::OUTSIDE_BORDER] {
            if let Some(flags) = rules.flags.get_mut(id as usize) {
                flags.bypasses_cooldown = true;
            }
        }
        rules
    }

    #[must_use]
    pub fn builtin() -> Self {
        Self {
            flags: (0..64).map(DamageTypeFlags::builtin_for).collect(),
            exhaustion: (0..64).map(Self::builtin_exhaustion).collect(),
        }
    }

    #[must_use]
    pub const fn builtin_exhaustion(damage_type: u32) -> f32 {
        match damage_type {
            damage_types::GENERIC
            | damage_types::GENERIC_KILL
            | damage_types::OUT_OF_WORLD
            | damage_types::FALL
            | damage_types::STARVE
            | damage_types::DROWN
            | damage_types::FREEZE
            | damage_types::IN_WALL
            | damage_types::CRAMMING
            | damage_types::MAGIC
            | damage_types::INDIRECT_MAGIC
            | damage_types::WITHER => 0.0,
            _ => 0.1,
        }
    }

    #[must_use]
    pub fn flags(&self, damage_type: u32) -> DamageTypeFlags {
        self.flags
            .get(damage_type as usize)
            .copied()
            .unwrap_or_else(|| DamageTypeFlags::builtin_for(damage_type))
    }

    #[must_use]
    pub fn exhaustion(&self, damage_type: u32) -> f32 {
        self.exhaustion
            .get(damage_type as usize)
            .copied()
            .unwrap_or_else(|| Self::builtin_exhaustion(damage_type))
    }

    fn mark(
        &mut self,
        tags: &Tags<DamageType>,
        key: TagKey<'_, DamageType>,
        mark: impl Fn(&mut DamageTypeFlags),
    ) {
        if let Some(tag) = tags.id(key) {
            for id in tags.members(tag) {
                if let Some(flags) = self.flags.get_mut(id.index()) {
                    mark(flags);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damage_types;

    #[test]
    fn the_builtin_rules_cover_environmental_damage() {
        let rules = DamageTypeRules::builtin();
        assert!(rules.flags(damage_types::FALL).is_fall);
        assert!(rules.flags(damage_types::FALL).bypasses_armor);
        assert!(rules.flags(damage_types::DROWN).is_drowning);
        assert!(rules.flags(damage_types::FREEZE).is_freezing);
        assert!(rules.flags(damage_types::IN_FIRE).is_fire);
        assert!(rules.flags(damage_types::LAVA).is_fire);
        assert!(
            rules
                .flags(damage_types::OUT_OF_WORLD)
                .bypasses_invulnerability
        );
        assert!(rules.flags(damage_types::OUT_OF_WORLD).bypasses_cooldown);
        assert!(!rules.flags(damage_types::PLAYER_ATTACK).is_fire);
    }

    #[test]
    fn the_builtin_exhaustion_follows_the_damage_type() {
        let rules = DamageTypeRules::builtin();
        assert_eq!(rules.exhaustion(damage_types::PLAYER_ATTACK), 0.1);
        assert_eq!(rules.exhaustion(damage_types::FALL), 0.0);
        assert_eq!(rules.exhaustion(damage_types::STARVE), 0.0);
        assert_eq!(rules.exhaustion(10_000), 0.1);
    }
}
