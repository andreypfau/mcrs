use bevy_ecs::component::Component;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::variant::{FrogVariant, PigVariant};
use mcrs_minecraft_protocol_game::clientbound::{AttributeOperation, AttributeSnapshot};
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, MetadataEntry, Pose};
use mcrs_minecraft_protocol_game::text::{Text, TextContent};
use mcrs_minecraft_resource::Id;

const SHARED_FLAGS: u8 = 0;
const CUSTOM_NAME: u8 = 2;
const NAME_VISIBLE: u8 = 3;
const POSE: u8 = 6;
const TICKS_FROZEN: u8 = 7;
const LIVING_FLAGS: u8 = 8;
const HEALTH: u8 = 9;
const MOB_FLAGS: u8 = 15;
const BABY: u8 = 16;
const AGE_LOCKED: u8 = 17;

const ON_FIRE: u8 = 1 << 0;
const CROUCHING: u8 = 1 << 1;
const SPRINTING: u8 = 1 << 3;
const SWIMMING: u8 = 1 << 4;
const INVISIBLE: u8 = 1 << 5;
const GLOWING: u8 = 1 << 6;

/// The plain texts of a custom name that turn an entity upside down.
pub const UPSIDE_DOWN_NAMES: [&str; 2] = ["Dinnerbone", "Grumm"];

/// The frozen ticks from which an entity is fully frozen.
pub const FULLY_FROZEN_TICKS: i32 = 140;

/// The numeric variant id that the entity data of a variant type carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariantId {
    Pig(Id<PigVariant>),
    Frog(Id<FrogVariant>),
}

/// The entity data that the client keeps for an entity, and its scale attribute.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct SyncedData {
    pub shared_flags: u8,
    /// The plain text of the custom name is one of [`UPSIDE_DOWN_NAMES`].
    pub upside_down: bool,
    pub name_visible: bool,
    pub pose: Pose,
    pub ticks_frozen: i32,
    pub living_flags: u8,
    pub health: f32,
    pub mob_flags: u8,
    pub baby: bool,
    pub age_locked: bool,
    /// `None` until the server sends a variant: the type then has its default variant.
    pub variant: Option<VariantId>,
    /// The value of the attribute `minecraft:scale`.
    pub scale: f64,
}

impl Default for SyncedData {
    fn default() -> Self {
        Self {
            shared_flags: 0,
            upside_down: false,
            name_visible: false,
            pose: Pose::Standing,
            ticks_frozen: 0,
            living_flags: 0,
            health: 1.0,
            mob_flags: 0,
            baby: false,
            age_locked: false,
            variant: None,
            scale: 1.0,
        }
    }
}

