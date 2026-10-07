use std::fmt;

use serde::de::DeserializeSeed;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::*;
use crate::keys::DataComponentType;

/// Every data component: its `DataComponentType` variant, its value type and
/// its flags. Exactly one of `persistent` / `transient`; `unit` for a value
/// with no fields; `ignore_swap_animation`; `nested` when the value embeds a
/// component patch or map; `nbt_wire` when the wire form is one network NBT tag.
#[macro_export]
macro_rules! for_each_data_component {
    ($callback:ident) => {
        $callback! {
        CustomData                   : CustomData               [persistent, nbt_wire],
        MaxStackSize                 : MaxStackSize             [persistent],
        MaxDamage                    : MaxDamage                [persistent],
        Damage                       : Damage                   [persistent, ignore_swap_animation],
        Unbreakable                  : Unbreakable              [persistent, unit],
        UseEffects                   : UseEffects               [persistent],
        CustomName                   : CustomName               [persistent],
        MinimumAttackCharge          : MinimumAttackCharge      [persistent],
        DamageType                   : DamageTypeRef            [persistent],
        ItemName                     : ItemName                 [persistent],
        ItemModel                    : ItemModel                [persistent],
        Lore                         : Lore                     [persistent],
        Rarity                       : Rarity                   [persistent],
        Enchantments                 : Enchantments             [persistent],
        CanPlaceOn                   : CanPlaceOn               [persistent, nested],
        CanBreak                     : CanBreak                 [persistent, nested],
        AttributeModifiers           : AttributeModifiers       [persistent],
        CustomModelData              : CustomModelData          [persistent],
        TooltipDisplay               : TooltipDisplay           [persistent],
        RepairCost                   : RepairCost               [persistent],
        CreativeSlotLock             : CreativeSlotLock         [transient, unit],
        EnchantmentGlintOverride     : EnchantmentGlintOverride [persistent],
        IntangibleProjectile         : IntangibleProjectile     [persistent, unit, nbt_wire],
        Food                         : Food                     [persistent],
        Consumable                   : Consumable               [persistent],
        UseRemainder                 : UseRemainder             [persistent, nested],
        UseCooldown                  : UseCooldown              [persistent],
        DamageResistant              : DamageResistant          [persistent],
        Tool                         : Tool                     [persistent],
        Weapon                       : Weapon                   [persistent],
        AttackRange                  : AttackRange              [persistent],
        Enchantable                  : Enchantable              [persistent],
        Equippable                   : Equippable               [persistent],
        Repairable                   : Repairable               [persistent],
        Glider                       : Glider                   [persistent, unit],
        TooltipStyle                 : TooltipStyle             [persistent],
        DeathProtection              : DeathProtection          [persistent],
        BlocksAttacks                : BlocksAttacks            [persistent],
        PiercingWeapon               : PiercingWeapon           [persistent],
        KineticWeapon                : KineticWeapon            [persistent],
        AttackAnimation              : AttackAnimation          [persistent],
        InteractAnimation            : InteractAnimation        [persistent],
        AdditionalTradeCost          : AdditionalTradeCost      [transient],
        BlockTransformer             : BlockTransformerRef      [persistent],
        VillagerFood                 : VillagerFood             [persistent],
        StoredEnchantments           : StoredEnchantments       [persistent],
        Dye                          : Dye                      [persistent],
        DyedColor                    : DyedColor                [persistent],
        MapId                        : MapId                    [persistent],
        MapDecorations               : MapDecorations           [persistent, nbt_wire],
        MapPostProcessing            : MapPostProcessing        [transient],
        ChargedProjectiles           : ChargedProjectiles       [persistent, nested],
        BundleContents               : BundleContents           [persistent, nested],
        PotionContents               : PotionContents           [persistent],
        PotionDurationScale          : PotionDurationScale      [persistent],
        SuspiciousStewEffects        : SuspiciousStewEffects    [persistent],
        WritableBookContent          : WritableBookContent      [persistent],
        WrittenBookContent           : WrittenBookContent       [persistent],
        Trim                         : Trim                     [persistent],
        DebugStickState              : DebugStickState          [persistent, nbt_wire],
        EntityData                   : EntityData               [persistent],
        BucketEntityData             : BucketEntityData         [persistent],
        BlockEntityData              : BlockEntityData          [persistent],
        Instrument                   : Instrument               [persistent],
        ProvidesTrimMaterial         : ProvidesTrimMaterial     [persistent],
        OminousBottleAmplifier       : OminousBottleAmplifier   [persistent],
        JukeboxPlayable              : JukeboxPlayable          [persistent],
        ProvidesBannerPatterns       : ProvidesBannerPatterns   [persistent],
        Recipes                      : Recipes                  [persistent, nbt_wire],
        LodestoneTracker             : LodestoneTracker         [persistent],
        FireworkExplosion            : FireworkExplosion        [persistent],
        Fireworks                    : Fireworks                [persistent],
        Profile                      : Profile                  [persistent],
        NoteBlockSound               : NoteBlockSound           [persistent],
        BannerPatterns               : BannerPatterns           [persistent],
        BaseColor                    : BaseColor                [persistent],
        PotDecorations               : PotDecorations           [persistent, nested],
        Container                    : Container                [persistent, nested],
        BlockState                   : BlockState               [persistent],
        Bees                         : Bees                     [persistent],
        SulfurCubeContent            : SulfurCubeContent        [persistent, nested],
        Lock                         : Lock                     [persistent, nested, nbt_wire],
        ContainerLoot                : ContainerLoot            [persistent, nbt_wire],
        BreakSound                   : BreakSound               [persistent],
        Compostable                  : Compostable              [persistent],
        CookingFuel                  : CookingFuel              [persistent],
        BrewingFuel                  : BrewingFuel              [persistent],
        MobVisibility                : MobVisibility            [persistent],
        VillagerVariant              : VillagerVariant          [persistent],
        WolfVariant                  : WolfVariant              [persistent],
        WolfSoundVariant             : WolfSoundVariant         [persistent],
        WolfCollar                   : WolfCollar               [persistent],
        FoxVariant                   : FoxVariant               [persistent],
        SalmonSize                   : SalmonSize               [persistent],
        ParrotVariant                : ParrotVariant            [persistent],
        TropicalFishPattern          : TropicalFishPattern      [persistent],
        TropicalFishBaseColor        : TropicalFishBaseColor    [persistent],
        TropicalFishPatternColor     : TropicalFishPatternColor [persistent],
        MooshroomVariant             : MooshroomVariant         [persistent],
        RabbitVariant                : RabbitVariant            [persistent],
        PigVariant                   : PigVariant               [persistent],
        PigSoundVariant              : PigSoundVariant          [persistent],
        CowVariant                   : CowVariant               [persistent],
        CowSoundVariant              : CowSoundVariant          [persistent],
        ChickenVariant               : ChickenVariant           [persistent],
        ChickenSoundVariant          : ChickenSoundVariant      [persistent],
        ZombieNautilusVariant        : ZombieNautilusVariant    [persistent],
        FrogVariant                  : FrogVariant              [persistent],
        HorseVariant                 : HorseVariant             [persistent],
        PaintingVariant              : PaintingVariant          [persistent],
        LlamaVariant                 : LlamaVariant             [persistent],
        AxolotlVariant               : AxolotlVariant           [persistent],
        CatVariant                   : CatVariant               [persistent],
        CatSoundVariant              : CatSoundVariant          [persistent],
        CatCollar                    : CatCollar                [persistent],
        SheepColor                   : SheepColor               [persistent],
        ShulkerColor                 : ShulkerColor             [persistent],
        ProvidesPotteryPattern       : ProvidesPotteryPattern   [persistent],
        SignTextFront                : SignTextFront            [persistent],
        SignTextBack                 : SignTextBack             [persistent],
        Waxed                        : Waxed                    [persistent, unit],
        CushionColor                 : CushionColor             [persistent],
        }
    };
}

