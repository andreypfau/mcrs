use std::io::Cursor;
use std::num::NonZeroU8;

use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::BlockId;
use mcrs_minecraft_item_stack::BlockSet;
use mcrs_minecraft_item_stack::BlockTagId;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::DimensionPos;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::EntityTypeId;
use mcrs_minecraft_item_stack::EntityTypeSet;
use mcrs_minecraft_item_stack::Equippable;
use mcrs_minecraft_item_stack::EquippableSlot;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::LodestoneTracker;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::ModifierDisplay;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::SavedError;
use mcrs_minecraft_item_stack::SlotGroup;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_item_stack::SoundEventRef;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::ToolRule;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_stack::read_saved_optional;
use mcrs_minecraft_item_stack::read_saved_with_slot;
use mcrs_minecraft_item_stack::write_saved;
use mcrs_minecraft_item_stack::write_saved_optional;
use mcrs_minecraft_item_stack::write_saved_with_slot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_profile::Profile;
use mcrs_minecraft_profile::ProfileIdentity;
use mcrs_minecraft_profile::Property;
use mcrs_minecraft_profile::SkinPatch;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_snbt::parse_compound;
use mcrs_minecraft_snbt::parse_tag;
use mcrs_minecraft_snbt::write_compound;
use mcrs_minecraft_sound::SoundEventValue;
use mcrs_minecraft_text::Text;
use mcrs_voxel::BlockPos;
use serde::Deserialize;

use crate::common::same;
use crate::common::same_map;
use crate::common::stack;
use crate::common::unhex;

const AIR: u16 = 0;
const STONE: u16 = 1;
const DIAMOND_SWORD: u16 = 2;
const SHARPNESS: u16 = 0;
const UNBREAKING: u16 = 1;
const ATTACK_DAMAGE: u16 = 0;
const STONE_BLOCK: u16 = 0;
const DIRT_BLOCK: u16 = 1;
const MINEABLE_PICKAXE: u16 = 0;
const MAX_STACK_SIZE_TYPE: u16 = 0;
const DAMAGE_TYPE: u16 = 1;

struct Names;

impl Names {
    fn paths(table: NameTable) -> &'static [&'static str] {
        match table {
            NameTable::Item => &["air", "stone", "diamond_sword"],
            NameTable::Enchantment => &["sharpness", "unbreaking"],
            NameTable::Attribute => &["attack_damage"],
            NameTable::Block => &["stone", "dirt"],
            NameTable::BlockTag => &["mineable/pickaxe"],
            NameTable::ComponentType => &[
                "max_stack_size",
                "damage",
                "entity_data",
                "block_entity_data",
            ],
            NameTable::MapDecorationType => &["player", "red_x", "target_x", "banner_red"],
            NameTable::SoundEvent => &["item.armor.equip_iron", "entity.item.break"],
            NameTable::EntityType => &["pig", "zombie"],
            NameTable::EntityTypeTag => &["skeletons"],
            NameTable::MobEffect => &["speed", "slowness", "haste"],
            NameTable::Potion => &["water"],
            NameTable::TrimMaterial => &["amethyst"],
            NameTable::TrimPattern => &["bolt", "coast"],
            NameTable::Instrument => &["ponder_goat_horn"],
            _ => &[],
        }
    }
}

impl NameLookup for Names {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        if name.namespace() != "minecraft" {
            return None;
        }
        let index = Self::paths(table)
            .iter()
            .position(|path| *path == name.path())?;
        u16::try_from(index).ok()
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        let path = Self::paths(table).get(usize::from(id))?;
        ResourceLocation::new("minecraft", path).ok()
    }
}

fn saved(snbt: &str) -> NbtCompound {
    parse_compound(snbt).expect("the test input is valid SNBT")
}

fn with_components(components: &str) -> NbtCompound {
    saved(&format!(
        "{{id:\"minecraft:stone\",components:{{{components}}}}}"
    ))
}

fn assert_same(actual: &NbtCompound, expected: &NbtCompound, case: &str) {
    assert!(
        same_map(actual, expected),
        "{case}: wrote {}, expected {}",
        write_compound(actual),
        write_compound(expected)
    );
}

fn tag_of(bytes: &[u8]) -> NbtTag {
    NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(bytes)))
        .expect("the golden holds one NBT value")
}

fn bytes_of(tag: &NbtTag) -> Vec<u8> {
    let mut writer = WriteAdaptor::new(Vec::new());
    tag.serialize(&mut writer)
        .expect("a tag writes to a vector");
    writer.into_inner()
}

fn modifier(id: &str, amount: f64, operation: ModifierOperation) -> AttributeModifier {
    let id = ResourceLocationBuf::read(id).expect("the test id is an identifier");
    AttributeModifier::new(AttributeId(ATTACK_DAMAGE), id, amount, operation)
}

