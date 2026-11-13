use mcrs_minecraft_entity::damage_type::{DamageScaling, DamageType};
use mcrs_minecraft_entity::damage_type_tag_keys;
use mcrs_minecraft_registries::{Registries, Tags};
use mcrs_minecraft_resource::{ResourceKey, ResourceLocation, TagKey};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageTypeFlags {
    pub bypasses_armor: bool,
    pub bypasses_resistance: bool,
    pub bypasses_invulnerability: bool,
    pub bypasses_effects: bool,
    pub bypasses_cooldown: bool,
    pub bypasses_enchantments: bool,
    pub is_fire: bool,
    pub is_fall: bool,
    pub is_drowning: bool,
    pub is_freezing: bool,
    pub no_impact: bool,
    pub no_knockback: bool,
    pub no_hurt_sound: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageTypeRules {
    names: Vec<mcrs_minecraft_resource::ResourceLocationBuf>,
    message_ids: Vec<String>,
    scalings: Vec<DamageScaling>,
    flags: Vec<DamageTypeFlags>,
    exhaustion: Vec<f32>,
}

impl DamageTypeRules {
    #[must_use]
    pub fn from_registries(registries: &Registries) -> Self {
        let Some(registry) = registries.registry::<DamageType>() else {
            return Self::default();
        };
        let mut rules = Self {
            names: Vec::with_capacity(registry.len()),
            message_ids: Vec::with_capacity(registry.len()),
            scalings: Vec::with_capacity(registry.len()),
            flags: vec![DamageTypeFlags::default(); registry.len()],
            exhaustion: Vec::with_capacity(registry.len()),
        };
        for (_, name, value) in registry.iter() {
            rules.names.push(name.clone());
            rules.message_ids.push(value.message_id.clone());
            rules.scalings.push(value.scaling);
            rules.exhaustion.push(value.exhaustion);
        }
        if let Some(tags) = registries.tags::<DamageType>() {
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_ARMOR, |flags| {
                flags.bypasses_armor = true;
            });
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_RESISTANCE, |flags| {
                flags.bypasses_resistance = true;
            });
            rules.mark(
                &tags,
                damage_type_tag_keys::BYPASSES_INVULNERABILITY,
                |flags| flags.bypasses_invulnerability = true,
            );
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_EFFECTS, |flags| {
                flags.bypasses_effects = true;
            });
            rules.mark(&tags, damage_type_tag_keys::BYPASSES_COOLDOWN, |flags| {
                flags.bypasses_cooldown = true;
            });
            rules.mark(
                &tags,
                damage_type_tag_keys::BYPASSES_ENCHANTMENTS,
                |flags| flags.bypasses_enchantments = true,
            );
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
            rules.mark(&tags, damage_type_tag_keys::NO_IMPACT, |flags| {
                flags.no_impact = true;
            });
            rules.mark(&tags, damage_type_tag_keys::NO_KNOCKBACK, |flags| {
                flags.no_knockback = true;
            });
            rules.mark(&tags, damage_type_tag_keys::NO_HURT_SOUND, |flags| {
                flags.no_hurt_sound = true;
            });
        }
        rules
    }

    #[must_use]
    pub fn id(&self, key: ResourceKey<'_, DamageType>) -> Option<u32> {
        self.names
            .iter()
            .position(|name| name == key.location())
            .and_then(|index| u32::try_from(index).ok())
    }

    #[must_use]
    pub fn id_path(&self, path: &str) -> Option<u32> {
        self.names
            .iter()
            .position(|held| held.path() == path && held.namespace() == "minecraft")
            .and_then(|index| u32::try_from(index).ok())
    }

    #[must_use]
    pub fn id_of(&self, name: ResourceLocation<'_>) -> Option<u32> {
        self.names
            .iter()
            .position(|held| held.as_location() == name)
            .and_then(|index| u32::try_from(index).ok())
    }

    #[must_use]
    pub fn name(&self, damage_type: u32) -> Option<ResourceLocation<'_>> {
        self.names
            .get(damage_type as usize)
            .map(mcrs_minecraft_resource::ResourceLocationBuf::as_location)
    }

    #[must_use]
    pub fn message_id(&self, damage_type: u32) -> Option<&str> {
        self.message_ids
            .get(damage_type as usize)
            .map(String::as_str)
    }

    #[must_use]
    pub fn scaling(&self, damage_type: u32) -> DamageScaling {
        self.scalings
            .get(damage_type as usize)
            .copied()
            .unwrap_or(DamageScaling::Never)
    }

    #[must_use]
    pub fn flags(&self, damage_type: u32) -> DamageTypeFlags {
        self.flags
            .get(damage_type as usize)
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn exhaustion(&self, damage_type: u32) -> f32 {
        self.exhaustion
            .get(damage_type as usize)
            .copied()
            .unwrap_or(0.0)
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
