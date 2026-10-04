use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, to_nbt_compound};
use mcrs_minecraft_protocol::item::{
    ComponentPatch, EnchantmentGlintOverride, ItemStackValue, ItemStackWithSlot,
};

fn stack(slot: u8, path: &str, count: i32, components: ComponentPatch) -> ItemStackWithSlot {
    ItemStackWithSlot {
        slot,
        stack: ItemStackValue {
            item: ResourceKey::from_location(ResourceLocation::minecraft(path)),
            count: Bounded(count),
            components,
        },
    }
}

#[test]
fn nbt_round_trip_keeps_the_slot_byte_and_component_tags() {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(EnchantmentGlintOverride(true));
    let value = stack(200, "diamond_sword", 3, patch);
    let nbt = to_nbt_compound(&value).unwrap();
    assert_eq!(nbt.get_byte("Slot"), Some(200u8 as i8));
    assert_eq!(nbt.get_int("count"), Some(3));
    let components = nbt.get_compound("components").unwrap();
    assert!(matches!(
        components.get("minecraft:enchantment_glint_override"),
        Some(NbtTag::Byte(1))
    ));
    let back: ItemStackWithSlot = from_tag(nbt.into()).unwrap();
    assert_eq!(back, value);
}

#[test]
fn absent_slot_and_count_take_their_defaults() {
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "minecraft:stone".to_owned());
    let back: ItemStackWithSlot = from_tag(nbt.into()).unwrap();
    assert_eq!(back, stack(0, "stone", 1, ComponentPatch::EMPTY));
}

#[test]
fn unknown_and_duplicate_keys_are_refused() {
    let mut nbt = NbtCompound::new();
    nbt.put_string("id", "minecraft:stone".to_owned());
    nbt.put_int("Count", 1);
    assert!(from_tag::<ItemStackWithSlot>(nbt.into()).is_err());
    let json = r#"{"Slot": 1, "id": "minecraft:stone", "Slot": 2}"#;
    assert!(serde_json::from_str::<ItemStackWithSlot>(json).is_err());
}
