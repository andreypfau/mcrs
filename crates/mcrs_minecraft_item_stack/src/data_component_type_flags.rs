use crate::data_component_type_keys::DataComponentType;

const PERSISTENT: u8 = 1;
const TRANSIENT: u8 = 2;
const UNIT: u8 = 4;
const IGNORE_SWAP_ANIMATION: u8 = 8;
const NESTED: u8 = 16;
const NBT_WIRE: u8 = 32;

const _: () = {
    let mut rest = DataComponentType::ALL;
    while let [kind, tail @ ..] = rest {
        let persistence = kind.flags() & (PERSISTENT | TRANSIENT);
        assert!(
            persistence == PERSISTENT || persistence == TRANSIENT,
            "a data component is either persistent or transient"
        );
        rest = tail;
    }
};

impl DataComponentType {
    #[expect(
        clippy::too_many_lines,
        reason = "one flat match arm for each component type of the registry"
    )]
    const fn flags(self) -> u8 {
        match self {
            Self::CustomData => PERSISTENT | NBT_WIRE,
            Self::MaxStackSize => PERSISTENT,
            Self::MaxDamage => PERSISTENT,
            Self::Damage => PERSISTENT | IGNORE_SWAP_ANIMATION,
            Self::Unbreakable => PERSISTENT | UNIT,
            Self::UseEffects => PERSISTENT,
            Self::CustomName => PERSISTENT,
            Self::MinimumAttackCharge => PERSISTENT,
            Self::DamageType => PERSISTENT,
            Self::ItemName => PERSISTENT,
            Self::ItemModel => PERSISTENT,
            Self::Lore => PERSISTENT,
            Self::Rarity => PERSISTENT,
            Self::Enchantments => PERSISTENT,
            Self::CanPlaceOn => PERSISTENT | NESTED,
            Self::CanBreak => PERSISTENT | NESTED,
            Self::AttributeModifiers => PERSISTENT,
            Self::CustomModelData => PERSISTENT,
            Self::TooltipDisplay => PERSISTENT,
            Self::RepairCost => PERSISTENT,
            Self::CreativeSlotLock => TRANSIENT | UNIT,
            Self::EnchantmentGlintOverride => PERSISTENT,
            Self::IntangibleProjectile => PERSISTENT | UNIT | NBT_WIRE,
            Self::Food => PERSISTENT,
            Self::Consumable => PERSISTENT,
            Self::UseRemainder => PERSISTENT | NESTED,
            Self::UseCooldown => PERSISTENT,
            Self::DamageResistant => PERSISTENT,
            Self::Tool => PERSISTENT,
            Self::Weapon => PERSISTENT,
            Self::AttackRange => PERSISTENT,
            Self::Enchantable => PERSISTENT,
            Self::Equippable => PERSISTENT,
            Self::Repairable => PERSISTENT,
            Self::Glider => PERSISTENT | UNIT,
            Self::TooltipStyle => PERSISTENT,
            Self::DeathProtection => PERSISTENT,
            Self::BlocksAttacks => PERSISTENT,
            Self::PiercingWeapon => PERSISTENT,
            Self::KineticWeapon => PERSISTENT,
            Self::AttackAnimation => PERSISTENT,
            Self::InteractAnimation => PERSISTENT,
            Self::AdditionalTradeCost => TRANSIENT,
            Self::BlockTransformer => PERSISTENT,
            Self::VillagerFood => PERSISTENT,
            Self::StoredEnchantments => PERSISTENT,
            Self::Dye => PERSISTENT,
            Self::DyedColor => PERSISTENT,
            Self::MapId => PERSISTENT,
            Self::MapDecorations => PERSISTENT | NBT_WIRE,
            Self::MapPostProcessing => TRANSIENT,
            Self::ChargedProjectiles => PERSISTENT | NESTED,
            Self::BundleContents => PERSISTENT | NESTED,
            Self::PotionContents => PERSISTENT,
            Self::PotionDurationScale => PERSISTENT,
            Self::SuspiciousStewEffects => PERSISTENT,
            Self::WritableBookContent => PERSISTENT,
            Self::WrittenBookContent => PERSISTENT,
            Self::Trim => PERSISTENT,
            Self::DebugStickState => PERSISTENT | NBT_WIRE,
            Self::EntityData => PERSISTENT,
            Self::BucketEntityData => PERSISTENT,
            Self::BlockEntityData => PERSISTENT,
            Self::Instrument => PERSISTENT,
            Self::ProvidesTrimMaterial => PERSISTENT,
            Self::OminousBottleAmplifier => PERSISTENT,
            Self::JukeboxPlayable => PERSISTENT,
            Self::ProvidesBannerPatterns => PERSISTENT,
            Self::Recipes => PERSISTENT | NBT_WIRE,
            Self::LodestoneTracker => PERSISTENT,
            Self::FireworkExplosion => PERSISTENT,
            Self::Fireworks => PERSISTENT,
            Self::Profile => PERSISTENT,
            Self::NoteBlockSound => PERSISTENT,
            Self::BannerPatterns => PERSISTENT,
            Self::BaseColor => PERSISTENT,
            Self::PotDecorations => PERSISTENT | NESTED,
            Self::Container => PERSISTENT | NESTED,
            Self::BlockState => PERSISTENT,
            Self::Bees => PERSISTENT,
            Self::SulfurCubeContent => PERSISTENT | NESTED,
            Self::Lock => PERSISTENT | NESTED | NBT_WIRE,
            Self::ContainerLoot => PERSISTENT,
            Self::BreakSound => PERSISTENT,
            Self::Compostable => PERSISTENT,
            Self::CookingFuel => PERSISTENT,
            Self::BrewingFuel => PERSISTENT,
            Self::MobVisibility => PERSISTENT,
            Self::VillagerVariant => PERSISTENT,
            Self::WolfVariant => PERSISTENT,
            Self::WolfSoundVariant => PERSISTENT,
            Self::WolfCollar => PERSISTENT,
            Self::FoxVariant => PERSISTENT,
            Self::SalmonSize => PERSISTENT,
            Self::ParrotVariant => PERSISTENT,
            Self::TropicalFishPattern => PERSISTENT,
            Self::TropicalFishBaseColor => PERSISTENT,
            Self::TropicalFishPatternColor => PERSISTENT,
            Self::MooshroomVariant => PERSISTENT,
            Self::RabbitVariant => PERSISTENT,
            Self::PigVariant => PERSISTENT,
            Self::PigSoundVariant => PERSISTENT,
            Self::CowVariant => PERSISTENT,
            Self::CowSoundVariant => PERSISTENT,
            Self::ChickenVariant => PERSISTENT,
            Self::ChickenSoundVariant => PERSISTENT,
            Self::ZombieNautilusVariant => PERSISTENT,
            Self::FrogVariant => PERSISTENT,
            Self::HorseVariant => PERSISTENT,
            Self::PaintingVariant => PERSISTENT,
            Self::LlamaVariant => PERSISTENT,
            Self::AxolotlVariant => PERSISTENT,
            Self::CatVariant => PERSISTENT,
            Self::CatSoundVariant => PERSISTENT,
            Self::CatCollar => PERSISTENT,
            Self::SheepColor => PERSISTENT,
            Self::ShulkerColor => PERSISTENT,
            Self::ProvidesPotteryPattern => PERSISTENT,
            Self::SignTextFront => PERSISTENT,
            Self::SignTextBack => PERSISTENT,
            Self::Waxed => PERSISTENT | UNIT,
            Self::CushionColor => PERSISTENT,
        }
    }

    #[must_use]
    pub const fn is_persistent(self) -> bool {
        self.flags() & PERSISTENT != 0
    }

    #[must_use]
    pub const fn is_unit(self) -> bool {
        self.flags() & UNIT != 0
    }

    #[must_use]
    pub const fn ignores_swap_animation(self) -> bool {
        self.flags() & IGNORE_SWAP_ANIMATION != 0
    }

    #[must_use]
    pub const fn is_nested(self) -> bool {
        self.flags() & NESTED != 0
    }

    #[must_use]
    pub const fn is_nbt_wire(self) -> bool {
        self.flags() & NBT_WIRE != 0
    }
}
