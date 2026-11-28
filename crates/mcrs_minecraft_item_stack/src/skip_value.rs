use crate::BuiltinKey;
use crate::ComponentType;
use crate::WireError;
use crate::any_names::AnyNames;
use crate::data_component_type_keys::DataComponentType;
use crate::shape_skip::ShapeSkip;
use crate::wire_scan::skip_component_map;
use crate::wire_value::read_value_at;

const F32: usize = 4;
const I32: usize = 4;

#[expect(
    clippy::too_many_lines,
    reason = "one flat match arm for each component type of the registry"
)]
pub(crate) fn skip_value(
    component: DataComponentType,
    input: &mut &[u8],
    depth: u32,
) -> Result<(), WireError> {
    use DataComponentType::*;
    let skip = ShapeSkip {
        component: ComponentType(component.id().number()),
        depth,
    };
    let value =
        |key: BuiltinKey, input: &mut &[u8]| read_value_at(key, input, &AnyNames, depth).map(drop);
    match component {
        CustomData => value(BuiltinKey::CustomData, input),
        MaxStackSize => value(BuiltinKey::MaxStackSize, input),
        MaxDamage => value(BuiltinKey::MaxDamage, input),
        Damage => value(BuiltinKey::Damage, input),
        Unbreakable => value(BuiltinKey::Unbreakable, input),
        UseEffects => {
            skip.boolean(input)?;
            skip.boolean(input)?;
            skip.bytes(input, F32)
        }
        CustomName => value(BuiltinKey::CustomName, input),
        MinimumAttackCharge | PotionDurationScale => skip.bytes(input, F32),
        DamageType
        | BlockTransformer
        | VillagerVariant
        | WolfVariant
        | WolfSoundVariant
        | PigVariant
        | PigSoundVariant
        | CowVariant
        | CowSoundVariant
        | ChickenVariant
        | ChickenSoundVariant
        | ZombieNautilusVariant
        | FrogVariant
        | CatVariant
        | CatSoundVariant
        | ProvidesPotteryPattern
        | Enchantable
        | VillagerFood
        | AdditionalTradeCost
        | MapPostProcessing
        | Dye
        | BaseColor
        | WolfCollar
        | TropicalFishBaseColor
        | TropicalFishPatternColor
        | CatCollar
        | SheepColor
        | ShulkerColor
        | CushionColor
        | FoxVariant
        | AxolotlVariant
        | SalmonSize
        | ParrotVariant
        | MooshroomVariant
        | LlamaVariant
        | HorseVariant
        | RabbitVariant
        | TropicalFishPattern
        | Rarity => skip.var_int(input).map(drop),
        ItemName => value(BuiltinKey::ItemName, input),
        ItemModel => value(BuiltinKey::ItemModel, input),
        Lore => value(BuiltinKey::Lore, input),
        Enchantments => value(BuiltinKey::Enchantments, input),
        CanPlaceOn | CanBreak => skip.list(input, block_predicate),
        AttributeModifiers => value(BuiltinKey::AttributeModifiers, input),
        CustomModelData => value(BuiltinKey::CustomModelData, input),
        TooltipDisplay => value(BuiltinKey::TooltipDisplay, input),
        RepairCost => value(BuiltinKey::RepairCost, input),
        CreativeSlotLock | Glider | Waxed => Ok(()),
        EnchantmentGlintOverride => value(BuiltinKey::EnchantmentGlintOverride, input),
        IntangibleProjectile | DebugStickState | Recipes | Lock | BucketEntityData => {
            skip.nbt(input)
        }
        TooltipStyle | NoteBlockSound => skip.string(input),
        Food => value(BuiltinKey::Food, input),
        Consumable => {
            skip.bytes(input, F32)?;
            skip.var_int(input)?;
            skip.optional(input, sound)?;
            skip.boolean(input)?;
            skip.list(input, consume_effect)
        }
        UseRemainder | SulfurCubeContent => skip.list_template(input),
        UseCooldown => {
            skip.bytes(input, F32)?;
            skip.optional(input, ShapeSkip::string)
        }
        DamageResistant | Repairable | ProvidesBannerPatterns => skip.holder_set(input),
        Tool => value(BuiltinKey::Tool, input),
        Weapon => {
            skip.var_int(input)?;
            skip.bytes(input, F32)
        }
        AttackRange => skip.bytes(input, 6 * F32),
        Equippable => value(BuiltinKey::Equippable, input),
        DeathProtection => skip.list(input, consume_effect),
        BlocksAttacks => {
            skip.bytes(input, 2 * F32)?;
            skip.list(input, damage_reduction)?;
            skip.bytes(input, 3 * F32)?;
            skip.optional(input, ShapeSkip::holder_set)?;
            skip.optional(input, sound)?;
            skip.optional(input, sound)
        }
        PiercingWeapon => value(BuiltinKey::PiercingWeapon, input),
        KineticWeapon => {
            skip.var_int(input)?;
            skip.var_int(input)?;
            for _ in 0..3 {
                skip.optional(input, kinetic_condition)?;
            }
            skip.bytes(input, 2 * F32)?;
            skip.optional(input, sound)?;
            skip.optional(input, sound)
        }
        AttackAnimation | InteractAnimation => {
            skip.var_int(input)?;
            skip.var_int(input).map(drop)
        }
        StoredEnchantments => value(BuiltinKey::StoredEnchantments, input),
        DyedColor => value(BuiltinKey::DyedColor, input),
        MapId => value(BuiltinKey::MapId, input),
        MapDecorations => value(BuiltinKey::MapDecorations, input),
        ChargedProjectiles | BundleContents => skip.list(input, ShapeSkip::list_template),
        PotionContents => value(BuiltinKey::PotionContents, input),
        SuspiciousStewEffects => value(BuiltinKey::SuspiciousStewEffects, input),
        WritableBookContent => skip.list(input, |skip, input| {
            filterable(skip, input, ShapeSkip::string)
        }),
        WrittenBookContent => {
            filterable(skip, input, ShapeSkip::string)?;
            skip.string(input)?;
            skip.var_int(input)?;
            skip.list(input, |skip, input| filterable(skip, input, ShapeSkip::nbt))?;
            skip.boolean(input).map(drop)
        }
        Trim => value(BuiltinKey::Trim, input),
        EntityData | BlockEntityData => {
            skip.var_int(input)?;
            skip.nbt(input)
        }
        Instrument => value(BuiltinKey::Instrument, input),
        ProvidesTrimMaterial => skip.holder(input, |skip, input| {
            skip.string(input)?;
            skip.nbt(input)
        }),
        OminousBottleAmplifier => value(BuiltinKey::OminousBottleAmplifier, input),
        JukeboxPlayable => skip.holder(input, |skip, input| {
            sound(skip, input)?;
            skip.nbt(input)?;
            skip.bytes(input, F32)?;
            skip.var_int(input).map(drop)
        }),
        LodestoneTracker => value(BuiltinKey::LodestoneTracker, input),
        FireworkExplosion => value(BuiltinKey::FireworkExplosion, input),
        Fireworks => value(BuiltinKey::Fireworks, input),
        Profile => value(BuiltinKey::Profile, input),
        BannerPatterns => value(BuiltinKey::BannerPatterns, input),
        PotDecorations => {
            for _ in 0..4 {
                skip.optional(input, ShapeSkip::slot_template)?;
            }
            Ok(())
        }
        Container => skip.list(input, |skip, input| {
            skip.optional(input, ShapeSkip::slot_template)
        }),
        BlockState => value(BuiltinKey::BlockState, input),
        Bees => value(BuiltinKey::Bees, input),
        ContainerLoot => value(BuiltinKey::ContainerLoot, input),
        BreakSound => sound(skip, input),
        Compostable => resolvable(skip, input, I32),
        CookingFuel | BrewingFuel => {
            resolvable(skip, input, I32)?;
            resolvable(skip, input, F32)
        }
        MobVisibility => {
            skip.holder_set(input)?;
            skip.bytes(input, F32)
        }
        PaintingVariant => skip.holder(input, |skip, input| {
            skip.var_int(input)?;
            skip.var_int(input)?;
            skip.string(input)?;
            skip.optional(input, ShapeSkip::nbt)?;
            skip.optional(input, ShapeSkip::nbt)
        }),
        SignTextFront | SignTextBack => {
            sign_messages(skip, input)?;
            skip.optional(input, sign_messages)?;
            skip.var_int(input)?;
            skip.boolean(input).map(drop)
        }
    }
}

