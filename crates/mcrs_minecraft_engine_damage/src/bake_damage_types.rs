use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::DamageTags;
use mcrs_minecraft_damage::DamageTypeRules;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_entity::damage_type_tag_keys;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::TagKey;

struct TagBit {
    tag: TagKey<'static, DamageType>,
    bit: DamageTags,
}

const TAGS: [TagBit; 8] = [
    TagBit {
        tag: damage_type_tag_keys::BYPASSES_ARMOR,
        bit: DamageTags::BYPASSES_ARMOR,
    },
    TagBit {
        tag: damage_type_tag_keys::BYPASSES_COOLDOWN,
        bit: DamageTags::BYPASSES_COOLDOWN,
    },
    TagBit {
        tag: damage_type_tag_keys::BYPASSES_EFFECTS,
        bit: DamageTags::BYPASSES_EFFECTS,
    },
    TagBit {
        tag: damage_type_tag_keys::BYPASSES_RESISTANCE,
        bit: DamageTags::BYPASSES_RESISTANCE,
    },
    TagBit {
        tag: damage_type_tag_keys::IS_FIRE,
        bit: DamageTags::IS_FIRE,
    },
    TagBit {
        tag: damage_type_tag_keys::IS_FALL,
        bit: DamageTags::IS_FALL,
    },
    TagBit {
        tag: damage_type_tag_keys::NO_KNOCKBACK,
        bit: DamageTags::NO_KNOCKBACK,
    },
    TagBit {
        tag: damage_type_tag_keys::BYPASSES_INVULNERABILITY,
        bit: DamageTags::BYPASSES_INVULNERABILITY,
    },
];

pub(crate) fn bake_damage_types(
    registries: Res<CurrentRegistries>,
    mut rules: ResMut<DamageTypeRules>,
) {
    let count = registries
        .entries::<DamageType>()
        .map_or(0, |entries| entries.len());
    let mut rows = vec![DamageTags::empty(); count];
    if let Some(tags) = registries.tags::<DamageType>() {
        for TagBit { tag, bit } in TAGS {
            let Some(tag) = tags.id(tag) else {
                continue;
            };
            for member in tags.members(tag) {
                if let Some(row) = rows.get_mut(member.index()) {
                    *row = row.union(bit);
                }
            }
        }
    }
    let baked = DamageTypeRules::new(rows);
    if *rules != baked {
        *rules = baked;
    }
}
