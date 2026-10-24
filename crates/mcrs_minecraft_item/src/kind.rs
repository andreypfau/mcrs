use std::fmt;

use mcrs_minecraft_registries::{
    FromWire, Named, Namer, Refs, Resolve, ResolveError, Resolved, Resolver, Wire, WireNames,
};
use serde::de::DeserializeSeed;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::*;
use crate::keys::DataComponentType;
use mcrs_minecraft_wire::{Decode, Encode, Result};
use std::io::Write;

/// Every data component: its `DataComponentType` variant, its value type and
/// its flags. Exactly one of `persistent` / `transient`; `unit` for a value
/// with no fields; `ignore_swap_animation`; `nested` when the value embeds a
/// component patch or map; `nbt_wire` when the wire form is one network NBT tag;
/// `refs` when the value type takes the reference mode `M: Refs`.
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
        CustomName                   : CustomName               [persistent, refs],
        MinimumAttackCharge          : MinimumAttackCharge      [persistent],
        DamageType                   : DamageTypeRef            [persistent, refs],
        ItemName                     : ItemName                 [persistent, refs],
        ItemModel                    : ItemModel                [persistent],
        Lore                         : Lore                     [persistent, refs],
        Rarity                       : Rarity                   [persistent],
        Enchantments                 : Enchantments             [persistent, refs],
        CanPlaceOn                   : CanPlaceOn               [persistent, nested, refs],
        CanBreak                     : CanBreak                 [persistent, nested, refs],
        AttributeModifiers           : AttributeModifiers       [persistent, refs],
        CustomModelData              : CustomModelData          [persistent],
        TooltipDisplay               : TooltipDisplay           [persistent],
        RepairCost                   : RepairCost               [persistent],
        CreativeSlotLock             : CreativeSlotLock         [transient, unit],
        EnchantmentGlintOverride     : EnchantmentGlintOverride [persistent],
        IntangibleProjectile         : IntangibleProjectile     [persistent, unit, nbt_wire],
        Food                         : Food                     [persistent],
        Consumable                   : Consumable               [persistent, refs],
        UseRemainder                 : UseRemainder             [persistent, nested, refs],
        UseCooldown                  : UseCooldown              [persistent],
        DamageResistant              : DamageResistant          [persistent, refs],
        Tool                         : Tool                     [persistent, refs],
        Weapon                       : Weapon                   [persistent],
        AttackRange                  : AttackRange              [persistent],
        Enchantable                  : Enchantable              [persistent],
        Equippable                   : Equippable               [persistent, refs],
        Repairable                   : Repairable               [persistent, refs],
        Glider                       : Glider                   [persistent, unit],
        TooltipStyle                 : TooltipStyle             [persistent],
        DeathProtection              : DeathProtection          [persistent, refs],
        BlocksAttacks                : BlocksAttacks            [persistent, refs],
        PiercingWeapon               : PiercingWeapon           [persistent, refs],
        KineticWeapon                : KineticWeapon            [persistent, refs],
        AttackAnimation              : AttackAnimation          [persistent],
        InteractAnimation            : InteractAnimation        [persistent],
        AdditionalTradeCost          : AdditionalTradeCost      [transient],
        BlockTransformer             : BlockTransformerRef      [persistent, refs],
        VillagerFood                 : VillagerFood             [persistent],
        StoredEnchantments           : StoredEnchantments       [persistent, refs],
        Dye                          : Dye                      [persistent],
        DyedColor                    : DyedColor                [persistent],
        MapId                        : MapId                    [persistent],
        MapDecorations               : MapDecorations           [persistent, nbt_wire],
        MapPostProcessing            : MapPostProcessing        [transient],
        ChargedProjectiles           : ChargedProjectiles       [persistent, nested, refs],
        BundleContents               : BundleContents           [persistent, nested, refs],
        PotionContents               : PotionContents           [persistent, refs],
        PotionDurationScale          : PotionDurationScale      [persistent],
        SuspiciousStewEffects        : SuspiciousStewEffects    [persistent, refs],
        WritableBookContent          : WritableBookContent      [persistent],
        WrittenBookContent           : WrittenBookContent       [persistent, refs],
        Trim                         : Trim                     [persistent, refs],
        DebugStickState              : DebugStickState          [persistent, nbt_wire],
        EntityData                   : EntityData               [persistent],
        BucketEntityData             : BucketEntityData         [persistent],
        BlockEntityData              : BlockEntityData          [persistent],
        Instrument                   : Instrument               [persistent, refs],
        ProvidesTrimMaterial         : ProvidesTrimMaterial     [persistent, refs],
        OminousBottleAmplifier       : OminousBottleAmplifier   [persistent],
        JukeboxPlayable              : JukeboxPlayable          [persistent, refs],
        ProvidesBannerPatterns       : ProvidesBannerPatterns   [persistent, refs],
        Recipes                      : Recipes                  [persistent, nbt_wire],
        LodestoneTracker             : LodestoneTracker         [persistent],
        FireworkExplosion            : FireworkExplosion        [persistent],
        Fireworks                    : Fireworks                [persistent],
        Profile                      : Profile                  [persistent],
        NoteBlockSound               : NoteBlockSound           [persistent],
        BannerPatterns               : BannerPatterns           [persistent, refs],
        BaseColor                    : BaseColor                [persistent],
        PotDecorations               : PotDecorations           [persistent, nested, refs],
        Container                    : Container                [persistent, nested, refs],
        BlockState                   : BlockState               [persistent],
        Bees                         : Bees                     [persistent],
        SulfurCubeContent            : SulfurCubeContent        [persistent, nested, refs],
        Lock                         : Lock                     [persistent, nested, nbt_wire, refs],
        ContainerLoot                : ContainerLoot            [persistent, nbt_wire],
        BreakSound                   : BreakSound               [persistent, refs],
        Compostable                  : Compostable              [persistent],
        CookingFuel                  : CookingFuel              [persistent],
        BrewingFuel                  : BrewingFuel              [persistent],
        MobVisibility                : MobVisibility            [persistent, refs],
        VillagerVariant              : VillagerVariant          [persistent, refs],
        WolfVariant                  : WolfVariant              [persistent, refs],
        WolfSoundVariant             : WolfSoundVariant         [persistent, refs],
        WolfCollar                   : WolfCollar               [persistent],
        FoxVariant                   : FoxVariant               [persistent],
        SalmonSize                   : SalmonSize               [persistent],
        ParrotVariant                : ParrotVariant            [persistent],
        TropicalFishPattern          : TropicalFishPattern      [persistent],
        TropicalFishBaseColor        : TropicalFishBaseColor    [persistent],
        TropicalFishPatternColor     : TropicalFishPatternColor [persistent],
        MooshroomVariant             : MooshroomVariant         [persistent],
        RabbitVariant                : RabbitVariant            [persistent],
        PigVariant                   : PigVariant               [persistent, refs],
        PigSoundVariant              : PigSoundVariant          [persistent, refs],
        CowVariant                   : CowVariant               [persistent, refs],
        CowSoundVariant              : CowSoundVariant          [persistent, refs],
        ChickenVariant               : ChickenVariant           [persistent, refs],
        ChickenSoundVariant          : ChickenSoundVariant      [persistent, refs],
        ZombieNautilusVariant        : ZombieNautilusVariant    [persistent, refs],
        FrogVariant                  : FrogVariant              [persistent, refs],
        HorseVariant                 : HorseVariant             [persistent],
        PaintingVariant              : PaintingVariant          [persistent, refs],
        LlamaVariant                 : LlamaVariant             [persistent],
        AxolotlVariant               : AxolotlVariant           [persistent],
        CatVariant                   : CatVariant               [persistent, refs],
        CatSoundVariant              : CatSoundVariant          [persistent, refs],
        CatCollar                    : CatCollar                [persistent],
        SheepColor                   : SheepColor               [persistent],
        ShulkerColor                 : ShulkerColor             [persistent],
        ProvidesPotteryPattern       : ProvidesPotteryPattern   [persistent, refs],
        SignTextFront                : SignTextFront            [persistent, refs],
        SignTextBack                 : SignTextBack             [persistent, refs],
        Waxed                        : Waxed                    [persistent, unit],
        CushionColor                 : CushionColor             [persistent],
        }
    };
}