fn sound(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    skip.holder(input, |skip, input| {
        skip.string(input)?;
        skip.optional(input, |skip, input| skip.bytes(input, F32))
    })
}

fn filterable(
    skip: ShapeSkip,
    input: &mut &[u8],
    mut value: impl FnMut(ShapeSkip, &mut &[u8]) -> Result<(), WireError>,
) -> Result<(), WireError> {
    value(skip, input)?;
    skip.optional(input, value)
}

fn resolvable(skip: ShapeSkip, input: &mut &[u8], constant: usize) -> Result<(), WireError> {
    if skip.boolean(input)? {
        skip.bytes(input, constant)
    } else {
        skip.string(input)
    }
}

fn sign_messages(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    for _ in 0..4 {
        skip.nbt(input)?;
    }
    Ok(())
}

fn kinetic_condition(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    skip.var_int(input)?;
    skip.bytes(input, 2 * F32)
}

fn damage_reduction(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    skip.bytes(input, F32)?;
    skip.optional(input, ShapeSkip::holder_set)?;
    skip.bytes(input, 2 * F32)
}

fn consume_effect(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    match skip.var_int(input)? {
        0 => {
            skip.list(input, |skip, input| {
                skip.var_int(input)?;
                effect_details(skip, input)
            })?;
            skip.bytes(input, F32)
        }
        1 => skip.holder_set(input),
        2 => Ok(()),
        3 => {
            skip.bytes(input, F32)?;
            skip.boolean(input).map(drop)
        }
        4 => sound(skip, input),
        _ => Err(skip.bad()),
    }
}

fn effect_details(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    let mut skip = skip;
    loop {
        skip.var_int(input)?;
        skip.var_int(input)?;
        for _ in 0..3 {
            skip.boolean(input)?;
        }
        if !skip.boolean(input)? {
            return Ok(());
        }
        skip = skip.nested()?;
    }
}

fn block_predicate(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    let skip = skip.nested()?;
    skip.optional(input, ShapeSkip::holder_set)?;
    skip.optional(input, |skip, input| skip.list(input, state_property))?;
    skip.optional(input, ShapeSkip::nbt)?;
    skip_component_map(input, skip.depth)?;
    skip.list(input, |skip, input| {
        skip.boolean(input)?;
        skip.var_int(input)?;
        skip.nbt(input)
    })
}

fn state_property(skip: ShapeSkip, input: &mut &[u8]) -> Result<(), WireError> {
    skip.string(input)?;
    if skip.boolean(input)? {
        skip.string(input)
    } else {
        skip.optional(input, ShapeSkip::string)?;
        skip.optional(input, ShapeSkip::string)
    }
}
