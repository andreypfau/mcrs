use crate::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{AttributeBases, TypeBase};

#[must_use]
pub const fn attribute_bases(entity_type: EntityType) -> Option<AttributeBases> {
    match BY_TYPE.split_at_checked(entity_type.id().number() as usize) {
        Some((_, [bases, ..])) => *bases,
        _ => None,
    }
}

const BY_TYPE: [Option<AttributeBases>; EntityType::ALL.len()] = {
    let mut table = [None; EntityType::ALL.len()];
    let mut rest = ROWS;
    while let [row, tail @ ..] = rest {
        if let Some((_, [slot, ..])) =
            table.split_at_mut_checked(row.entity_type.id().number() as usize)
        {
            *slot = Some(AttributeBases::new(row.more, row.differing));
        }
        rest = tail;
    }
    table
};

struct TypeRow {
    entity_type: EntityType,
    more: &'static [Attribute],
    differing: &'static [TypeBase],
}

const fn base(attribute: Attribute, base: f64) -> TypeBase {
    TypeBase { attribute, base }
}

#[rustfmt::skip]
const ROWS: &[TypeRow] = {
    use Attribute::*;
    &[
        TypeRow { entity_type: EntityType::Allay, more: &[AttackDamage, FlyingSpeed, FollowRange], differing: &[base(FlyingSpeed, 0.1_f32 as f64), base(FollowRange, 16.0), base(MovementSpeed, 0.1_f32 as f64)] },
        TypeRow { entity_type: EntityType::Armadillo, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 12.0), base(MovementSpeed, 0.14)] },
        TypeRow { entity_type: EntityType::ArmorStand, more: &[], differing: &[base(StepHeight, 0.0)] },
        TypeRow { entity_type: EntityType::Axolotl, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 14.0), base(MovementSpeed, 1.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Bat, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 6.0)] },
        TypeRow { entity_type: EntityType::Bee, more: &[AttackDamage, FlyingSpeed, FollowRange, TemptRange], differing: &[base(FlyingSpeed, 0.6_f32 as f64), base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Blaze, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 6.0), base(FollowRange, 48.0), base(MovementSpeed, 0.23_f32 as f64)] },
        TypeRow { entity_type: EntityType::Bogged, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 16.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Breeze, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 3.0), base(FollowRange, 24.0), base(MaxHealth, 30.0), base(MovementSpeed, 0.63_f32 as f64)] },
        TypeRow { entity_type: EntityType::Camel, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(MaxHealth, 32.0), base(MovementSpeed, 0.09_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.5)] },
        TypeRow { entity_type: EntityType::CamelHusk, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(MaxHealth, 32.0), base(MovementSpeed, 0.09_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.5)] },
        TypeRow { entity_type: EntityType::Cat, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 3.0), base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::CaveSpider, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 12.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Chicken, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 4.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Cod, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 3.0)] },
        TypeRow { entity_type: EntityType::CopperGolem, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 12.0), base(MovementSpeed, 0.2_f32 as f64), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Cow, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 0.2_f32 as f64)] },
        TypeRow { entity_type: EntityType::Creaking, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 3.0), base(MaxHealth, 1.0), base(MovementSpeed, 0.4_f32 as f64), base(StepHeight, 1.0625)] },
        TypeRow { entity_type: EntityType::Creeper, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Dolphin, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 3.0), base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 1.2_f32 as f64)] },
        TypeRow { entity_type: EntityType::Donkey, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(JumpStrength, 0.5), base(MaxHealth, 53.0), base(MovementSpeed, 0.175_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Drowned, more: &[AttackDamage, FollowRange, SpawnReinforcements], differing: &[base(Armor, 2.0), base(AttackDamage, 3.0), base(FollowRange, 35.0), base(MovementSpeed, 0.23_f32 as f64), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::ElderGuardian, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 8.0), base(FollowRange, 16.0), base(MaxHealth, 80.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::EnderDragon, more: &[FollowRange], differing: &[base(CameraDistance, 16.0), base(FollowRange, 16.0), base(MaxHealth, 200.0)] },
        TypeRow { entity_type: EntityType::Enderman, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 7.0), base(FollowRange, 64.0), base(MaxHealth, 40.0), base(MovementSpeed, 0.3_f32 as f64), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Endermite, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 8.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Evoker, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 12.0), base(MaxHealth, 24.0), base(MovementSpeed, 0.5)] },
        TypeRow { entity_type: EntityType::Fox, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(MaxHealth, 10.0), base(MovementSpeed, 0.3_f32 as f64), base(SafeFallDistance, 5.0)] },
        TypeRow { entity_type: EntityType::Frog, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 10.0), base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 1.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Frostbite, more: &[AttackDamage, FollowRange, SpawnReinforcements], differing: &[base(Armor, 2.0), base(AttackDamage, 3.0), base(FollowRange, 35.0), base(MaxHealth, 24.0), base(MovementSpeed, 0.21_f32 as f64)] },
        TypeRow { entity_type: EntityType::Ghast, more: &[FlyingSpeed, FollowRange], differing: &[base(CameraDistance, 8.0), base(FlyingSpeed, 0.06), base(FollowRange, 100.0), base(MaxHealth, 10.0)] },
        TypeRow { entity_type: EntityType::Giant, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 50.0), base(CameraDistance, 16.0), base(FollowRange, 16.0), base(MaxHealth, 100.0), base(MovementSpeed, 0.5)] },
        TypeRow { entity_type: EntityType::GlowSquid, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 10.0)] },
        TypeRow { entity_type: EntityType::Goat, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 0.2_f32 as f64)] },
        TypeRow { entity_type: EntityType::Guardian, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 6.0), base(FollowRange, 16.0), base(MaxHealth, 30.0), base(MovementSpeed, 0.5)] },
        TypeRow { entity_type: EntityType::HappyGhast, more: &[FlyingSpeed, FollowRange, TemptRange], differing: &[base(CameraDistance, 8.0), base(FlyingSpeed, 0.05), base(FollowRange, 16.0), base(MovementSpeed, 0.05), base(TemptRange, 16.0)] },
        TypeRow { entity_type: EntityType::Hoglin, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 6.0), base(AttackKnockback, 1.0), base(FollowRange, 16.0), base(KnockbackResistance, 0.6_f32 as f64), base(MaxHealth, 40.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Horse, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(JumpStrength, 0.7), base(MaxHealth, 53.0), base(MovementSpeed, 0.225_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Husk, more: &[AttackDamage, FollowRange, SpawnReinforcements], differing: &[base(Armor, 2.0), base(AttackDamage, 3.0), base(FollowRange, 35.0), base(MovementSpeed, 0.23_f32 as f64)] },
        TypeRow { entity_type: EntityType::Illusioner, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 18.0), base(MaxHealth, 32.0), base(MovementSpeed, 0.5)] },
        TypeRow { entity_type: EntityType::IronGolem, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 15.0), base(FollowRange, 16.0), base(KnockbackResistance, 1.0), base(MaxHealth, 100.0), base(MovementSpeed, 0.25), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Llama, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(JumpStrength, 0.5), base(MaxHealth, 53.0), base(MovementSpeed, 0.175_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::MagmaCube, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MovementSpeed, 0.2_f32 as f64)] },
        TypeRow { entity_type: EntityType::Mannequin, more: &[], differing: &[] },
        TypeRow { entity_type: EntityType::Mooshroom, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 0.2_f32 as f64)] },
        TypeRow { entity_type: EntityType::Mule, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(JumpStrength, 0.5), base(MaxHealth, 53.0), base(MovementSpeed, 0.175_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Nautilus, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 3.0), base(FollowRange, 16.0), base(KnockbackResistance, 0.3_f32 as f64), base(MaxHealth, 15.0), base(MovementSpeed, 1.0)] },
        TypeRow { entity_type: EntityType::Ocelot, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 3.0), base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Panda, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 6.0), base(FollowRange, 16.0), base(MovementSpeed, 0.15_f32 as f64)] },
        TypeRow { entity_type: EntityType::Parched, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 16.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Parrot, more: &[AttackDamage, FlyingSpeed, FollowRange, TemptRange], differing: &[base(AttackDamage, 3.0), base(FlyingSpeed, 0.4_f32 as f64), base(FollowRange, 16.0), base(MaxHealth, 6.0), base(MovementSpeed, 0.2_f32 as f64)] },
        TypeRow { entity_type: EntityType::Phantom, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0)] },
        TypeRow { entity_type: EntityType::Pig, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 10.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Piglin, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 5.0), base(FollowRange, 16.0), base(MaxHealth, 16.0), base(MovementSpeed, 0.35_f32 as f64)] },
        TypeRow { entity_type: EntityType::PiglinBrute, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 7.0), base(FollowRange, 12.0), base(MaxHealth, 50.0), base(MovementSpeed, 0.35_f32 as f64)] },
        TypeRow { entity_type: EntityType::Pillager, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 5.0), base(MaxHealth, 24.0), base(MovementSpeed, 0.35_f32 as f64)] },
        TypeRow { entity_type: EntityType::Player, more: &[AttackDamage, AttackSpeed, BlockBreakSpeed, BlockInteractionRange, Luck, MiningEfficiency, SneakingSpeed, SubmergedMiningSpeed, SweepingDamageRatio, WaypointReceiveRange], differing: &[base(AttackDamage, 1.0), base(MovementSpeed, 0.1_f32 as f64), base(WaypointTransmitRange, 60000000.0), base(WaypointReceiveRange, 60000000.0)] },
        TypeRow { entity_type: EntityType::PolarBear, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 6.0), base(FollowRange, 20.0), base(MaxHealth, 30.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Pufferfish, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 3.0)] },
        TypeRow { entity_type: EntityType::Rabbit, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 3.0), base(FollowRange, 16.0), base(MaxHealth, 3.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Ravager, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 12.0), base(AttackKnockback, 1.5), base(KnockbackResistance, 0.75), base(MaxHealth, 100.0), base(MovementSpeed, 0.3), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Salmon, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 3.0)] },
        TypeRow { entity_type: EntityType::Sheep, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 8.0), base(MovementSpeed, 0.23_f32 as f64)] },
        TypeRow { entity_type: EntityType::Shulker, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 30.0)] },
        TypeRow { entity_type: EntityType::Silverfish, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 1.0), base(FollowRange, 16.0), base(MaxHealth, 8.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Skeleton, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::SkeletonHorse, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(JumpStrength, 0.7), base(MaxHealth, 15.0), base(MovementSpeed, 0.2_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Slime, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0)] },
        TypeRow { entity_type: EntityType::Sniffer, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 14.0), base(MovementSpeed, 0.1_f32 as f64)] },
        TypeRow { entity_type: EntityType::SnowGolem, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 4.0), base(MovementSpeed, 0.2_f32 as f64)] },
        TypeRow { entity_type: EntityType::Spider, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 16.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Squid, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 10.0)] },
        TypeRow { entity_type: EntityType::Stray, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Strider, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MovementSpeed, 0.175_f32 as f64)] },
        TypeRow { entity_type: EntityType::SulfurCube, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(TemptRange, 8.0)] },
        TypeRow { entity_type: EntityType::Tadpole, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 6.0), base(MovementSpeed, 1.0)] },
        TypeRow { entity_type: EntityType::TraderLlama, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(JumpStrength, 0.5), base(MaxHealth, 53.0), base(MovementSpeed, 0.175_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::TropicalFish, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 3.0)] },
        TypeRow { entity_type: EntityType::Turtle, more: &[FollowRange, TemptRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 30.0), base(MovementSpeed, 0.25), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::Vex, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 4.0), base(FollowRange, 16.0), base(MaxHealth, 14.0)] },
        TypeRow { entity_type: EntityType::Villager, more: &[FollowRange], differing: &[base(FollowRange, 16.0), base(MovementSpeed, 0.5)] },
        TypeRow { entity_type: EntityType::Vindicator, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 5.0), base(FollowRange, 12.0), base(MaxHealth, 24.0), base(MovementSpeed, 0.35_f32 as f64)] },
        TypeRow { entity_type: EntityType::WanderingTrader, more: &[FollowRange], differing: &[base(FollowRange, 16.0)] },
        TypeRow { entity_type: EntityType::Warden, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 30.0), base(AttackKnockback, 1.5), base(FollowRange, 24.0), base(KnockbackResistance, 1.0), base(MaxHealth, 500.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Witch, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MaxHealth, 26.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Wither, more: &[AttackDamage, FlyingSpeed, FollowRange], differing: &[base(Armor, 4.0), base(FlyingSpeed, 0.6_f32 as f64), base(FollowRange, 40.0), base(MaxHealth, 300.0), base(MovementSpeed, 0.6_f32 as f64)] },
        TypeRow { entity_type: EntityType::WitherSkeleton, more: &[AttackDamage, FollowRange], differing: &[base(FollowRange, 16.0), base(MovementSpeed, 0.25)] },
        TypeRow { entity_type: EntityType::Wolf, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 4.0), base(FollowRange, 16.0), base(MaxHealth, 8.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Zoglin, more: &[AttackDamage, FollowRange], differing: &[base(AttackDamage, 6.0), base(AttackKnockback, 1.0), base(FollowRange, 16.0), base(KnockbackResistance, 0.6_f32 as f64), base(MaxHealth, 40.0), base(MovementSpeed, 0.3_f32 as f64)] },
        TypeRow { entity_type: EntityType::Zombie, more: &[AttackDamage, FollowRange, SpawnReinforcements], differing: &[base(Armor, 2.0), base(AttackDamage, 3.0), base(FollowRange, 35.0), base(MovementSpeed, 0.23_f32 as f64)] },
        TypeRow { entity_type: EntityType::ZombieHorse, more: &[FollowRange, TemptRange], differing: &[base(FallDamageMultiplier, 0.5), base(FollowRange, 16.0), base(JumpStrength, 0.7), base(MaxHealth, 25.0), base(MovementSpeed, 0.225_f32 as f64), base(SafeFallDistance, 6.0), base(StepHeight, 1.0)] },
        TypeRow { entity_type: EntityType::ZombieNautilus, more: &[AttackDamage, FollowRange, TemptRange], differing: &[base(AttackDamage, 3.0), base(FollowRange, 16.0), base(KnockbackResistance, 0.3_f32 as f64), base(MaxHealth, 15.0), base(MovementSpeed, 1.1_f32 as f64)] },
        TypeRow { entity_type: EntityType::ZombieVillager, more: &[AttackDamage, FollowRange, SpawnReinforcements], differing: &[base(Armor, 2.0), base(AttackDamage, 3.0), base(FollowRange, 35.0), base(MovementSpeed, 0.23_f32 as f64)] },
        TypeRow { entity_type: EntityType::ZombifiedPiglin, more: &[AttackDamage, FollowRange, SpawnReinforcements], differing: &[base(Armor, 2.0), base(AttackDamage, 5.0), base(FollowRange, 35.0), base(MovementSpeed, 0.23_f32 as f64)] },
    ]
};
