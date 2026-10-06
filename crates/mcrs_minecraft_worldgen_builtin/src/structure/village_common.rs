use super::*;
use mcrs_minecraft_keys as keys;

/// A marker that turns into `floor`, with the `animals` of one `kind` standing on it.
fn animals(c: &mut Canvas, floor: &str, kind: Fields, animals: &[([f64; 3], [i32; 3], Fields)]) {
    c.socket_facing([0, 0, 0], "down_south", BOTTOM, EMPTY, floor);
    for (pos, block_pos, animal) in animals {
        c.entity(*pos, *block_pos, &[MOB, kind, animal]);
    }
}

pub fn well_bottom(c: &mut Canvas) {
    let cobblestone = block(keys::block::COBBLESTONE.as_static_str());
    let water = block("minecraft:water[level=0]");
    let jigsaw = settled(keys::block::JIGSAW.as_static_str());
    c.solid(&cobblestone, [0, 0, 0], [3, 2, 3]);
    c.solid(&water, [1, 1, 1], [2, 2, 2]);
    c.place(&jigsaw, 3, 2, 0);

    c.jigsaw(
        [3, 2, 0],
        "minecraft:bottom",
        mcrs_minecraft_worldgen_feature::keys::template_pool::VILLAGE_COMMON_WELL_BOTTOMS.as_str(),
        keys::block::COBBLESTONE.as_static_str(),
    );
}

#[rustfmt::skip]
mod data {
    use super::{Fields, Tag};

