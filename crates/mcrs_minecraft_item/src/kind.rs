use std::fmt;

use mcrs_minecraft_registries::{
    FromWire, Named, Namer, Refs, Resolve, ResolveError, Resolved, Resolver, Wire, WireNames,
};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::*;
use crate::data_component_type_keys::DataComponentType;
use mcrs_minecraft_wire::{Decode, Encode, Result};
use std::io::Write;

#[macro_export]
macro_rules! for_each_data_component {
    ($callback:ident) => {
        $callback! {
        CustomData                   : CustomData               [nbt_wire],
        MaxStackSize                 : MaxStackSize             [],
        MaxDamage                    : MaxDamage                [],
        Damage                       : Damage                   [],
        Unbreakable                  : Unbreakable              [],
        UseEffects                   : UseEffects               [],
        CustomName                   : CustomName               [refs],
        MinimumAttackCharge          : MinimumAttackCharge      [],
        DamageType                   : DamageTypeRef            [refs],
        ItemName                     : ItemName                 [refs],
        ItemModel                    : ItemModel                [],
        Lore                         : Lore                     [refs],
        Rarity                       : Rarity                   [],
        Enchantments                 : Enchantments             [refs],
        CanPlaceOn                   : CanPlaceOn               [refs],
        CanBreak                     : CanBreak                 [refs],
        AttributeModifiers           : AttributeModifiers       [refs],
        CustomModelData              : CustomModelData          [],
        TooltipDisplay               : TooltipDisplay           [],
        RepairCost                   : RepairCost               [],
        CreativeSlotLock             : CreativeSlotLock         [],
        EnchantmentGlintOverride     : EnchantmentGlintOverride [],
        IntangibleProjectile         : IntangibleProjectile     [nbt_wire],
        Food                         : Food                     [],
        Consumable                   : Consumable               [refs],
        UseRemainder                 : UseRemainder             [refs],
        UseCooldown                  : UseCooldown              [],
        DamageResistant              : DamageResistant          [refs],
        Tool                         : Tool                     [refs],
        Weapon                       : Weapon                   [],
        AttackRange                  : AttackRange              [],
        Enchantable                  : Enchantable              [],
        Equippable                   : Equippable               [refs],
        Repairable                   : Repairable               [refs],
        Glider                       : Glider                   [],
        TooltipStyle                 : TooltipStyle             [],
        DeathProtection              : DeathProtection          [refs],
        BlocksAttacks                : BlocksAttacks            [refs],
        PiercingWeapon               : PiercingWeapon           [refs],
        KineticWeapon                : KineticWeapon            [refs],
        AttackAnimation              : AttackAnimation          [],
        InteractAnimation            : InteractAnimation        [],
        AdditionalTradeCost          : AdditionalTradeCost      [],
        BlockTransformer             : BlockTransformerRef      [refs],
        VillagerFood                 : VillagerFood             [],
        StoredEnchantments           : StoredEnchantments       [refs],
        Dye                          : Dye                      [],
        DyedColor                    : DyedColor                [],
        MapId                        : MapId                    [],
        MapDecorations               : MapDecorations           [nbt_wire],
        MapPostProcessing            : MapPostProcessing        [],
        ChargedProjectiles           : ChargedProjectiles       [refs],
        BundleContents               : BundleContents           [refs],
        PotionContents               : PotionContents           [refs],
        PotionDurationScale          : PotionDurationScale      [],
        SuspiciousStewEffects        : SuspiciousStewEffects    [refs],
        WritableBookContent          : WritableBookContent      [],
        WrittenBookContent           : WrittenBookContent       [refs],
        Trim                         : Trim                     [refs],
        DebugStickState              : DebugStickState          [nbt_wire],
        EntityData                   : EntityData               [],
        BucketEntityData             : BucketEntityData         [],
        BlockEntityData              : BlockEntityData          [],
        Instrument                   : Instrument               [refs],
        ProvidesTrimMaterial         : ProvidesTrimMaterial     [refs],
        OminousBottleAmplifier       : OminousBottleAmplifier   [],
        JukeboxPlayable              : JukeboxPlayable          [refs],
        ProvidesBannerPatterns       : ProvidesBannerPatterns   [refs],
        Recipes                      : Recipes                  [nbt_wire],
        LodestoneTracker             : LodestoneTracker         [],
        FireworkExplosion            : FireworkExplosion        [],
        Fireworks                    : Fireworks                [],
        Profile                      : Profile                  [],
        NoteBlockSound               : NoteBlockSound           [],
        BannerPatterns               : BannerPatterns           [refs],
        BaseColor                    : BaseColor                [],
        PotDecorations               : PotDecorations           [refs],
        Container                    : Container                [refs],
        BlockState                   : BlockState               [],
        Bees                         : Bees                     [],
        SulfurCubeContent            : SulfurCubeContent        [refs],
        Lock                         : Lock                     [nbt_wire, refs],
        ContainerLoot                : ContainerLoot            [],
        BreakSound                   : BreakSound               [refs],
        Compostable                  : Compostable              [],
        CookingFuel                  : CookingFuel              [],
        BrewingFuel                  : BrewingFuel              [],
        MobVisibility                : MobVisibility            [refs],
        VillagerVariant              : VillagerVariant          [refs],
        WolfVariant                  : WolfVariant              [refs],
        WolfSoundVariant             : WolfSoundVariant         [refs],
        WolfCollar                   : WolfCollar               [],
        FoxVariant                   : FoxVariant               [],
        SalmonSize                   : SalmonSize               [],
        ParrotVariant                : ParrotVariant            [],
        TropicalFishPattern          : TropicalFishPattern      [],
        TropicalFishBaseColor        : TropicalFishBaseColor    [],
        TropicalFishPatternColor     : TropicalFishPatternColor [],
        MooshroomVariant             : MooshroomVariant         [],
        RabbitVariant                : RabbitVariant            [],
        PigVariant                   : PigVariant               [refs],
        PigSoundVariant              : PigSoundVariant          [refs],
        CowVariant                   : CowVariant               [refs],
        CowSoundVariant              : CowSoundVariant          [refs],
        ChickenVariant               : ChickenVariant           [refs],
        ChickenSoundVariant          : ChickenSoundVariant      [refs],
        ZombieNautilusVariant        : ZombieNautilusVariant    [refs],
        FrogVariant                  : FrogVariant              [refs],
        HorseVariant                 : HorseVariant             [],
        PaintingVariant              : PaintingVariant          [refs],
        LlamaVariant                 : LlamaVariant             [],
        AxolotlVariant               : AxolotlVariant           [],
        CatVariant                   : CatVariant               [refs],
        CatSoundVariant              : CatSoundVariant          [refs],
        CatCollar                    : CatCollar                [],
        SheepColor                   : SheepColor               [],
        ShulkerColor                 : ShulkerColor             [],
        ProvidesPotteryPattern       : ProvidesPotteryPattern   [refs],
        SignTextFront                : SignTextFront            [refs],
        SignTextBack                 : SignTextBack             [refs],
        Waxed                        : Waxed                    [],
        CushionColor                 : CushionColor             [],
        }
    };
}

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

pub trait Moded {
    type In<M: Refs>: Clone + PartialEq + fmt::Debug + Send + Sync + 'static;
}

pub trait ItemDataComponent:
    Clone + PartialEq + fmt::Debug + Into<ItemComponentValue> + Send + Sync + 'static
{
    const KIND: DataComponentType;
    fn from_value(value: &ItemComponentValue) -> Option<&Self>;
}

macro_rules! if_nbt_wire {
    ([] $then:tt {$($else:tt)*}) => { $($else)* };
    ([nbt_wire $(, $rest:ident)*] {$($then:tt)*} $else:tt) => { $($then)* };
    ([$first:ident $(, $rest:ident)*] $then:tt $else:tt) => {
        if_nbt_wire!([$($rest),*] $then $else)
    };
}

macro_rules! value_wire {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        const _: () = {
            $(
                assert!(
                    DataComponentType::$kind.is_nbt_wire()
                        == if_nbt_wire!([$($flag),*] {true} {false}),
                    "the nbt_wire marker of the item table agrees with the component type"
                );
            )*
        };

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
