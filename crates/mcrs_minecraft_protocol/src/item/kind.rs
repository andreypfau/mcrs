use std::fmt;

use mcrs_minecraft_core::ResourceLocation;
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::*;

/// Every data component kind in vanilla registration order, which is the wire
/// id: `$id` must equal the entry's position, which is asserted at compile time.
/// Flags: exactly one of `persistent` / `transient`; `unit` for a value with no
/// fields; `ignore_swap_animation`; `nested` when the value embeds a component
/// patch or map; `nbt_wire` when the wire form is one network NBT tag.
#[macro_export]
macro_rules! for_each_data_component {
    ($callback:ident) => {
        $callback! {
      0 "custom_data"                 : CustomData                  [persistent, nbt_wire],
      1 "max_stack_size"              : MaxStackSize                [persistent],
      2 "max_damage"                  : MaxDamage                   [persistent],
      3 "damage"                      : Damage                      [persistent, ignore_swap_animation],
      4 "unbreakable"                 : Unbreakable                 [persistent, unit],
      5 "use_effects"                 : UseEffects                  [persistent],
      6 "custom_name"                 : CustomName                  [persistent],
      7 "minimum_attack_charge"       : MinimumAttackCharge         [persistent],
      8 "damage_type"                 : DamageTypeRef               [persistent],
      9 "item_name"                   : ItemName                    [persistent],
     10 "item_model"                  : ItemModel                   [persistent],
     11 "lore"                        : Lore                        [persistent],
     12 "rarity"                      : Rarity                      [persistent],
     13 "enchantments"                : Enchantments                [persistent],
     14 "can_place_on"                : CanPlaceOn                  [persistent, nested],
     15 "can_break"                   : CanBreak                    [persistent, nested],
     16 "attribute_modifiers"         : AttributeModifiers          [persistent],
     17 "custom_model_data"           : CustomModelData             [persistent],
     18 "tooltip_display"             : TooltipDisplay              [persistent],
     19 "repair_cost"                 : RepairCost                  [persistent],
     20 "creative_slot_lock"          : CreativeSlotLock            [transient, unit],
     21 "enchantment_glint_override"  : EnchantmentGlintOverride    [persistent],
     22 "intangible_projectile"       : IntangibleProjectile        [persistent, unit, nbt_wire],
     23 "food"                        : Food                        [persistent],
     24 "consumable"                  : Consumable                  [persistent],
     25 "use_remainder"               : UseRemainder                [persistent, nested],
     26 "use_cooldown"                : UseCooldown                 [persistent],
     27 "damage_resistant"            : DamageResistant             [persistent],
     28 "tool"                        : Tool                        [persistent],
     29 "weapon"                      : Weapon                      [persistent],
     30 "attack_range"                : AttackRange                 [persistent],
     31 "enchantable"                 : Enchantable                 [persistent],
     32 "equippable"                  : Equippable                  [persistent],
     33 "repairable"                  : Repairable                  [persistent],
     34 "glider"                      : Glider                      [persistent, unit],
     35 "tooltip_style"               : TooltipStyle                [persistent],
     36 "death_protection"            : DeathProtection             [persistent],
     37 "blocks_attacks"              : BlocksAttacks               [persistent],
     38 "piercing_weapon"             : PiercingWeapon              [persistent],
     39 "kinetic_weapon"              : KineticWeapon               [persistent],
     40 "attack_animation"            : AttackAnimation             [persistent],
     41 "interact_animation"          : InteractAnimation           [persistent],
     42 "additional_trade_cost"       : AdditionalTradeCost         [transient],
     43 "block_transformer"           : BlockTransformerRef         [persistent],
     44 "villager_food"               : VillagerFood                [persistent],
     45 "stored_enchantments"         : StoredEnchantments          [persistent],
     46 "dye"                         : Dye                         [persistent],
     47 "dyed_color"                  : DyedColor                   [persistent],
     48 "map_id"                      : MapId                       [persistent],
     49 "map_decorations"             : MapDecorations              [persistent, nbt_wire],
     50 "map_post_processing"         : MapPostProcessing           [transient],
     51 "charged_projectiles"         : ChargedProjectiles          [persistent, nested],
     52 "bundle_contents"             : BundleContents              [persistent, nested],
     53 "potion_contents"             : PotionContents              [persistent],
     54 "potion_duration_scale"       : PotionDurationScale         [persistent],
     55 "suspicious_stew_effects"     : SuspiciousStewEffects       [persistent],
     56 "writable_book_content"       : WritableBookContent         [persistent],
     57 "written_book_content"        : WrittenBookContent          [persistent],
     58 "trim"                        : Trim                        [persistent],
     59 "debug_stick_state"           : DebugStickState             [persistent, nbt_wire],
     60 "entity_data"                 : EntityData                  [persistent],
     61 "bucket_entity_data"          : BucketEntityData            [persistent],
     62 "block_entity_data"           : BlockEntityData             [persistent],
     63 "instrument"                  : Instrument                  [persistent],
     64 "provides_trim_material"      : ProvidesTrimMaterial        [persistent],
     65 "ominous_bottle_amplifier"    : OminousBottleAmplifier      [persistent],
     66 "jukebox_playable"            : JukeboxPlayable             [persistent],
     67 "provides_banner_patterns"    : ProvidesBannerPatterns      [persistent],
     68 "recipes"                     : Recipes                     [persistent, nbt_wire],
     69 "lodestone_tracker"           : LodestoneTracker            [persistent],
     70 "firework_explosion"          : FireworkExplosion           [persistent],
     71 "fireworks"                   : Fireworks                   [persistent],
     72 "profile"                     : Profile                     [persistent],
     73 "note_block_sound"            : NoteBlockSound              [persistent],
     74 "banner_patterns"             : BannerPatterns              [persistent],
     75 "base_color"                  : BaseColor                   [persistent],
     76 "pot_decorations"             : PotDecorations              [persistent, nested],
     77 "container"                   : Container                   [persistent, nested],
     78 "block_state"                 : BlockState                  [persistent],
     79 "bees"                        : Bees                        [persistent],
     80 "sulfur_cube_content"         : SulfurCubeContent           [persistent, nested],
     81 "lock"                        : Lock                        [persistent, nested, nbt_wire],
     82 "container_loot"              : ContainerLoot               [persistent, nbt_wire],
     83 "break_sound"                 : BreakSound                  [persistent],
     84 "compostable"                 : Compostable                 [persistent],
     85 "cooking_fuel"                : CookingFuel                 [persistent],
     86 "brewing_fuel"                : BrewingFuel                 [persistent],
     87 "mob_visibility"              : MobVisibility               [persistent],
     88 "villager/variant"            : VillagerVariant             [persistent],
     89 "wolf/variant"                : WolfVariant                 [persistent],
     90 "wolf/sound_variant"          : WolfSoundVariant            [persistent],
     91 "wolf/collar"                 : WolfCollar                  [persistent],
     92 "fox/variant"                 : FoxVariant                  [persistent],
     93 "salmon/size"                 : SalmonSize                  [persistent],
     94 "parrot/variant"              : ParrotVariant               [persistent],
     95 "tropical_fish/pattern"       : TropicalFishPattern         [persistent],
     96 "tropical_fish/base_color"    : TropicalFishBaseColor       [persistent],
     97 "tropical_fish/pattern_color" : TropicalFishPatternColor    [persistent],
     98 "mooshroom/variant"           : MooshroomVariant            [persistent],
     99 "rabbit/variant"              : RabbitVariant               [persistent],
    100 "pig/variant"                 : PigVariant                  [persistent],
    101 "pig/sound_variant"           : PigSoundVariant             [persistent],
    102 "cow/variant"                 : CowVariant                  [persistent],
    103 "cow/sound_variant"           : CowSoundVariant             [persistent],
    104 "chicken/variant"             : ChickenVariant              [persistent],
    105 "chicken/sound_variant"       : ChickenSoundVariant         [persistent],
    106 "zombie_nautilus/variant"     : ZombieNautilusVariant       [persistent],
    107 "frog/variant"                : FrogVariant                 [persistent],
    108 "horse/variant"               : HorseVariant                [persistent],
    109 "painting/variant"            : PaintingVariant             [persistent],
    110 "llama/variant"               : LlamaVariant                [persistent],
    111 "axolotl/variant"             : AxolotlVariant              [persistent],
    112 "cat/variant"                 : CatVariant                  [persistent],
    113 "cat/sound_variant"           : CatSoundVariant             [persistent],
    114 "cat/collar"                  : CatCollar                   [persistent],
    115 "sheep/color"                 : SheepColor                  [persistent],
    116 "shulker/color"               : ShulkerColor                [persistent],
    117 "provides_pottery_pattern"    : ProvidesPotteryPattern      [persistent],
    118 "sign_text_front"             : SignTextFront               [persistent],
    119 "sign_text_back"              : SignTextBack                [persistent],
    120 "waxed"                       : Waxed                       [persistent, unit],
    121 "cushion/color"               : CushionColor                [persistent],
        }
    };
}

