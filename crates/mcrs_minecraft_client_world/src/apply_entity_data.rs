use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_visual::{SyncedData, UPSIDE_DOWN_NAMES, VariantId};
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, MetadataEntry};
use mcrs_minecraft_protocol_game::entity_data_index::{
    AGE_LOCKED, BABY, CUSTOM_NAME, HEALTH, LIVING_FLAGS, MOB_FLAGS, NAME_VISIBLE, POSE,
    SHARED_FLAGS, SLEEPING_POSITION, TICKS_FROZEN,
};
use mcrs_minecraft_protocol_game::text::{Text, TextContent};

pub fn apply_entity_data<'a>(
    synced: &mut SyncedData,
    data: Option<&EntityTypeData>,
    entries: impl IntoIterator<Item = &'a MetadataEntry<'a>>,
) {
    for entry in entries {
        apply_entry(synced, data, entry);
    }
}

fn apply_entry(synced: &mut SyncedData, data: Option<&EntityTypeData>, entry: &MetadataEntry<'_>) {
    let living = data.is_some_and(EntityTypeData::is_living);
    let ageable = data.is_some_and(EntityTypeData::is_ageable);
    let mob = data.is_some_and(|data| data.mob().is_some());
    match (entry.index, &entry.value) {
        (SHARED_FLAGS, MetaDataValue::Byte(flags)) => synced.shared_flags = *flags as u8,
        (CUSTOM_NAME, MetaDataValue::OptionalText(name)) => {
            synced.upside_down = name.as_ref().is_some_and(is_upside_down_name);
            synced.custom_name.clone_from(name);
        }
        (NAME_VISIBLE, MetaDataValue::Boolean(visible)) => synced.name_visible = *visible,
        (POSE, MetaDataValue::Pose(pose)) => synced.pose = *pose,
        (TICKS_FROZEN, MetaDataValue::VarInt(ticks)) => synced.ticks_frozen = ticks.0,
        (LIVING_FLAGS, MetaDataValue::Byte(flags)) if living => {
            synced.living_flags = *flags as u8;
        }
        (HEALTH, MetaDataValue::Float(health)) if living => synced.health = *health,
        (SLEEPING_POSITION, MetaDataValue::OptionalBlockPos(at)) if living => {
            synced.sleeping_position = *at;
        }
        (MOB_FLAGS, MetaDataValue::Byte(flags)) if mob => synced.mob_flags = *flags as u8,
        (BABY, MetaDataValue::Boolean(baby)) if ageable => synced.baby = *baby,
        (AGE_LOCKED, MetaDataValue::Boolean(locked)) if ageable => synced.age_locked = *locked,
        (_, MetaDataValue::PigVariant(id)) => synced.variant = Some(VariantId::Pig(*id)),
        (_, MetaDataValue::FrogVariant(id)) => synced.variant = Some(VariantId::Frog(*id)),
        _ => {}
    }
}

fn is_upside_down_name(name: &Text) -> bool {
    let mut plain = String::new();
    push_plain(name, &mut plain);
    UPSIDE_DOWN_NAMES.contains(&plain.as_str())
}

fn push_plain(text: &Text, plain: &mut String) {
    if let TextContent::Text { text } = &text.content {
        plain.push_str(text);
    }
    for child in &text.extra {
        push_plain(child, plain);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_resource::Id;

    fn entry(index: u8, value: MetaDataValue<'static>) -> MetadataEntry<'static> {
        MetadataEntry { index, value }
    }

    #[test]
    fn a_pig_reads_the_living_and_ageable_indices_and_its_variant() {
        let type_data = EntityTypeData::of(EntityType::Pig);
        let mut data = SyncedData::default();
        let variant = Id::checked(2, 4).unwrap();
        apply_entity_data(
            &mut data,
            type_data,
            &[
                entry(0, MetaDataValue::Byte(0x21)),
                entry(9, MetaDataValue::Float(0.0)),
                entry(16, MetaDataValue::Boolean(true)),
                entry(19, MetaDataValue::PigVariant(variant)),
                entry(7, MetaDataValue::VarInt(VarInt(140))),
            ],
        );
        assert!(data.is_on_fire(type_data));
        assert!(data.is_invisible());
        assert!(data.is_dying());
        assert!(data.baby);
        assert!(data.is_fully_frozen());
        assert_eq!(data.variant, Some(VariantId::Pig(variant)));
    }

    #[test]
    fn the_plain_text_of_the_custom_name_turns_an_entity_upside_down() {
        let type_data = EntityTypeData::of(EntityType::Pig);
        let mut data = SyncedData::default();
        let name = |text: Text| entry(2, MetaDataValue::OptionalText(Some(text)));
        apply_entity_data(&mut data, type_data, &[name(Text::text("Dinner") + "bone")]);
        assert!(data.upside_down, "the siblings count in the plain text");
        apply_entity_data(&mut data, type_data, &[name(Text::text("dinnerbone"))]);
        assert!(!data.upside_down, "the comparison is case sensitive");
        apply_entity_data(&mut data, type_data, &[name(Text::text("Grumm"))]);
        apply_entity_data(
            &mut data,
            type_data,
            &[entry(2, MetaDataValue::OptionalText(None))],
        );
        assert!(!data.upside_down, "a removed name is not upside down");
        assert_eq!(data.custom_name, None, "a removed name clears the tag text");
        apply_entity_data(&mut data, type_data, &[name(Text::text("Pig"))]);
        assert_eq!(data.custom_name, Some(Text::text("Pig")));
    }

    #[test]
    fn an_index_of_another_type_is_ignored() {
        let type_data = EntityTypeData::of(EntityType::Player);
        let mut data = SyncedData::default();
        apply_entity_data(
            &mut data,
            type_data,
            &[entry(16, MetaDataValue::Boolean(true))],
        );
        assert!(!data.baby, "a player has no baby flag at index 16");
    }
}