#[allow(non_upper_case_globals)]
mod flag {
    pub(super) const persistent: u8 = 1;
    pub(super) const transient: u8 = 2;
    pub(super) const unit: u8 = 4;
    pub(super) const ignore_swap_animation: u8 = 8;
    pub(super) const nested: u8 = 16;
    pub(super) const nbt_wire: u8 = 32;
}

macro_rules! data_components {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        impl DataComponentType {
            const fn flags(self) -> u8 {
                match self {
                    $(Self::$kind => 0 $(| flag::$flag)*),*
                }
            }
        }

        const _: () = {
            $(
                let flags = DataComponentType::$kind.flags();
                assert!(
                    (flags & (flag::persistent | flag::transient)) == flag::persistent
                        || (flags & (flag::persistent | flag::transient)) == flag::transient,
                    "a data component is either persistent or transient"
                );
            )*
        };

        #[derive(Clone, Debug, PartialEq)]
        pub enum ItemComponentValue {
            $($ty($ty)),*
        }

        impl ItemComponentValue {
            pub fn kind(&self) -> DataComponentType {
                match self {
                    $(Self::$ty(_) => DataComponentType::$kind),*
                }
            }

            pub fn serialize_value<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                if !self.kind().is_persistent() {
                    return Err(S::Error::custom(format_args!(
                        "Encountered transient component {}",
                        self.kind().location()
                    )));
                }
                match self {
                    $(Self::$ty(value) => Serialize::serialize(value, s)),*
                }
            }

            pub fn deserialize_value<'de, D: Deserializer<'de>>(
                kind: DataComponentType,
                d: D,
            ) -> Result<Self, D::Error> {
                match kind {
                    $(DataComponentType::$kind => <$ty as Deserialize<'de>>::deserialize(d).map(Self::$ty)),*
                }
            }
        }

        impl Serialize for ItemComponentValue {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.serialize_value(s)
            }
        }

        impl<'de> DeserializeSeed<'de> for DataComponentType {
            type Value = ItemComponentValue;

            fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                ItemComponentValue::deserialize_value(self, d)
            }
        }

        $(
            impl ItemDataComponent for $ty {
                const KIND: DataComponentType = DataComponentType::$kind;

                fn from_value(value: &ItemComponentValue) -> Option<&Self> {
                    match value {
                        ItemComponentValue::$ty(value) => Some(value),
                        _ => None,
                    }
                }
            }

            impl From<$ty> for ItemComponentValue {
                fn from(value: $ty) -> Self {
                    ItemComponentValue::$ty(value)
                }
            }
        )*
    };
}

for_each_data_component!(data_components);

pub trait ItemDataComponent:
    Clone + PartialEq + fmt::Debug + Into<ItemComponentValue> + Send + Sync + 'static
{
    const KIND: DataComponentType;
    fn from_value(value: &ItemComponentValue) -> Option<&Self>;
}

impl DataComponentType {
    pub const fn is_persistent(self) -> bool {
        self.flags() & flag::persistent != 0
    }

    pub const fn is_unit(self) -> bool {
        self.flags() & flag::unit != 0
    }

    pub const fn ignores_swap_animation(self) -> bool {
        self.flags() & flag::ignore_swap_animation != 0
    }

    pub const fn is_nested(self) -> bool {
        self.flags() & flag::nested != 0
    }

    pub const fn is_nbt_wire(self) -> bool {
        self.flags() & flag::nbt_wire != 0
    }
}

/// Vanilla names the parsed identifier, so a bare path is reported with its
/// default namespace.
pub fn unknown_component_error(id: &str) -> String {
    let namespace = if id.contains(':') { "" } else { "minecraft:" };
    format!("No component with type: '{namespace}{id}'")
}