mod flag {
    pub(super) const PERSISTENT: u8 = 1;
    pub(super) const TRANSIENT: u8 = 2;
    pub(super) const UNIT: u8 = 4;
    pub(super) const IGNORE_SWAP_ANIMATION: u8 = 8;
    pub(super) const NESTED: u8 = 16;
    pub(super) const NBT_WIRE: u8 = 32;

    #[allow(non_upper_case_globals)]
    pub(super) const persistent: u8 = PERSISTENT;
    #[allow(non_upper_case_globals)]
    pub(super) const transient: u8 = TRANSIENT;
    #[allow(non_upper_case_globals)]
    pub(super) const unit: u8 = UNIT;
    #[allow(non_upper_case_globals)]
    pub(super) const ignore_swap_animation: u8 = IGNORE_SWAP_ANIMATION;
    #[allow(non_upper_case_globals)]
    pub(super) const nested: u8 = NESTED;
    #[allow(non_upper_case_globals)]
    pub(super) const nbt_wire: u8 = NBT_WIRE;
}

macro_rules! data_components {
    ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum ItemComponentKind {
            $($ty = $id),*
        }

        const FLAGS: [u8; ItemComponentKind::COUNT] = [$(0 $(| flag::$flag)*),*];
        const IDS: [ResourceLocation<&'static str>; ItemComponentKind::COUNT] =
            [$(ResourceLocation::new_static(concat!("minecraft:", $name))),*];

        const _: () = {
            let mut position = 0;
            $(
                assert!($id == position, "a data component's wire id must be its position");
                let flags = FLAGS[position];
                assert!(
                    (flags & (flag::PERSISTENT | flag::TRANSIENT)) == flag::PERSISTENT
                        || (flags & (flag::PERSISTENT | flag::TRANSIENT)) == flag::TRANSIENT,
                    "a data component is either persistent or transient"
                );
                position += 1;
            )*
            let _ = position;
        };

        impl ItemComponentKind {
            pub const COUNT: usize = [$($id),*].len();
            pub const ALL: [ItemComponentKind; Self::COUNT] = [$(Self::$ty),*];

            pub fn from_id(id: &str) -> Option<Self> {
                let path = id.strip_prefix("minecraft:").unwrap_or(id);
                match path {
                    $($name => Some(Self::$ty),)*
                    _ => None,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq)]
        pub enum ItemComponentValue {
            $($ty($ty)),*
        }

        impl ItemComponentValue {
            pub fn kind(&self) -> ItemComponentKind {
                match self {
                    $(Self::$ty(_) => ItemComponentKind::$ty),*
                }
            }

            pub fn serialize_value<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                if !self.kind().is_persistent() {
                    return Err(S::Error::custom(format_args!(
                        "Encountered transient component {}",
                        self.kind().id()
                    )));
                }
                match self {
                    $(Self::$ty(value) => Serialize::serialize(value, s)),*
                }
            }

            pub fn deserialize_value<'de, D: Deserializer<'de>>(
                kind: ItemComponentKind,
                d: D,
            ) -> Result<Self, D::Error> {
                match kind {
                    $(ItemComponentKind::$ty => <$ty as Deserialize<'de>>::deserialize(d).map(Self::$ty)),*
                }
            }
        }

        $(
            impl ItemDataComponent for $ty {
                const KIND: ItemComponentKind = ItemComponentKind::$ty;

                fn into_value(self) -> ItemComponentValue {
                    ItemComponentValue::$ty(self)
                }

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

pub trait ItemDataComponent: Clone + PartialEq + fmt::Debug + Send + Sync + 'static {
    const KIND: ItemComponentKind;
    fn into_value(self) -> ItemComponentValue;
    fn from_value(value: &ItemComponentValue) -> Option<&Self>;
}

impl ItemComponentKind {
    pub const fn wire_id(self) -> u16 {
        self as u16
    }

    pub const fn from_wire_id(id: u16) -> Option<Self> {
        if (id as usize) < Self::COUNT {
            Some(Self::ALL[id as usize])
        } else {
            None
        }
    }

    pub const fn id(self) -> ResourceLocation<&'static str> {
        IDS[self as usize]
    }

    const fn flags(self) -> u8 {
        FLAGS[self as usize]
    }

    pub const fn is_persistent(self) -> bool {
        self.flags() & flag::PERSISTENT != 0
    }

    pub const fn is_unit(self) -> bool {
        self.flags() & flag::UNIT != 0
    }

    pub const fn ignores_swap_animation(self) -> bool {
        self.flags() & flag::IGNORE_SWAP_ANIMATION != 0
    }

    pub const fn is_nested(self) -> bool {
        self.flags() & flag::NESTED != 0
    }

    pub const fn is_nbt_wire(self) -> bool {
        self.flags() & flag::NBT_WIRE != 0
    }

    /// Vanilla names the parsed identifier, so a bare path is reported with
    /// its default namespace.
    pub(crate) fn unknown_id_error(id: &str) -> String {
        let namespace = if id.contains(':') { "" } else { "minecraft:" };
        format!("No component with type: '{namespace}{id}'")
    }
}

impl fmt::Display for ItemComponentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id().as_str())
    }
}

impl Serialize for ItemComponentKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id().as_str())
    }
}

impl<'de> Deserialize<'de> for ItemComponentKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        Self::from_id(&id).ok_or_else(|| D::Error::custom(Self::unknown_id_error(&id)))
    }
}
