use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::chicken_sound_variant_keys;
use mcrs_minecraft_entity::cow_sound_variant_keys;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::pig_sound_variant_keys;
use mcrs_minecraft_entity::registry_keys::CHICKEN_SOUND_VARIANT;
use mcrs_minecraft_entity::registry_keys::COW_SOUND_VARIANT;
use mcrs_minecraft_entity::registry_keys::PIG_SOUND_VARIANT;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::entity::MetadataEntry;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocation;

const LEFT_HANDED: f64 = 0.05;

pub(crate) fn first_looks(
    kind: &EntityTypeData,
    stream: &KeyedStream,
    registries: Option<&Registries>,
) -> MobLooks {
    if !kind.is_ageable() {
        return MobLooks::default();
    }
    let unit = stream.unit(0);
    let sound = match kind.entity_type {
        EntityType::Pig => pick(
            registries,
            PIG_SOUND_VARIANT.location(),
            pig_sound_variant_keys::CLASSIC.location(),
            unit,
        )
        .map(MetaDataValue::PigSoundVariant),
        EntityType::Cow => pick(
            registries,
            COW_SOUND_VARIANT.location(),
            cow_sound_variant_keys::CLASSIC.location(),
            unit,
        )
        .map(MetaDataValue::CowSoundVariant),
        EntityType::Chicken => pick(
            registries,
            CHICKEN_SOUND_VARIANT.location(),
            chicken_sound_variant_keys::CLASSIC.location(),
            unit,
        )
        .map(MetaDataValue::ChickenSoundVariant),
        _ => None,
    };
    let type_entries = kind
        .variant_indices
        .sound_variant
        .zip(sound)
        .map(|(index, value)| MetadataEntry { index, value })
        .into_iter()
        .collect();
    MobLooks {
        no_ai: false,
        left_handed: stream.unit(1) < LEFT_HANDED,
        type_entries,
    }
}

fn pick<R>(
    registries: Option<&Registries>,
    registry: ResourceLocation<'static>,
    default: ResourceLocation<'static>,
    unit: f64,
) -> Option<Id<R>> {
    let names = registries?.names_of(registry)?;
    let index = ((unit * names.len() as f64) as usize).min(names.len().saturating_sub(1));
    let name = names.get(index)?;
    if *name == default {
        return None;
    }
    Id::checked(u16::try_from(index).ok()?, names.len())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mcrs_minecraft_entity_key::EntityUuid;
    use mcrs_minecraft_game_registries::registry_load::test_registries;

    use super::*;

    #[test]
    fn the_first_setup_draws_each_sound_variant_with_an_equal_chance() {
        let cases = [
            (&EntityTypeData::PIG, "PigSoundVariant", 3),
            (&EntityTypeData::COW, "CowSoundVariant", 2),
            (&EntityTypeData::CHICKEN, "ChickenSoundVariant", 2),
        ];
        for (kind, name, entries) in cases {
            let mut counts: BTreeMap<String, u32> = BTreeMap::new();
            for uuid in 0..3000u128 {
                let looks = first_looks(
                    kind,
                    &KeyedStream::new(&EntityUuid::from_u128(uuid), 0),
                    Some(test_registries()),
                );
                let drawn = match looks.type_entries.as_slice() {
                    [] => "default".to_owned(),
                    [entry] => {
                        assert_eq!(Some(entry.index), kind.variant_indices.sound_variant);
                        format!("{:?}", entry.value)
                    }
                    more => panic!("one sound variant entry at most: {more:?}"),
                };
                *counts.entry(drawn).or_default() += 1;
            }
            assert_eq!(counts.len(), entries, "{name}: {counts:?}");
            for (drawn, count) in &counts {
                assert!(
                    drawn == "default" || drawn.starts_with(name),
                    "{name}: {drawn}"
                );
                let share = f64::from(*count) / 3000.0;
                assert!(
                    (share - 1.0 / entries as f64).abs() < 0.04,
                    "{name}: {counts:?}"
                );
            }
        }
    }
}