impl SyncedData {
    /// Stores the entries that an entity of the type of `data` has. An entry of an index that
    /// the type does not have, or of another wire type, changes nothing. A type with no `data`
    /// has only the entries of an entity.
    pub fn apply<'a>(
        &mut self,
        data: Option<&EntityTypeData>,
        entries: impl IntoIterator<Item = &'a MetadataEntry<'a>>,
    ) {
        for entry in entries {
            self.apply_entry(data, entry);
        }
    }

    fn apply_entry(&mut self, data: Option<&EntityTypeData>, entry: &MetadataEntry<'_>) {
        let living = data.is_some_and(EntityTypeData::is_living);
        let ageable = data.is_some_and(EntityTypeData::is_ageable);
        let mob = data.is_some_and(|data| data.mob().is_some());
        match (entry.index, &entry.value) {
            (SHARED_FLAGS, MetaDataValue::Byte(flags)) => self.shared_flags = *flags as u8,
            (CUSTOM_NAME, MetaDataValue::OptionalText(name)) => {
                self.upside_down = name.as_ref().is_some_and(is_upside_down_name);
            }
            (NAME_VISIBLE, MetaDataValue::Boolean(visible)) => self.name_visible = *visible,
            (POSE, MetaDataValue::Pose(pose)) => self.pose = *pose,
            (TICKS_FROZEN, MetaDataValue::VarInt(ticks)) => self.ticks_frozen = ticks.0,
            (LIVING_FLAGS, MetaDataValue::Byte(flags)) if living => {
                self.living_flags = *flags as u8;
            }
            (HEALTH, MetaDataValue::Float(health)) if living => self.health = *health,
            (MOB_FLAGS, MetaDataValue::Byte(flags)) if mob => self.mob_flags = *flags as u8,
            (BABY, MetaDataValue::Boolean(baby)) if ageable => self.baby = *baby,
            (AGE_LOCKED, MetaDataValue::Boolean(locked)) if ageable => self.age_locked = *locked,
            (_, MetaDataValue::PigVariant(id)) => self.variant = Some(VariantId::Pig(*id)),
            (_, MetaDataValue::FrogVariant(id)) => self.variant = Some(VariantId::Frog(*id)),
            _ => {}
        }
    }

    /// Takes the scale from the attributes of an attributes packet.
    pub fn apply_attributes<'a>(
        &mut self,
        attributes: impl IntoIterator<Item = &'a AttributeSnapshot<'a>>,
    ) {
        for attribute in attributes {
            if attribute.attribute == Attribute::Scale {
                self.scale = attribute_value(attribute);
            }
        }
    }

    #[must_use]
    pub fn is_dying(&self) -> bool {
        self.health <= 0.0
    }

    #[must_use]
    pub fn is_on_fire(&self, data: Option<&EntityTypeData>) -> bool {
        self.shared_flags & ON_FIRE != 0 && !data.is_some_and(|data| data.fire_immune)
    }

    #[must_use]
    pub fn is_sneaking(&self) -> bool {
        self.shared_flags & CROUCHING != 0
    }

    #[must_use]
    pub fn is_sprinting(&self) -> bool {
        self.shared_flags & SPRINTING != 0
    }

    #[must_use]
    pub fn is_swimming(&self) -> bool {
        self.shared_flags & SWIMMING != 0
    }

    #[must_use]
    pub fn is_invisible(&self) -> bool {
        self.shared_flags & INVISIBLE != 0
    }

    #[must_use]
    pub fn is_glowing(&self) -> bool {
        self.shared_flags & GLOWING != 0
    }

    #[must_use]
    pub fn is_fully_frozen(&self) -> bool {
        self.ticks_frozen >= FULLY_FROZEN_TICKS
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

/// The smallest and the largest value of the attribute `minecraft:scale`.
pub const SCALE_RANGE: (f64, f64) = (0.0625, 16.0);

/// The base plus each added value `b′`, plus `b′` times the sum of the base multipliers, then
/// times one plus each total multiplier, clamped to [`SCALE_RANGE`]. NaN gives the smallest
/// scale.
fn attribute_value(attribute: &AttributeSnapshot<'_>) -> f64 {
    let sum = |operation: AttributeOperation| -> f64 {
        attribute
            .modifiers
            .iter()
            .filter(|modifier| modifier.operation == operation)
            .map(|modifier| modifier.amount)
            .sum()
    };
    let added = attribute.base + sum(AttributeOperation::AddValue);
    let mut value = added + added * sum(AttributeOperation::AddMultipliedBase);
    for modifier in &attribute.modifiers {
        if modifier.operation == AttributeOperation::AddMultipliedTotal {
            value *= 1.0 + modifier.amount;
        }
    }
    let (min, max) = SCALE_RANGE;
    if value.is_nan() {
        min
    } else {
        value.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_protocol::VarInt;

    fn entry(index: u8, value: MetaDataValue<'static>) -> MetadataEntry<'static> {
        MetadataEntry { index, value }
    }

    #[test]
    fn a_pig_reads_the_living_and_ageable_indices_and_its_variant() {
        let type_data = EntityTypeData::of(EntityType::Pig);
        let mut data = SyncedData::default();
        let variant = Id::checked(2, 4).unwrap();
        data.apply(
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
        data.apply(type_data, &[name(Text::text("Dinner") + "bone")]);
        assert!(data.upside_down, "the siblings count in the plain text");
        data.apply(type_data, &[name(Text::text("dinnerbone"))]);
        assert!(!data.upside_down, "the comparison is case sensitive");
        data.apply(type_data, &[name(Text::text("Grumm"))]);
        data.apply(type_data, &[entry(2, MetaDataValue::OptionalText(None))]);
        assert!(!data.upside_down, "a removed name is not upside down");
    }

    #[test]
    fn an_index_of_another_type_is_ignored() {
        let type_data = EntityTypeData::of(EntityType::Player);
        let mut data = SyncedData::default();
        data.apply(type_data, &[entry(16, MetaDataValue::Boolean(true))]);
        assert!(!data.baby, "a player has no baby flag at index 16");
    }
}