#[expect(
    non_upper_case_globals,
    reason = "each data_components! row names its flags in lower case and the macro splices them in as identifiers"
)]
mod flag {
    pub(super) const persistent: u8 = 1;
    pub(super) const transient: u8 = 2;
    pub(super) const unit: u8 = 4;
    pub(super) const ignore_swap_animation: u8 = 8;
    pub(super) const nested: u8 = 16;
    pub(super) const nbt_wire: u8 = 32;
    pub(super) const refs: u8 = 64;
}

/// Expands to the first group when the flag list holds `refs`, else to the second.
#[macro_export]
#[doc(hidden)]
macro_rules! if_refs {
    ([] $then:tt {$($else:tt)*}) => { $($else)* };
    ([refs $(, $rest:ident)*] {$($then:tt)*} $else:tt) => { $($then)* };
    ([$first:ident $(, $rest:ident)*] $then:tt $else:tt) => {
        $crate::if_refs!([$($rest),*] $then $else)
    };
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

        $(
            impl Moded for $ty {
                type In<M: Refs> = $crate::if_refs!([$($flag),*] {$ty<M>} {$ty});
            }
        )*

        #[derive(Clone, Debug, PartialEq)]
        pub enum ItemComponentValue<M: Refs = Resolved> {
            $($ty(<$ty as Moded>::In<M>)),*
        }

        impl<M: Refs> ItemComponentValue<M> {
            pub fn kind(&self) -> DataComponentType {
                match self {
                    $(Self::$ty(_) => DataComponentType::$kind),*
                }
            }
        }

        impl ItemComponentValue<Named> {
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
                    $(DataComponentType::$kind => Deserialize::deserialize(d).map(Self::$ty)),*
                }
            }
        }

        impl Serialize for ItemComponentValue<Named> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.serialize_value(s)
            }
        }

        impl<'de> DeserializeSeed<'de> for DataComponentType {
            type Value = ItemComponentValue<Named>;

            fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                ItemComponentValue::deserialize_value(self, d)
            }
        }

        impl Resolve for ItemComponentValue {
            type Named = ItemComponentValue<Named>;

            fn resolve<C: Resolver>(named: Self::Named, ctx: &C) -> Result<Self, ResolveError> {
                let _ = ctx;
                Ok(match named {
                    $(ItemComponentValue::<Named>::$ty(value) => Self::$ty($crate::if_refs!(
                        [$($flag),*]
                        {Resolve::resolve(value, ctx)
                            .map_err(|e| e.at(DataComponentType::$kind.as_static_str()))?}
                        {value}
                    ))),*
                })
            }

            fn name<C: Namer>(&self, names: &C) -> Result<Self::Named, ResolveError> {
                let _ = names;
                Ok(match self {
                    $(Self::$ty(value) => ItemComponentValue::<Named>::$ty($crate::if_refs!(
                        [$($flag),*]
                        {value
                            .name(names)
                            .map_err(|e| e.at(DataComponentType::$kind.as_static_str()))?}
                        {value.clone()}
                    ))),*
                })
            }
        }

        impl FromWire for ItemComponentValue {
            type Wire = ItemComponentValue<Wire>;

            fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
                let _ = ctx;
                Ok(match wire {
                    $(ItemComponentValue::<Wire>::$ty(value) => Self::$ty($crate::if_refs!(
                        [$($flag),*]
                        {FromWire::from_wire(value, ctx)
                            .map_err(|e| e.at(DataComponentType::$kind.as_static_str()))?}
                        {value}
                    ))),*
                })
            }

            fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
                let _ = ctx;
                Ok(match self {
                    $(Self::$ty(value) => ItemComponentValue::<Wire>::$ty($crate::if_refs!(
                        [$($flag),*]
                        {value
                            .to_wire(ctx)
                            .map_err(|e| e.at(DataComponentType::$kind.as_static_str()))?}
                        {value.clone()}
                    ))),*
                })
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