#[test]
fn a_saved_stack_with_each_decoded_type_reads_to_its_value_and_writes_back() {
    let nbt = saved(
        r##"{id:"minecraft:diamond_sword",count:2,components:{
            "minecraft:max_stack_size":16,
            "minecraft:max_damage":1561,
            "minecraft:damage":7,
            "minecraft:unbreakable":{},
            "minecraft:item_model":"minecraft:diamond_sword",
            "minecraft:rarity":"epic",
            "minecraft:enchantments":{"minecraft:sharpness":5,"minecraft:unbreaking":255},
            "minecraft:attribute_modifiers":[
                {type:"minecraft:attack_damage",id:"minecraft:base_attack_damage",amount:6.0d,operation:"add_value",slot:"mainhand"},
                {type:"minecraft:attack_damage",id:"test:hidden",amount:-2.5d,operation:"add_multiplied_total",display:{type:"hidden"}}],
            "minecraft:tooltip_display":{hide_tooltip:1b,hidden_components:["minecraft:damage","minecraft:max_stack_size"]},
            "minecraft:repair_cost":3,
            "minecraft:food":{nutrition:4,saturation:1.8000001f,can_always_eat:1b},
            "minecraft:tool":{rules:[
                {blocks:"#minecraft:mineable/pickaxe",speed:8.0f,correct_for_drops:1b},
                {blocks:["minecraft:stone","minecraft:dirt"]},
                {blocks:"minecraft:dirt",correct_for_drops:0b}],
                default_mining_speed:2.5f,damage_per_block:0,can_destroy_blocks_in_creative:0b}
        }}"##,
    );

    let level = |enchantment: u16, level: u8| EnchantmentLevel {
        enchantment: EnchantmentId(enchantment),
        level: NonZeroU8::new(level).expect("the test levels are not zero"),
    };
    let mut base = modifier(
        "minecraft:base_attack_damage",
        6.0,
        ModifierOperation::AddValue,
    );
    base.slot = SlotGroup::Mainhand;
    let mut hidden = modifier("test:hidden", -2.5, ModifierOperation::AddMultipliedTotal);
    hidden.display = ModifierDisplay::Hidden;
    let mut tooltip = TooltipDisplay::hiding([
        ComponentType(DAMAGE_TYPE),
        ComponentType(MAX_STACK_SIZE_TYPE),
    ]);
    tooltip.hide_tooltip = true;
    let mut tag_rule = ToolRule::new(BlockSet::Tag(BlockTagId(MINEABLE_PICKAXE)))
        .with_speed(8.0)
        .expect("8 is a valid speed");
    tag_rule.correct_for_drops = Some(true);
    let list_rule = ToolRule::new(BlockSet::Blocks(
        vec![BlockId(STONE_BLOCK), BlockId(DIRT_BLOCK)].into(),
    ));
    let mut block_rule = ToolRule::new(BlockSet::Blocks(vec![BlockId(DIRT_BLOCK)].into()));
    block_rule.correct_for_drops = Some(false);
    let mut tool = Tool::new(vec![tag_rule, list_rule, block_rule]);
    tool.default_mining_speed = 2.5;
    tool.damage_per_block = 0;
    tool.can_destroy_blocks_in_creative = false;
    let model = ResourceLocationBuf::read("minecraft:diamond_sword").expect("an identifier");
    let expected = stack(DIAMOND_SWORD, 2)
        .set(MaxStackSize(16))
        .set(MaxDamage(1561))
        .set(Damage(7))
        .set(Unbreakable)
        .set(ItemModel::new(model))
        .set(Rarity::Epic)
        .set(Enchantments::new([level(SHARPNESS, 5), level(UNBREAKING, 255)]).expect("two types"))
        .set(AttributeModifiers {
            modifiers: vec![base, hidden].into(),
        })
        .set(tooltip)
        .set(RepairCost(3))
        .set(Food {
            nutrition: 4,
            saturation: 1.800_000_1,
            can_always_eat: true,
        })
        .set(tool);

    assert_eq!(read_saved(&nbt, &Names), Ok(expected.clone()));
    let written = write_saved(&expected, &Names).expect("each value is in range");
    assert_same(&written, &nbt, "each decoded type");
}