    pub const CAT_MOB: Fields = &[("Age", Tag::Int(0)), ("CollarColor", Tag::Byte(14)), ("ForcedAge", Tag::Int(0)), ("Health", Tag::Float(10.0)), ("InLove", Tag::Int(0)), ("Leashed", Tag::Byte(0)), ("OwnerUUID", Tag::String("")), ("Pos", Tag::List(&[Tag::Double(106.59408887908502), Tag::Double(70.0), Tag::Double(-8.223750217101706)])), ("Rotation", Tag::List(&[Tag::Float(157.44612), Tag::Float(0.0)])), ("Sitting", Tag::Byte(0)), ("UUID", Tag::IntArray(&[821777084, -1905768911, -1314745931, 365541343])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.30000001192092896))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])])), ("id", Tag::String(mcrs_minecraft_entity::keys::EntityType::Cat.as_static_str()))];
    pub const HORSE_MOB: Fields = &[("Age", Tag::Int(0)), ("Bred", Tag::Byte(0)), ("EatingHaystack", Tag::Byte(0)), ("ForcedAge", Tag::Int(0)), ("InLove", Tag::Int(0)), ("Leashed", Tag::Byte(0)), ("Tame", Tag::Byte(0)), ("Temper", Tag::Int(0)), ("Variant", Tag::Int(261)), ("id", Tag::String(mcrs_minecraft_entity::keys::EntityType::Horse.as_static_str()))];
    pub const SHEEP_MOB: Fields = &[("Age", Tag::Int(0)), ("Color", Tag::Byte(0)), ("ForcedAge", Tag::Int(0)), ("Health", Tag::Float(8.0)), ("InLove", Tag::Int(0)), ("Leashed", Tag::Byte(0)), ("Motion", Tag::List(&[Tag::Double(-0.09219544780654096), Tag::Double(-0.0784000015258789), Tag::Double(0.09219544780654096)])), ("Rotation", Tag::List(&[Tag::Float(152.29918), Tag::Float(0.0)])), ("Sheared", Tag::Byte(0)), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(8.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.23000000417232513))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.054645176271711594)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])])), ("id", Tag::String(mcrs_minecraft_entity::keys::EntityType::Sheep.as_static_str()))];
    pub const COW_MOB: Fields = &[("Age", Tag::Int(0)), ("ForcedAge", Tag::Int(0)), ("Health", Tag::Float(10.0)), ("InLove", Tag::Int(0)), ("Leashed", Tag::Byte(0)), ("id", Tag::String(mcrs_minecraft_entity::keys::EntityType::Cow.as_static_str()))];
    pub const PIG_MOB: Fields = &[("Age", Tag::Int(0)), ("ForcedAge", Tag::Int(0)), ("Health", Tag::Float(10.0)), ("InLove", Tag::Int(0)), ("Leashed", Tag::Byte(0)), ("id", Tag::String(mcrs_minecraft_entity::keys::EntityType::Pig.as_static_str()))];
    pub const IRON_GOLEM_MOB: Fields = &[("Health", Tag::Float(100.0)), ("Leashed", Tag::Byte(0)), ("PlayerCreated", Tag::Byte(0)), ("Pos", Tag::List(&[Tag::Double(-57.45), Tag::Double(65.0), Tag::Double(-17.56)])), ("Rotation", Tag::List(&[Tag::Float(0.0), Tag::Float(0.0)])), ("UUID", Tag::IntArray(&[-68686641, -1554888549, -1769239523, 814428888])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(100.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(1.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.25))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.04635499892508863)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])])), ("id", Tag::String(mcrs_minecraft_entity::keys::EntityType::IronGolem.as_static_str()))];
    pub const CAT_ENTITY: Fields = &[("CatType", Tag::Int(1)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::BLACK.as_static_str())), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.30000001192092896))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.04026913607345884)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const CAT_ENTITY_10: Fields = &[("CatType", Tag::Int(8)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::WHITE.as_static_str())), ("UUID", Tag::IntArray(&[1971218531, 1228358991, -1575516236, 307837820])), ("Rotation", Tag::List(&[Tag::Float(137.05533), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(94.38217550315007), Tag::Double(70.0), Tag::Double(-8.100423939161246)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.30000001192092896))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.05918833585863412)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const CAT_ENTITY_2: Fields = &[("CatType", Tag::Int(4)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::BRITISH_SHORTHAIR.as_static_str())), ("UUID", Tag::IntArray(&[-1845145170, -1254342509, -1757105393, -529596160])), ("Rotation", Tag::List(&[Tag::Float(130.67245), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(98.1028744671547), Tag::Double(70.0), Tag::Double(-8.359899483462057)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.30000001192092896))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.01056965619542546)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const CAT_ENTITY_3: Fields = &[("CatType", Tag::Int(5)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::CALICO.as_static_str())), ("UUID", Tag::IntArray(&[-1183424020, 314265441, -1608804961, 4283046])), ("Rotation", Tag::List(&[Tag::Float(61.25116), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(99.03443539028753), Tag::Double(70.0), Tag::Double(-3.9627158492299044)]))];
    pub const CAT_ENTITY_4: Fields = &[("CatType", Tag::Int(10)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::ALL_BLACK.as_static_str())), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.30000001192092896))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.04026913607345884)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const CAT_ENTITY_5: Fields = &[("CatType", Tag::Int(6)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::PERSIAN.as_static_str())), ("UUID", Tag::IntArray(&[-1207851330, -2104799263, -1488738441, 459489061])), ("Rotation", Tag::List(&[Tag::Float(79.62802), Tag::Float(-40.0)])), ("Pos", Tag::List(&[Tag::Double(96.70458798203734), Tag::Double(70.0), Tag::Double(-3.9961968953776763)]))];
    pub const CAT_ENTITY_6: Fields = &[("CatType", Tag::Int(7)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::RAGDOLL.as_static_str())), ("UUID", Tag::IntArray(&[777251988, 828918218, -1611082442, 275989742])), ("Rotation", Tag::List(&[Tag::Float(-153.18343), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(92.0931867003231), Tag::Double(70.0), Tag::Double(-4.600347460065649)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.30000001192092896))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.04983638176425301)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const CAT_ENTITY_7: Fields = &[("CatType", Tag::Int(2)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::RED.as_static_str())), ("UUID", Tag::IntArray(&[54188730, 246500305, -1526174494, 616100662])), ("Rotation", Tag::List(&[Tag::Float(0.0), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(90.62), Tag::Double(70.0), Tag::Double(-9.074999988079071)]))];
    pub const CAT_ENTITY_8: Fields = &[("CatType", Tag::Int(3)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::SIAMESE.as_static_str())), ("UUID", Tag::IntArray(&[-1662907009, -2012133535, -1413786051, 698820659])), ("Rotation", Tag::List(&[Tag::Float(0.0), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(84.49192441515564), Tag::Double(69.0), Tag::Double(-5.1540007863679485)]))];
    pub const CAT_ENTITY_9: Fields = &[("CatType", Tag::Int(0)), ("variant", Tag::String(mcrs_minecraft_entity::keys::cat_variant::TABBY.as_static_str())), ("UUID", Tag::IntArray(&[-825753700, 1130906265, -1586988053, 428001104])), ("Rotation", Tag::List(&[Tag::Float(-113.609924), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(102.6606286838719), Tag::Double(70.0), Tag::Double(-8.191039682127876)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.30000001192092896))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.03185785106345955)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const COW_ENTITY: Fields = &[("UUID", Tag::IntArray(&[-1361869468, 529090201, -2039296741, -2131567754])), ("Motion", Tag::List(&[Tag::Double(-0.09219544780654096), Tag::Double(-0.0784000015258789), Tag::Double(-0.09219544780654096)])), ("Rotation", Tag::List(&[Tag::Float(153.60942), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-1725.925000011921), Tag::Double(4.0), Tag::Double(483.07499998807907)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.20000000298023224))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.034295998919266164)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const COW_ENTITY_2: Fields = &[("UUID", Tag::IntArray(&[-1865549295, -1871426108, -1397859800, -383448021])), ("Motion", Tag::List(&[Tag::Double(0.04609772390327048), Tag::Double(-0.0784000015258789), Tag::Double(0.04609772390327048)])), ("Rotation", Tag::List(&[Tag::Float(-88.56763), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-1725.074999988079), Tag::Double(4.0), Tag::Double(483.92500001192093)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.20000000298023224))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.04798545788703565)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const HORSE_ENTITY: Fields = &[("UUID", Tag::IntArray(&[-171020602, 93012791, -1771621546, 79228184])), ("Health", Tag::Float(24.0)), ("Rotation", Tag::List(&[Tag::Float(81.717514), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-1725.6767578125), Tag::Double(4.0), Tag::Double(488.6767578125)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(24.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.2622826207931691))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.022192265715227574)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::JumpStrength.as_static_str())), ("base", Tag::Double(0.6530562827837499))])]))];
    pub const HORSE_ENTITY_2: Fields = &[("UUID", Tag::IntArray(&[-1427474246, 390680113, -2038584555, -403291149])), ("Motion", Tag::List(&[Tag::Double(0.0), Tag::Double(-0.0784000015258789), Tag::Double(-0.05945717794201762)])), ("Health", Tag::Float(28.0)), ("Rotation", Tag::List(&[Tag::Float(60.220142), Tag::Float(-9.624206)])), ("Variant", Tag::Int(773)), ("Pos", Tag::List(&[Tag::Double(-1725.6767578125), Tag::Double(4.0), Tag::Double(498.3232421875)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(28.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.2021045130465277))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.15926849961451717)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::JumpStrength.as_static_str())), ("base", Tag::Double(0.7165035064057977))])]))];
    pub const HORSE_ENTITY_3: Fields = &[("UUID", Tag::IntArray(&[1187682076, -533118836, -1149808569, -1741801379])), ("Motion", Tag::List(&[Tag::Double(0.0), Tag::Double(-0.0784000015258789), Tag::Double(0.02972858897100881)])), ("Health", Tag::Float(26.333334)), ("Rotation", Tag::List(&[Tag::Float(0.0), Tag::Float(0.0)])), ("Variant", Tag::Int(768)), ("Pos", Tag::List(&[Tag::Double(-1725.6767578125), Tag::Double(4.0), Tag::Double(498.6767578125)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(26.333333333333332))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.22700428320403765))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::JumpStrength.as_static_str())), ("base", Tag::Double(0.7225464741268007))])]))];
    pub const HORSE_ENTITY_4: Fields = &[("UUID", Tag::IntArray(&[2088444919, 861817062, -1827831632, 696926046])), ("Motion", Tag::List(&[Tag::Double(-0.05945717794201762), Tag::Double(-0.0784000015258789), Tag::Double(0.05945717794201762)])), ("Health", Tag::Float(23.0)), ("Rotation", Tag::List(&[Tag::Float(1.2443948), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-1725.6767578125), Tag::Double(4.0), Tag::Double(503.6767578125)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(23.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.25600656498018504))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.0033747390987335897)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::JumpStrength.as_static_str())), ("base", Tag::Double(0.8283572877170323))])]))];
    pub const HORSE_ENTITY_5: Fields = &[("UUID", Tag::IntArray(&[1507650762, -1771945129, -1706666283, -277023426])), ("Motion", Tag::List(&[Tag::Double(0.02972858897100881), Tag::Double(-0.0784000015258789), Tag::Double(-0.02972858897100881)])), ("Health", Tag::Float(22.0)), ("Rotation", Tag::List(&[Tag::Float(0.0), Tag::Float(16.289177)])), ("Pos", Tag::List(&[Tag::Double(-1725.3232421875), Tag::Double(4.0), Tag::Double(503.3232421875)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(22.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.26253001048827457))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::JumpStrength.as_static_str())), ("base", Tag::Double(0.7876303040911276))])]))];
    pub const HORSE_ENTITY_6: Fields = &[("UUID", Tag::IntArray(&[-1286506481, 1615678055, -1279056836, 1603525937])), ("Health", Tag::Float(19.0)), ("Rotation", Tag::List(&[Tag::Float(164.32266), Tag::Float(0.0)])), ("Variant", Tag::Int(515)), ("Pos", Tag::List(&[Tag::Double(-1725.6767578125), Tag::Double(4.0), Tag::Double(507.6767578125)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(19.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.26228120303964536))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.010975439041230035)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::JumpStrength.as_static_str())), ("base", Tag::Double(0.6683554450629531))])]))];
    pub const HORSE_ENTITY_7: Fields = &[("UUID", Tag::IntArray(&[167890054, -2146287463, -1724168437, -459860845])), ("Health", Tag::Float(29.0)), ("Rotation", Tag::List(&[Tag::Float(-139.90747), Tag::Float(-35.437256)])), ("Variant", Tag::Int(6)), ("Pos", Tag::List(&[Tag::Double(-1725.4619769632711), Tag::Double(4.0), Tag::Double(511.32324218749994)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(29.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.2184342219686214))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.026412065736424697)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::JumpStrength.as_static_str())), ("base", Tag::Double(0.4222346876059485))])]))];
    pub const IRON_GOLEM_ENTITY: Fields = &[];
    pub const PIG_ENTITY: Fields = &[("UUID", Tag::IntArray(&[-512938654, -789165095, -1793655983, -1496727646])), ("Motion", Tag::List(&[Tag::Double(-0.04609772390327048), Tag::Double(-0.0784000015258789), Tag::Double(0.04609772390327048)])), ("Rotation", Tag::List(&[Tag::Float(111.30494), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-1725.925000011921), Tag::Double(4.0), Tag::Double(493.92500001192093)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.25))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.0021525815410118176)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const PIG_ENTITY_2: Fields = &[("UUID", Tag::IntArray(&[1371438866, -856669291, -1356771440, 1289927486])), ("Motion", Tag::List(&[Tag::Double(0.09219544780654096), Tag::Double(-0.0784000015258789), Tag::Double(-0.09219544780654096)])), ("Rotation", Tag::List(&[Tag::Float(-27.40152), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-1725.074999988079), Tag::Double(4.0), Tag::Double(493.07499998807907)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(10.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.25))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(0.058692256942751125)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const SHEEP_ENTITY: Fields = &[("UUID", Tag::IntArray(&[712369672, -1883748545, -1740320237, 550055650])), ("Color", Tag::Byte(15)), ("Pos", Tag::List(&[Tag::Double(-1716.925000011921), Tag::Double(4.0), Tag::Double(485.92500001192093)]))];
    pub const SHEEP_ENTITY_2: Fields = &[("UUID", Tag::IntArray(&[285296999, -1735244984, -1910948763, 861678319])), ("Motion", Tag::List(&[Tag::Double(0.04609772390327048), Tag::Double(-0.0784000015258789), Tag::Double(-0.04609772390327048)])), ("LeftHanded", Tag::Byte(1)), ("Rotation", Tag::List(&[Tag::Float(21.586525), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-1716.074999988079), Tag::Double(4.0), Tag::Double(485.07499998807907)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(8.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.23000000417232513))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.07929991095224685)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
    pub const SHEEP_ENTITY_3: Fields = &[("UUID", Tag::IntArray(&[932422599, 596394752, -1824382623, -1699496795])), ("Pos", Tag::List(&[Tag::Double(-1720.925000011921), Tag::Double(4.0), Tag::Double(476.92500001192093)]))];
    pub const SHEEP_ENTITY_4: Fields = &[("UUID", Tag::IntArray(&[-1015750806, 31606471, -1529664443, -1589461777])), ("Motion", Tag::List(&[Tag::Double(0.04609772390327048), Tag::Double(-0.0784000015258789), Tag::Double(-0.04609772390327048)])), ("LeftHanded", Tag::Byte(1)), ("Rotation", Tag::List(&[Tag::Float(21.586525), Tag::Float(-40.0)])), ("Pos", Tag::List(&[Tag::Double(-1720.074999988079), Tag::Double(4.0), Tag::Double(476.07499998807907)])), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MaxHealth.as_static_str())), ("base", Tag::Double(8.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::KnockbackResistance.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::MovementSpeed.as_static_str())), ("base", Tag::Double(0.23000000417232513))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::Armor.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::ArmorToughness.as_static_str())), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::FollowRange.as_static_str())), ("modifiers", Tag::List(&[Tag::Compound(&[("amount", Tag::Double(-0.07929991095224685)), ("id", Tag::String("minecraft:random_spawn_bonus")), ("operation", Tag::String("add_multiplied_base"))])])), ("base", Tag::Double(16.0))]), Tag::Compound(&[("id", Tag::String(mcrs_minecraft_entity::keys::Attribute::AttackKnockback.as_static_str())), ("base", Tag::Double(0.0))])]))];
}
use data::*;