/// A component value type in each reference mode: itself when it names no registry entry.
pub trait Moded {
    type In<M: Refs>: Clone + PartialEq + fmt::Debug + Send + Sync + 'static;
}

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

/// Expands to the first group when the flag list holds `nbt_wire`, else to the second.
macro_rules! if_nbt_wire {
    ([] $then:tt {$($else:tt)*}) => { $($else)* };
    ([nbt_wire $(, $rest:ident)*] {$($then:tt)*} $else:tt) => { $($then)* };
    ([$first:ident $(, $rest:ident)*] $then:tt $else:tt) => {
        if_nbt_wire!([$($rest),*] $then $else)
    };
}

macro_rules! value_wire {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        impl Encode for ItemComponentValue<Wire> {
            fn encode(&self, w: impl Write) -> Result<()> {
                match self {
                    $(Self::$ty(value) => if_nbt_wire!([$($flag),*] {
                        mcrs_minecraft_nbt::encode_wire(value, w)
                    } {
                        value.encode(w)
                    }),)*
                }
            }
        }

        /// The value of `kind`, whose wire layout the kind alone selects.
        pub fn decode_component_value(
            kind: DataComponentType,
            r: &mut &[u8],
        ) -> Result<ItemComponentValue<Wire>> {
            Ok(match kind {
                $(DataComponentType::$kind => ItemComponentValue::$ty(if_nbt_wire!([$($flag),*] {
                    mcrs_minecraft_nbt::decode_wire(r)?
                } {
                    Decode::decode(r)?
                })),)*
            })
        }
    };
}

for_each_data_component!(value_wire);