#[test]
fn the_writer_gives_the_canonical_form_of_what_the_reader_accepts() {
    let cases: [(&str, &str); 9] = [
        (r#"{id:"stone"}"#, r#"{id:"minecraft:stone",count:1}"#),
        (
            r#"{id:"stone",count:3b,components:{"minecraft:max_stack_size":64.5f,"minecraft:max_damage":10000000000.0d,"minecraft:damage":-0.5d,"minecraft:repair_cost":4294967298L,"minecraft:map_id":2.7d,"minecraft:enchantments":{sharpness:3s},"minecraft:attribute_modifiers":[{type:"attack_damage",id:"base",amount:2,operation:"add_value"}],"minecraft:tooltip_display":{hide_tooltip:2b},"minecraft:food":{nutrition:4.9f,saturation:1,can_always_eat:0.5f},"minecraft:tool":{rules:[{blocks:"stone",speed:2b,correct_for_drops:-1}],default_mining_speed:3L,damage_per_block:2.9d,can_destroy_blocks_in_creative:0L}}}"#,
            r#"{id:"minecraft:stone",count:3,components:{"minecraft:max_stack_size":64,"minecraft:max_damage":2147483647,"minecraft:damage":0,"minecraft:repair_cost":2,"minecraft:map_id":2,"minecraft:enchantments":{"minecraft:sharpness":3},"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"minecraft:base",amount:2.0d,operation:"add_value"}],"minecraft:tooltip_display":{hide_tooltip:1b},"minecraft:food":{nutrition:4,saturation:1.0f,can_always_eat:1b},"minecraft:tool":{rules:[{blocks:"minecraft:stone",speed:2.0f,correct_for_drops:1b}],default_mining_speed:3.0f,damage_per_block:2,can_destroy_blocks_in_creative:0b}}}"#,
        ),
        (
            r#"{id:"stone",components:{damage:3,"!minecraft:damage":{},"!repair_cost":{},"minecraft:repair_cost":5}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"!minecraft:damage":{},"minecraft:repair_cost":5}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:5,components:{"minecraft:bundle_contents":[{id:"minecraft:stone"}],"!custom_data":{x:1},repair_cost:2,"!damage":{x:1},"minecraft:custom_name":"named"}}"#,
            r#"{id:"minecraft:stone",count:5,components:{"!minecraft:damage":{},"minecraft:custom_name":"named","minecraft:repair_cost":2,"minecraft:bundle_contents":[{id:"minecraft:stone"}],"!minecraft:custom_data":{}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:unbreakable":{x:1}}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:unbreakable":{}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"attack_damage",id:"base",amount:1.5d,operation:"add_multiplied_base",slot:"any",display:{type:"default"}}],"minecraft:food":{nutrition:0,saturation:0.5f,can_always_eat:0b}}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"minecraft:base",amount:1.5d,operation:"add_multiplied_base"}],"minecraft:food":{nutrition:0,saturation:0.5f}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tool":{rules:[{blocks:["stone"]}],default_mining_speed:1.0f,damage_per_block:1,can_destroy_blocks_in_creative:1b},"minecraft:enchantments":{unbreaking:1}}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tool":{rules:[{blocks:"minecraft:stone"}]},"minecraft:enchantments":{"minecraft:unbreaking":1}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tooltip_display":{hide_tooltip:0b,hidden_components:["minecraft:damage","damage","max_stack_size"]},"minecraft:item_model":"stone"}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tooltip_display":{hidden_components:["minecraft:damage","minecraft:max_stack_size"]},"minecraft:item_model":"minecraft:stone"}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"test:shown",amount:1.0d,operation:"add_value",display:{type:"override",value:{text:"shown",bold:1b}}}]}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"test:shown",amount:1.0d,operation:"add_value",display:{type:"override",value:{text:"shown",bold:1b}}}]}}"#,
        ),
    ];
    for (input, canonical) in cases {
        let stack = read_saved(&saved(input), &Names).expect(input);
        let written = write_saved(&stack, &Names).expect(input);
        assert_same(&written, &saved(canonical), input);
    }
}

#[test]
fn an_equippable_takes_its_defaults_and_keeps_an_empty_entity_list_apart_from_no_list() {
    let head = Equippable::new(EquippableSlot::Head);
    let body = Equippable::new(EquippableSlot::Body);
    let mut nobody = body.clone();
    nobody.allowed_entities = Some(EntityTypeSet::EntityTypes(Vec::new().into()));
    let mut pig = body.clone();
    pig.allowed_entities = Some(EntityTypeSet::EntityTypes(vec![EntityTypeId(0)].into()));
    let mut snip = Equippable::new(EquippableSlot::Feet);
    snip.shearing_sound = Some(SoundEventRef::Inline(SoundEventValue {
        sound_id: ResourceLocationBuf::read("mcrs:snip").expect("an identifier"),
        range: None,
    }));
    let cases = [
        (
            r#"{slot:"head",dispensable:1b,swappable:1b,damage_on_hurt:1b,equip_on_interact:0b,can_be_sheared:0b}"#,
            &head,
            r#"{slot:"head"}"#,
        ),
        (r#"{slot:"body"}"#, &body, r#"{slot:"body"}"#),
        (
            r#"{slot:"body",allowed_entities:[]}"#,
            &nobody,
            r#"{slot:"body",allowed_entities:[]}"#,
        ),
        (
            r#"{slot:"body",allowed_entities:["pig"]}"#,
            &pig,
            r#"{slot:"body",allowed_entities:"minecraft:pig"}"#,
        ),
        (
            r#"{slot:"feet",shearing_sound:{sound_id:"mcrs:snip",range:"far"}}"#,
            &snip,
            r#"{slot:"feet",shearing_sound:{sound_id:"mcrs:snip"}}"#,
        ),
    ];
    for (input, expected, canonical) in cases {
        let read = read_saved(
            &with_components(&format!(r#""minecraft:equippable":{input}"#)),
            &Names,
        )
        .expect(input);
        assert_eq!(read, stack(STONE, 1).set(expected.clone()), "{input}");
        let written = write_saved(&read, &Names).expect(input);
        let canonical = saved(&format!(
            r#"{{id:"minecraft:stone",count:1,components:{{"minecraft:equippable":{canonical}}}}}"#
        ));
        assert_same(&written, &canonical, input);
    }
    assert_ne!(body, nobody, "STK-SHP-031: no list is not the empty list");
}

#[test]
fn the_patch_keeps_one_order_whatever_the_order_of_the_saved_keys() {
    let forward = with_components(
        r#""minecraft:entity_data":{id:"minecraft:pig"},"!minecraft:block_entity_data":{},"minecraft:repair_cost":2,"!minecraft:damage":{},"minecraft:custom_name":"named""#,
    );
    let backward = with_components(
        r#""minecraft:custom_name":"named","!minecraft:damage":{},"minecraft:repair_cost":2,"!minecraft:block_entity_data":{},"minecraft:entity_data":{id:"minecraft:pig"}"#,
    );

    let stack = read_saved(&forward, &Names).expect("each key is valid");
    assert_eq!(read_saved(&backward, &Names).as_ref(), Ok(&stack));

    let undecoded: Vec<&str> = stack
        .patch()
        .iter()
        .filter_map(|entry| match entry {
            PatchEntry::Undecoded(value) => Some(value.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(undecoded, ["minecraft:entity_data"]);

    let written = write_saved(&stack, &Names).expect("each entry has a saved form");
    let Some(NbtTag::Compound(patch)) = written.get("components") else {
        panic!("the writer wrote no patch: {}", write_compound(&written));
    };
    let keys: Vec<&str> = patch
        .child_tags
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "!minecraft:damage",
            "minecraft:custom_name",
            "minecraft:repair_cost",
            "!minecraft:block_entity_data",
            "minecraft:entity_data",
        ]
    );
}

#[test]
fn the_saved_count_is_an_int_from_1_to_99_and_an_absent_count_is_1() {
    let cases: [(&str, Result<u8, SavedError>); 12] = [
        ("", Ok(1)),
        (",count:1", Ok(1)),
        (",count:99", Ok(99)),
        (",count:0", Err(SavedError::BadCount)),
        (",count:-1", Err(SavedError::BadCount)),
        (",count:100", Err(SavedError::BadCount)),
        (",count:257", Err(SavedError::BadCount)),
        (",count:1b", Ok(1)),
        (",count:64.5f", Ok(64)),
        (",count:4294967298L", Ok(2)),
        (",count:0.5f", Err(SavedError::BadCount)),
        (",count:\"1\"", Err(SavedError::BadCount)),
    ];
    for (count, expected) in cases {
        let nbt = saved(&format!("{{id:\"minecraft:stone\"{count}}}"));
        let read = read_saved(&nbt, &Names).map(|stack| stack.count.get());
        assert_eq!(read, expected, "count `{count}`");
    }

    assert!(write_saved(&stack(STONE, 99), &Names).is_ok());
}

#[test]
fn the_reader_names_the_rule_that_a_saved_stack_breaks() {
    let frames: [(&str, SavedError); 6] = [
        (r#"{count:1}"#, SavedError::BadItemId),
        (r#"{id:"Not An Id"}"#, SavedError::BadItemId),
        (r#"{id:7}"#, SavedError::BadItemId),
        (r#"{id:"minecraft:emerald"}"#, SavedError::UnknownItem),
        (r#"{id:"air"}"#, SavedError::AirItem),
        (r#"{id:"stone",components:[]}"#, SavedError::BadPatch),
    ];
    for (input, expected) in frames {
        assert_eq!(read_saved(&saved(input), &Names), Err(expected), "{input}");
    }

    let unknown =
        |component: BuiltinKey, table: NameTable| SavedError::UnknownEntry { component, table };
    let patches: [(&str, SavedError); 23] = [
        (r#""Bad Key":1"#, SavedError::BadKey),
        (r#""!Bad Key":{}"#, SavedError::BadKey),
        (
            r#""minecraft:no_such_type":1"#,
            SavedError::UnknownComponentType,
        ),
        (
            r#""!minecraft:no_such_type":{}"#,
            SavedError::UnknownComponentType,
        ),
        (r#""!minecraft:damage":1b"#, SavedError::BadRemovalValue),
        (r#"damage:1,"minecraft:damage":2"#, SavedError::RepeatedKey),
        (
            r#""!entity_data":{},"!minecraft:entity_data":{}"#,
            SavedError::RepeatedKey,
        ),
        (
            r#""minecraft:damage":-1.5f"#,
            SavedError::OutOfRange(BuiltinKey::Damage),
        ),
        (
            r#""minecraft:damage":"7""#,
            SavedError::BadValue(BuiltinKey::Damage),
        ),
        (
            r#""minecraft:unbreakable":1b"#,
            SavedError::BadValue(BuiltinKey::Unbreakable),
        ),
        (
            r#""minecraft:rarity":"legendary""#,
            SavedError::BadValue(BuiltinKey::Rarity),
        ),
        (
            r#""minecraft:item_model":"Not An Id""#,
            SavedError::BadValue(BuiltinKey::ItemModel),
        ),
        (
            r#""minecraft:food":{nutrition:1}"#,
            SavedError::BadValue(BuiltinKey::Food),
        ),
        (
            r#""minecraft:tooltip_display":{hide_tooltip:"true"}"#,
            SavedError::BadValue(BuiltinKey::TooltipDisplay),
        ),
        (
            r#""minecraft:attribute_modifiers":[{type:"attack_damage",id:"base",amount:1.0d,operation:"add_value",display:{type:"override"}}]"#,
            SavedError::BadValue(BuiltinKey::AttributeModifiers),
        ),
        (
            r#""minecraft:enchantments":{"minecraft:sharpness":0}"#,
            SavedError::OutOfRange(BuiltinKey::Enchantments),
        ),
        (
            r#""minecraft:enchantments":{"minecraft:sharpness":256}"#,
            SavedError::OutOfRange(BuiltinKey::Enchantments),
        ),
        (
            r#""minecraft:food":{nutrition:-1,saturation:0.0f}"#,
            SavedError::OutOfRange(BuiltinKey::Food),
        ),
        (
            r#""minecraft:tool":{rules:[{blocks:"stone",speed:0.0f}]}"#,
            SavedError::OutOfRange(BuiltinKey::Tool),
        ),
        (
            r#""minecraft:enchantments":{"minecraft:mending":1}"#,
            unknown(BuiltinKey::Enchantments, NameTable::Enchantment),
        ),
        (
            r##""minecraft:tool":{rules:[{blocks:"#minecraft:no_such_tag"}]}"##,
            unknown(BuiltinKey::Tool, NameTable::BlockTag),
        ),
        (
            r#""minecraft:tool":{rules:[{blocks:["stone","bedrock"]}]}"#,
            unknown(BuiltinKey::Tool, NameTable::Block),
        ),
        (
            r#""minecraft:tooltip_display":{hidden_components:["minecraft:no_such_type"]}"#,
            unknown(BuiltinKey::TooltipDisplay, NameTable::ComponentType),
        ),
    ];
    for (components, expected) in patches {
        let nbt = with_components(components);
        assert_eq!(read_saved(&nbt, &Names), Err(expected), "{components}");
    }
}

#[test]
fn a_lodestone_target_reads_from_each_array_form_and_a_long_keeps_its_low_32_bits() {
    let target = |x: i32, y: i32, z: i32| LodestoneTracker {
        target: Some(DimensionPos::new(
            ResourceLocationBuf::new("minecraft", "overworld").expect("a valid dimension"),
            BlockPos::new(x, y, z),
        )),
        tracked: true,
    };
    let cases: [(&str, Result<LodestoneTracker, SavedError>); 6] = [
        ("[I;1,-2,3]", Ok(target(1, -2, 3))),
        ("[B;1b,-2b,3b]", Ok(target(1, -2, 3))),
        ("[L;1L,-2L,3L]", Ok(target(1, -2, 3))),
        (
            "[L;4294967297L,2147483649L,-1L]",
            Ok(target(1, -2147483647, -1)),
        ),
        ("[1,-2,3]", Ok(target(1, -2, 3))),
        (
            "[1,2]",
            Err(SavedError::BadValue(BuiltinKey::LodestoneTracker)),
        ),
    ];
    for (pos, expected) in cases {
        let nbt = with_components(&format!(
            r#""minecraft:lodestone_tracker":{{target:{{dimension:"minecraft:overworld",pos:{pos}}}}}"#
        ));
        let expected = expected.map(|tracker| stack(STONE, 1).set(tracker));
        assert_eq!(read_saved(&nbt, &Names), expected, "{pos}");
    }
}

#[test]
fn a_map_decoration_needs_a_map_with_each_field_and_a_known_type() {
    let bad_value = SavedError::BadValue(BuiltinKey::MapDecorations);
    let cases: [(&str, SavedError); 2] = [
        (r#"{"+":1b}"#, bad_value),
        (
            r#"{"+":{type:"no_such_type",x:1,z:2,rotation:0}}"#,
            SavedError::UnknownEntry {
                component: BuiltinKey::MapDecorations,
                table: NameTable::MapDecorationType,
            },
        ),
    ];
    for (decorations, expected) in cases {
        let nbt = with_components(&format!(r#""minecraft:map_decorations":{decorations}"#));
        assert_eq!(read_saved(&nbt, &Names), Err(expected), "{decorations}");
    }
}

#[test]
fn a_profile_ignores_the_order_of_its_property_groups_but_not_the_case_of_its_name() {
    let profile = |profile: &str| {
        read_saved(
            &with_components(&format!(r#""minecraft:profile":{profile}"#)),
            &Names,
        )
        .expect("the test profile is valid")
    };
    let first_x = profile(r#"{properties:[{name:"x",value:"1"},{name:"y",value:"2"}]}"#);
    let first_y = profile(r#"{properties:[{name:"y",value:"2"},{name:"x",value:"1"}]}"#);
    assert_eq!(first_x, first_y);
    let one_two = profile(r#"{properties:[{name:"x",value:"1"},{name:"x",value:"2"}]}"#);
    let two_one = profile(r#"{properties:[{name:"x",value:"2"},{name:"x",value:"1"}]}"#);
    assert_ne!(one_two, two_one);
    let upper = profile(r#""Steve""#);
    let lower = profile(r#""steve""#);
    assert_ne!(upper, lower);
    let partial = |properties: &[(&str, &str)]| Profile {
        profile: ProfileIdentity::Partial {
            name: None,
            id: None,
            properties: properties
                .iter()
                .map(|&(name, value)| Property {
                    name: name.to_owned(),
                    value: value.to_owned(),
                    signature: None,
                })
                .collect(),
        },
        skin: SkinPatch::default(),
    };
    let ungrouped = [
        (
            partial(&[("x", "1"), ("t", "1"), ("x", "1")]),
            partial(&[("x", "1"), ("t", "1"), ("y", "1")]),
            false,
        ),
        (
            partial(&[("x", "1"), ("t", "1"), ("x", "2")]),
            partial(&[("x", "1"), ("x", "2"), ("t", "1")]),
            true,
        ),
    ];
    for (left, right, equal) in ungrouped {
        assert_eq!(left == right, equal, "{left:?} == {right:?}");
        assert_eq!(right == left, equal, "{right:?} == {left:?}");
    }
}

#[test]
fn the_map_form_of_the_properties_holds_at_most_16_properties_in_total() {
    let values = vec![r#""v""#; 17].join(",");
    let nbt = with_components(&format!(
        r#""minecraft:profile":{{properties:{{p:[{values}]}}}}"#
    ));
    assert_eq!(
        read_saved(&nbt, &Names),
        Err(SavedError::BadValue(BuiltinKey::Profile))
    );
}

#[test]
fn the_writer_names_the_rule_that_a_stack_breaks() {
    let unnamed_enchantment = Enchantments::new([EnchantmentLevel {
        enchantment: EnchantmentId(9),
        level: NonZeroU8::MIN,
    }])
    .expect("one enchantment");
    let cases: [(ItemStack, SavedError); 5] = [
        (stack(999, 1), SavedError::UnnamedId(NameTable::Item)),
        (stack(AIR, 1), SavedError::AirItem),
        (
            stack(STONE, 1).set(MaxStackSize(100)),
            SavedError::OutOfRange(BuiltinKey::MaxStackSize),
        ),
        (
            stack(STONE, 1).set(Damage(-1)),
            SavedError::OutOfRange(BuiltinKey::Damage),
        ),
        (
            stack(STONE, 1).set(unnamed_enchantment),
            SavedError::UnnamedId(NameTable::Enchantment),
        ),
    ];
    for (stack, expected) in cases {
        assert_eq!(write_saved(&stack, &Names), Err(expected), "{stack:?}");
    }
}

#[test]
fn the_optional_form_is_an_empty_map_and_the_slot_is_an_unsigned_byte() {
    assert_eq!(read_saved_optional(&NbtCompound::new(), &Names), Ok(None));
    assert_eq!(write_saved_optional(None, &Names), Ok(NbtCompound::new()));
    let stone = stack(STONE, 3);
    let written = write_saved_optional(Some(&stone), &Names).expect("a stone has a saved form");
    assert_eq!(
        read_saved_optional(&written, &Names),
        Ok(Some(stone.clone()))
    );

    let slots: [(&str, Result<u8, SavedError>); 4] = [
        (r#"{id:"stone"}"#, Ok(0)),
        (r#"{id:"stone",Slot:5b}"#, Ok(5)),
        (r#"{id:"stone",Slot:-1b}"#, Ok(255)),
        (r#"{id:"stone",Slot:5}"#, Err(SavedError::BadSlot)),
    ];
    for (input, expected) in slots {
        let read = read_saved_with_slot(&saved(input), &Names).map(|slotted| slotted.slot);
        assert_eq!(read, expected, "{input}");
    }
    let slotted = SlotStack {
        slot: 255,
        stack: stone,
    };
    let written = write_saved_with_slot(&slotted, &Names).expect("a stone has a saved form");
    assert_same(
        &written,
        &saved(r#"{id:"minecraft:stone",count:3,Slot:-1b}"#),
        "slot 255",
    );
}

#[derive(Deserialize)]
struct NestedGolden {
    cases: Vec<NestedCase>,
}

#[derive(Deserialize)]
struct NestedCase {
    name: String,
    nbt: String,
}

#[test]
fn a_stack_that_the_game_saved_reads_to_its_value_and_writes_back() {
    let golden: NestedGolden =
        serde_json::from_str(&fixture("item_nested").text()).expect("the golden is JSON");
    let case = golden
        .cases
        .iter()
        .find(|case| case.name == "use_remainder_full")
        .expect("the golden has the case");
    let NbtTag::Compound(nbt) = tag_of(&unhex(&case.nbt)) else {
        panic!("the case is not a map");
    };

    let read = read_saved(&nbt, &Names).expect("the game wrote a valid stack");

    let decoded = stack(DIAMOND_SWORD, 3)
        .set(MaxStackSize(16))
        .set(Damage(7))
        .set(Unbreakable)
        .set(CustomName(Text::text("named")))
        .unset::<RepairCost>();
    assert_eq!(read, decoded);

    let written = write_saved(&read, &Names).expect("the stack has a saved form");
    assert_same(&written, &nbt, "use_remainder_full");
}

#[derive(Deserialize)]
struct PlainGolden {
    values: Vec<PlainValue>,
    errors: Vec<PlainError>,
}

#[derive(Deserialize)]
struct PlainValue {
    kind: String,
    nbt: Option<String>,
}

#[derive(Deserialize)]
struct PlainError {
    kind: String,
    input: String,
    error: String,
}

const PLAIN_KINDS: [(&str, BuiltinKey); 22] = [
    ("max_stack_size", BuiltinKey::MaxStackSize),
    ("max_damage", BuiltinKey::MaxDamage),
    ("damage", BuiltinKey::Damage),
    ("unbreakable", BuiltinKey::Unbreakable),
    ("rarity", BuiltinKey::Rarity),
    ("repair_cost", BuiltinKey::RepairCost),
    ("attack_animation", BuiltinKey::AttackAnimation),
    ("interact_animation", BuiltinKey::InteractAnimation),
    ("map_id", BuiltinKey::MapId),
    ("map_decorations", BuiltinKey::MapDecorations),
    ("profile", BuiltinKey::Profile),
    ("dyed_color", BuiltinKey::DyedColor),
    ("custom_model_data", BuiltinKey::CustomModelData),
    (
        "enchantment_glint_override",
        BuiltinKey::EnchantmentGlintOverride,
    ),
    ("block_state", BuiltinKey::BlockState),
    ("lodestone_tracker", BuiltinKey::LodestoneTracker),
    ("trim", BuiltinKey::Trim),
    ("potion_contents", BuiltinKey::PotionContents),
    ("custom_data", BuiltinKey::CustomData),
    ("suspicious_stew_effects", BuiltinKey::SuspiciousStewEffects),
    (
        "ominous_bottle_amplifier",
        BuiltinKey::OminousBottleAmplifier,
    ),
    ("instrument", BuiltinKey::Instrument),
];

fn plain_kind(kind: &str) -> Option<BuiltinKey> {
    PLAIN_KINDS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, component)| *component)
}

const JSON_ONLY_ERRORS: [(&str, &str); 2] = [
    ("enchantment_glint_override", "1"),
    (
        "instrument",
        r#"{"sound_event":"minecraft:entity.item.break","use_duration":-0.0,"range":1.0,"description":"x"}"#,
    ),
];

fn registry_of(component: BuiltinKey) -> Option<NameTable> {
    match component {
        BuiltinKey::Trim => Some(NameTable::TrimPattern),
        BuiltinKey::PotionContents => Some(NameTable::Potion),
        BuiltinKey::Instrument => Some(NameTable::Instrument),
        _ => None,
    }
}

fn with_component(kind: &str, value: NbtTag) -> NbtCompound {
    let mut patch = NbtCompound::new();
    patch.put(&format!("minecraft:{kind}"), value);
    let mut nbt = saved(r#"{id:"minecraft:stone"}"#);
    nbt.put("components", NbtTag::Compound(patch));
    nbt
}

#[test]
fn the_components_that_the_game_saved_decode_and_write_the_same_bytes() {
    let golden: PlainGolden =
        serde_json::from_str(&fixture("item_plain").text()).expect("the golden is JSON");

    let mut checked = 0;
    for value in &golden.values {
        if plain_kind(&value.kind).is_none() {
            continue;
        }
        let hex = value.nbt.as_ref().expect("each of the nine types is saved");
        let bytes = unhex(hex);
        let nbt = with_component(&value.kind, tag_of(&bytes));
        let read = read_saved(&nbt, &Names).expect("the game wrote a valid component");
        assert!(
            matches!(read.patch(), [PatchEntry::Value(_)]),
            "{} {hex} is not decoded",
            value.kind
        );
        let written = write_saved(&read, &Names).expect("the component has a saved form");
        let Some(NbtTag::Compound(patch)) = written.get("components") else {
            panic!("the writer wrote no patch for {}", value.kind);
        };
        let key = format!("minecraft:{}", value.kind);
        let written = patch.get(&key).expect("the writer wrote the component");
        let expected = tag_of(&bytes);
        if let NbtTag::Compound(_) = expected {
            assert!(same(written, &expected), "{} {hex}", value.kind);
        } else {
            assert_eq!(bytes_of(written), bytes, "{} {hex}", value.kind);
        }
        checked += 1;
    }
    assert!(checked >= 32, "the golden lost cases: {checked}");

    let mut rejected = 0;
    for error in &golden.errors {
        let Some(component) = plain_kind(&error.kind) else {
            continue;
        };
        if JSON_ONLY_ERRORS.contains(&(error.kind.as_str(), error.input.as_str())) {
            continue;
        }
        let input = parse_tag(&error.input).expect("the rejected input is valid SNBT");
        let message = error.error.as_str();
        let expected = match component {
            BuiltinKey::Unbreakable
            | BuiltinKey::Rarity
            | BuiltinKey::MapDecorations
            | BuiltinKey::Profile => SavedError::BadValue(component),
            BuiltinKey::MaxStackSize
            | BuiltinKey::MaxDamage
            | BuiltinKey::Damage
            | BuiltinKey::RepairCost
            | BuiltinKey::MapId => SavedError::OutOfRange(component),
            _ if message.contains("Failed to get element")
                || message.contains("Unknown registry key") =>
            {
                SavedError::UnknownEntry {
                    component,
                    table: registry_of(component)
                        .expect("only a registry component has an unknown entry"),
                }
            }
            _ if message.contains("within range")
                || message.contains("must be positive")
                || message.contains("must be non-negative") =>
            {
                SavedError::OutOfRange(component)
            }
            _ => SavedError::BadValue(component),
        };
        let nbt = with_component(&error.kind, input);
        assert_eq!(
            read_saved(&nbt, &Names),
            Err(expected),
            "{} {}",
            error.kind,
            error.input
        );
        rejected += 1;
    }
    assert!(rejected >= 18, "the golden lost error cases: {rejected}");
}

#[test]
fn an_inline_instrument_with_a_negative_zero_use_duration_is_out_of_range() {
    let mut instrument = NbtCompound::new();
    instrument.put(
        "sound_event",
        NbtTag::String("minecraft:entity.item.break".into()),
    );
    instrument.put("use_duration", NbtTag::Float(-0.0));
    instrument.put("range", NbtTag::Float(1.0));
    instrument.put("description", NbtTag::String("x".into()));
    let nbt = with_component("instrument", NbtTag::Compound(instrument));
    assert_eq!(
        read_saved(&nbt, &Names),
        Err(SavedError::OutOfRange(BuiltinKey::Instrument))
    );
}