pub const TEMPLATES: &[Entry] = &[
    ("village/common/animals/cat_black", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.594088879085021, 1.0, 0.7762497828982937],
                [0, 1, 0],
                CAT_ENTITY,
            )],
        )
    }),
    ("village/common/animals/cat_british", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.10287446715470594, 1.0, 0.6401005165379434],
                [0, 1, 0],
                CAT_ENTITY_2,
            )],
        )
    }),
    ("village/common/animals/cat_calico", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [1.034435390287527, 1.0, 1.0372841507700956],
                [1, 1, 1],
                CAT_ENTITY_3,
            )],
        )
    }),
    ("village/common/animals/cat_jellie", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.594088879085021, 1.0, 0.7762497828982937],
                [0, 1, 0],
                CAT_ENTITY_4,
            )],
        )
    }),
    ("village/common/animals/cat_persian", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.7045879820373386, 1.0, 1.0038031046223237],
                [0, 1, 1],
                CAT_ENTITY_5,
            )],
        )
    }),
    ("village/common/animals/cat_ragdoll", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.09318670032310195, 1.0, 0.3996525399343511],
                [0, 1, 0],
                CAT_ENTITY_6,
            )],
        )
    }),
    ("village/common/animals/cat_red", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.6200000000000045, 1.0, -0.07499998807907104],
                [0, 1, -1],
                CAT_ENTITY_7,
            )],
        )
    }),
    ("village/common/animals/cat_siamese", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.491924415155637, 1.0, 0.8459992136320515],
                [0, 1, 0],
                CAT_ENTITY_8,
            )],
        )
    }),
    ("village/common/animals/cat_tabby", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.6606286838719058, 1.0, 0.8089603178721241],
                [0, 1, 0],
                CAT_ENTITY_9,
            )],
        )
    }),
    ("village/common/animals/cat_white", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            CAT_MOB,
            &[(
                [0.38217550315006577, 1.0, 0.8995760608387542],
                [0, 1, 0],
                CAT_ENTITY_10,
            )],
        )
    }),
    ("village/common/animals/cows_1", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            COW_MOB,
            &[
                (
                    [0.07499998807907104, 1.0, 0.07499998807907104],
                    [0, 1, 0],
                    COW_ENTITY,
                ),
                (
                    [0.925000011920929, 1.0, 0.925000011920929],
                    [0, 1, 0],
                    COW_ENTITY_2,
                ),
            ],
        )
    }),
    ("village/common/animals/horses_1", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            HORSE_MOB,
            &[([0.3232421875, 1.0, 0.6767578125], [0, 1, 0], HORSE_ENTITY)],
        )
    }),
    ("village/common/animals/horses_2", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            HORSE_MOB,
            &[
                ([0.3232421875, 1.0, 0.3232421875], [0, 1, 0], HORSE_ENTITY_2),
                ([0.3232421875, 1.0, 0.6767578125], [0, 1, 0], HORSE_ENTITY_3),
            ],
        )
    }),
    ("village/common/animals/horses_3", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            HORSE_MOB,
            &[
                ([0.3232421875, 1.0, 0.6767578125], [0, 1, 0], HORSE_ENTITY_4),
                ([0.6767578125, 1.0, 0.3232421875], [0, 1, 0], HORSE_ENTITY_5),
            ],
        )
    }),
    ("village/common/animals/horses_4", [1, 3, 1], |c| {
        animals(
            c,
            keys::block::HAY_BLOCK.as_static_str(),
            HORSE_MOB,
            &[([0.3232421875, 1.0, 0.6767578125], [0, 1, 0], HORSE_ENTITY_6)],
        )
    }),
    ("village/common/animals/horses_5", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            HORSE_MOB,
            &[(
                [0.5380230367288732, 1.0, 0.32324218749994316],
                [0, 1, 0],
                HORSE_ENTITY_7,
            )],
        )
    }),
    ("village/common/animals/pigs_1", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            PIG_MOB,
            &[
                (
                    [0.07499998807907104, 1.0, 0.925000011920929],
                    [0, 1, 0],
                    PIG_ENTITY,
                ),
                (
                    [0.925000011920929, 1.0, 0.07499998807907104],
                    [0, 1, 0],
                    PIG_ENTITY_2,
                ),
            ],
        )
    }),
    ("village/common/animals/sheep_1", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            SHEEP_MOB,
            &[
                (
                    [0.07499998807907104, 1.0, 0.925000011920929],
                    [0, 1, 0],
                    SHEEP_ENTITY,
                ),
                (
                    [0.925000011920929, 1.0, 0.07499998807907104],
                    [0, 1, 0],
                    SHEEP_ENTITY_2,
                ),
            ],
        )
    }),
    ("village/common/animals/sheep_2", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            SHEEP_MOB,
            &[
                (
                    [0.07499998807907104, 1.0, 0.925000011920929],
                    [0, 1, 0],
                    SHEEP_ENTITY_3,
                ),
                (
                    [0.925000011920929, 1.0, 0.07499998807907104],
                    [0, 1, 0],
                    SHEEP_ENTITY_4,
                ),
            ],
        )
    }),
    ("village/common/iron_golem", [1, 3, 1], |c| {
        animals(
            c,
            NOTHING,
            IRON_GOLEM_MOB,
            &[(
                [0.5499999999999972, 1.0, 0.4400000000000013],
                [0, 1, 0],
                IRON_GOLEM_ENTITY,
            )],
        )
    }),
    ("village/common/well_bottom", [4, 3, 4], well_bottom),
];
