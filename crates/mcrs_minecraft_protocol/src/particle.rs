use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;

use anyhow::{Context, ensure};
use bytes::Bytes;
use mcrs_minecraft_core::codec::{self, PositiveInt, float_value, int_value};
use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::Template;
use crate::item::component::{ArgbInt, BlockReg, RgbInt};
use crate::item::ctx::{DecodeCtx, EncodeCtx, Opaque, ctx_free};
use crate::{Decode, Encode, VarInt};

/// Every particle type in vanilla registration order, which is the wire id.
macro_rules! particle_types {
    ($($id:literal $full:literal $bare:literal : $variant:ident $(($payload:ty))?),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum ParticleKind {
            $($variant = $id),*
        }

        const _: () = {
            let mut position = 0;
            $(
                assert!($id == position, "a particle type's wire id must be its position");
                position += 1;
            )*
            let _ = position;
        };

        impl ParticleKind {
            pub const COUNT: usize = [$($id),*].len();
            pub const ALL: [ParticleKind; Self::COUNT] = [$(Self::$variant),*];

            pub const fn id(self) -> ResourceLocation<&'static str> {
                match self {
                    $(Self::$variant => ResourceLocation::new_static($full)),*
                }
            }

            pub fn from_id(id: &str) -> Option<Self> {
                match id.strip_prefix("minecraft:").unwrap_or(id) {
                    $($bare => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "type")]
        pub enum ParticleOptions {
            $(
                #[serde(rename = $full, alias = $bare)]
                $variant $(($payload))?
            ),*
        }

        impl ParticleOptions {
            pub fn kind(&self) -> ParticleKind {
                match self {
                    $(variant_pattern!(Self::$variant, _payload $(: $payload)?) => ParticleKind::$variant),*
                }
            }

            pub fn encode_payload(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                match self {
                    $(variant_pattern!(Self::$variant, payload $(: $payload)?) => {
                        encode_arm!(payload, ctx, w $(, $payload)?)
                    })*
                }
            }

            pub fn decode_payload(
                kind: ParticleKind,
                ctx: &dyn RegistryLookup,
                r: &mut &[u8],
            ) -> anyhow::Result<Self> {
                Ok(match kind {
                    $(ParticleKind::$variant => decode_arm!(Self::$variant, ctx, r $(, $payload)?)),*
                })
            }
        }
    };
}

macro_rules! variant_pattern {
    ($variant:path, $binding:ident) => {
        $variant
    };
    ($variant:path, $binding:ident : $payload:ty) => {
        $variant($binding)
    };
}

macro_rules! encode_arm {
    ($binding:ident, $ctx:ident, $w:ident) => {
        Ok(())
    };
    ($binding:ident, $ctx:ident, $w:ident, $payload:ty) => {
        $binding.encode_ctx($ctx, $w)
    };
}

macro_rules! decode_arm {
    ($variant:path, $ctx:ident, $r:ident) => {
        $variant
    };
    ($variant:path, $ctx:ident, $r:ident, $payload:ty) => {
        $variant(<$payload>::decode_ctx($ctx, $r)?)
    };
}

particle_types! {
      0 "minecraft:angry_villager"                  "angry_villager"                  : AngryVillager,
      1 "minecraft:block"                           "block"                           : Block(BlockParticle),
      2 "minecraft:block_marker"                    "block_marker"                    : BlockMarker(BlockParticle),
      3 "minecraft:bubble"                          "bubble"                          : Bubble,
      4 "minecraft:sulfur_bubbles"                  "sulfur_bubbles"                  : SulfurBubbles,
      5 "minecraft:noxious_gas"                     "noxious_gas"                     : NoxiousGas,
      6 "minecraft:noxious_gas_cloud"               "noxious_gas_cloud"               : NoxiousGasCloud,
      7 "minecraft:geyser"                          "geyser"                          : Geyser(GeyserParticle),
      8 "minecraft:geyser_base"                     "geyser_base"                     : GeyserBase(GeyserBaseParticle),
      9 "minecraft:geyser_poof"                     "geyser_poof"                     : GeyserPoof(GeyserBaseParticle),
     10 "minecraft:geyser_plume"                    "geyser_plume"                    : GeyserPlume(GeyserParticle),
     11 "minecraft:cloud"                           "cloud"                           : Cloud,
     12 "minecraft:copper_fire_flame"               "copper_fire_flame"               : CopperFireFlame,
     13 "minecraft:crit"                            "crit"                            : Crit,
     14 "minecraft:damage_indicator"                "damage_indicator"                : DamageIndicator,
     15 "minecraft:dragon_breath"                   "dragon_breath"                   : DragonBreath(PowerParticle),
     16 "minecraft:dripping_lava"                   "dripping_lava"                   : DrippingLava,
     17 "minecraft:falling_lava"                    "falling_lava"                    : FallingLava,
     18 "minecraft:landing_lava"                    "landing_lava"                    : LandingLava,
     19 "minecraft:dripping_water"                  "dripping_water"                  : DrippingWater,
     20 "minecraft:falling_water"                   "falling_water"                   : FallingWater,
     21 "minecraft:dust"                            "dust"                            : Dust(DustParticle),
     22 "minecraft:dust_color_transition"           "dust_color_transition"           : DustColorTransition(DustColorTransitionParticle),
     23 "minecraft:effect"                          "effect"                          : Effect(SpellParticle),
     24 "minecraft:elder_guardian"                  "elder_guardian"                  : ElderGuardian,
     25 "minecraft:enchanted_hit"                   "enchanted_hit"                   : EnchantedHit,
     26 "minecraft:enchant"                         "enchant"                         : Enchant,
     27 "minecraft:end_rod"                         "end_rod"                         : EndRod,
     28 "minecraft:entity_effect"                   "entity_effect"                   : EntityEffect(ColorParticle),
     29 "minecraft:explosion_emitter"               "explosion_emitter"               : ExplosionEmitter,
     30 "minecraft:explosion"                       "explosion"                       : Explosion,
     31 "minecraft:gust"                            "gust"                            : Gust,
     32 "minecraft:small_gust"                      "small_gust"                      : SmallGust,
     33 "minecraft:gust_emitter_large"              "gust_emitter_large"              : GustEmitterLarge,
     34 "minecraft:gust_emitter_small"              "gust_emitter_small"              : GustEmitterSmall,
     35 "minecraft:sonic_boom"                      "sonic_boom"                      : SonicBoom,
     36 "minecraft:falling_dust"                    "falling_dust"                    : FallingDust(BlockParticle),
     37 "minecraft:firework"                        "firework"                        : Firework,
     38 "minecraft:fishing"                         "fishing"                         : Fishing,
     39 "minecraft:flame"                           "flame"                           : Flame,
     40 "minecraft:infested"                        "infested"                        : Infested,
     41 "minecraft:cherry_leaves"                   "cherry_leaves"                   : CherryLeaves,
     42 "minecraft:pale_oak_leaves"                 "pale_oak_leaves"                 : PaleOakLeaves,
     43 "minecraft:red_poplar_leaves"               "red_poplar_leaves"               : RedPoplarLeaves,
     44 "minecraft:orange_poplar_leaves"            "orange_poplar_leaves"            : OrangePoplarLeaves,
     45 "minecraft:yellow_poplar_leaves"            "yellow_poplar_leaves"            : YellowPoplarLeaves,
     46 "minecraft:tinted_leaves"                   "tinted_leaves"                   : TintedLeaves(ColorParticle),
     47 "minecraft:sculk_soul"                      "sculk_soul"                      : SculkSoul,
     48 "minecraft:sculk_charge"                    "sculk_charge"                    : SculkCharge(SculkChargeParticle),
     49 "minecraft:sculk_charge_pop"                "sculk_charge_pop"                : SculkChargePop,
     50 "minecraft:soul_fire_flame"                 "soul_fire_flame"                 : SoulFireFlame,
     51 "minecraft:soul"                            "soul"                            : Soul,
     52 "minecraft:flash"                           "flash"                           : Flash(ColorParticle),
     53 "minecraft:happy_villager"                  "happy_villager"                  : HappyVillager,
     54 "minecraft:composter"                       "composter"                       : Composter,
     55 "minecraft:heart"                           "heart"                           : Heart,
     56 "minecraft:instant_effect"                  "instant_effect"                  : InstantEffect(SpellParticle),
     57 "minecraft:item"                            "item"                            : Item(ItemParticle),
     58 "minecraft:vibration"                       "vibration"                       : Vibration(VibrationParticle),
     59 "minecraft:trail"                           "trail"                           : Trail(TrailParticle),
     60 "minecraft:pause_mob_growth"                "pause_mob_growth"                : PauseMobGrowth,
     61 "minecraft:reset_mob_growth"                "reset_mob_growth"                : ResetMobGrowth,
     62 "minecraft:item_slime"                      "item_slime"                      : ItemSlime,
     63 "minecraft:item_cobweb"                     "item_cobweb"                     : ItemCobweb,
     64 "minecraft:item_snowball"                   "item_snowball"                   : ItemSnowball,
     65 "minecraft:large_smoke"                     "large_smoke"                     : LargeSmoke,
     66 "minecraft:lava"                            "lava"                            : Lava,
     67 "minecraft:mycelium"                        "mycelium"                        : Mycelium,
     68 "minecraft:note"                            "note"                            : Note,
     69 "minecraft:poof"                            "poof"                            : Poof,
     70 "minecraft:portal"                          "portal"                          : Portal,
     71 "minecraft:rain"                            "rain"                            : Rain,
     72 "minecraft:smoke"                           "smoke"                           : Smoke,
     73 "minecraft:white_smoke"                     "white_smoke"                     : WhiteSmoke,
     74 "minecraft:sneeze"                          "sneeze"                          : Sneeze,
     75 "minecraft:spit"                            "spit"                            : Spit,
     76 "minecraft:squid_ink"                       "squid_ink"                       : SquidInk,
     77 "minecraft:sweep_attack"                    "sweep_attack"                    : SweepAttack,
     78 "minecraft:totem_of_undying"                "totem_of_undying"                : TotemOfUndying,
     79 "minecraft:underwater"                      "underwater"                      : Underwater,
     80 "minecraft:splash"                          "splash"                          : Splash,
     81 "minecraft:witch"                           "witch"                           : Witch,
     82 "minecraft:bubble_pop"                      "bubble_pop"                      : BubblePop,
     83 "minecraft:current_down"                    "current_down"                    : CurrentDown,
     84 "minecraft:bubble_column_up"                "bubble_column_up"                : BubbleColumnUp,
     85 "minecraft:nautilus"                        "nautilus"                        : Nautilus,
     86 "minecraft:dolphin"                         "dolphin"                         : Dolphin,
     87 "minecraft:campfire_cosy_smoke"             "campfire_cosy_smoke"             : CampfireCosySmoke,
     88 "minecraft:campfire_signal_smoke"           "campfire_signal_smoke"           : CampfireSignalSmoke,
     89 "minecraft:dripping_honey"                  "dripping_honey"                  : DrippingHoney,
     90 "minecraft:falling_honey"                   "falling_honey"                   : FallingHoney,
     91 "minecraft:landing_honey"                   "landing_honey"                   : LandingHoney,
     92 "minecraft:falling_nectar"                  "falling_nectar"                  : FallingNectar,
     93 "minecraft:falling_spore_blossom"           "falling_spore_blossom"           : FallingSporeBlossom,
     94 "minecraft:ash"                             "ash"                             : Ash,
     95 "minecraft:crimson_spore"                   "crimson_spore"                   : CrimsonSpore,
     96 "minecraft:warped_spore"                    "warped_spore"                    : WarpedSpore,
     97 "minecraft:spore_blossom_air"               "spore_blossom_air"               : SporeBlossomAir,
     98 "minecraft:dripping_obsidian_tear"          "dripping_obsidian_tear"          : DrippingObsidianTear,
     99 "minecraft:falling_obsidian_tear"           "falling_obsidian_tear"           : FallingObsidianTear,
    100 "minecraft:landing_obsidian_tear"           "landing_obsidian_tear"           : LandingObsidianTear,
    101 "minecraft:reverse_portal"                  "reverse_portal"                  : ReversePortal,
    102 "minecraft:white_ash"                       "white_ash"                       : WhiteAsh,
    103 "minecraft:small_flame"                     "small_flame"                     : SmallFlame,
    104 "minecraft:snowflake"                       "snowflake"                       : Snowflake,
    105 "minecraft:dripping_dripstone_lava"         "dripping_dripstone_lava"         : DrippingDripstoneLava,
    106 "minecraft:falling_dripstone_lava"          "falling_dripstone_lava"          : FallingDripstoneLava,
    107 "minecraft:dripping_dripstone_water"        "dripping_dripstone_water"        : DrippingDripstoneWater,
    108 "minecraft:falling_dripstone_water"         "falling_dripstone_water"         : FallingDripstoneWater,
    109 "minecraft:glow_squid_ink"                  "glow_squid_ink"                  : GlowSquidInk,
    110 "minecraft:glow"                            "glow"                            : Glow,
    111 "minecraft:wax_on"                          "wax_on"                          : WaxOn,
    112 "minecraft:wax_off"                         "wax_off"                         : WaxOff,
    113 "minecraft:electric_spark"                  "electric_spark"                  : ElectricSpark,
    114 "minecraft:scrape"                          "scrape"                          : Scrape,
    115 "minecraft:shriek"                          "shriek"                          : Shriek(ShriekParticle),
    116 "minecraft:egg_crack"                       "egg_crack"                       : EggCrack,
    117 "minecraft:dust_plume"                      "dust_plume"                      : DustPlume,
    118 "minecraft:trial_spawner_detection"         "trial_spawner_detection"         : TrialSpawnerDetection,
    119 "minecraft:trial_spawner_detection_ominous" "trial_spawner_detection_ominous" : TrialSpawnerDetectionOminous,
    120 "minecraft:vault_connection"                "vault_connection"                : VaultConnection,
    121 "minecraft:dust_pillar"                     "dust_pillar"                     : DustPillar(BlockParticle),
    122 "minecraft:ominous_spawning"                "ominous_spawning"                : OminousSpawning,
    123 "minecraft:raid_omen"                       "raid_omen"                       : RaidOmen,
    124 "minecraft:trial_omen"                      "trial_omen"                      : TrialOmen,
    125 "minecraft:block_crumble"                   "block_crumble"                   : BlockCrumble(BlockParticle),
    126 "minecraft:firefly"                         "firefly"                         : Firefly,
    127 "minecraft:sulfur_cube_goo"                 "sulfur_cube_goo"                 : SulfurCubeGoo,
}

impl ParticleKind {
    pub const fn from_wire_id(id: i32) -> Option<Self> {
        if id >= 0 && (id as usize) < Self::COUNT {
            Some(Self::ALL[id as usize])
        } else {
            None
        }
    }
}

impl fmt::Display for ParticleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id().as_str())
    }
}

impl Encode for ParticleKind {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(*self as i32).encode(w)
    }
}

impl Decode<'_> for ParticleKind {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        Self::from_wire_id(id).with_context(|| format!("unknown particle type {id}"))
    }
}

impl EncodeCtx for ParticleOptions {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.kind().encode(&mut w)?;
        self.encode_payload(ctx, w)
    }
}

impl DecodeCtx<'_> for ParticleOptions {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let kind = ParticleKind::decode(r)?;
        Self::decode_payload(kind, ctx, r)
    }
}

/// A bare block id names the block's default state, which is what empty
/// `properties` mean; any other state is `{id, properties}`. Properties read
/// from a datapack stay as written, so a partial set is completed only when
/// the id is resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockStateValue {
    pub block: ResourceKey<BlockReg>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct BlockStateRepr {
    id: ResourceKey<BlockReg>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    properties: BTreeMap<String, String>,
}

impl Serialize for BlockStateValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.properties.is_empty() {
            return self.block.serialize(s);
        }
        BlockStateRepr {
            id: self.block.clone(),
            properties: self.properties.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for BlockStateValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct BlockStateVisitor;

        impl<'de> Visitor<'de> for BlockStateVisitor {
            type Value = BlockStateValue;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a block id or a block state")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<BlockStateValue, E> {
                Ok(BlockStateValue {
                    block: ResourceKey::deserialize(value::StrDeserializer::<E>::new(id))?,
                    properties: BTreeMap::new(),
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<BlockStateValue, A::Error> {
                let repr = BlockStateRepr::deserialize(value::MapAccessDeserializer::new(map))?;
                Ok(BlockStateValue {
                    block: repr.id,
                    properties: repr.properties,
                })
            }
        }

        d.deserialize_any(BlockStateVisitor)
    }
}

impl EncodeCtx for BlockStateValue {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        let properties: Vec<(&str, &str)> = self
            .properties
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let id = ctx
            .block_state_id(self.block.location(), &properties)
            .with_context(|| format!("{} has no block state {:?}", self.block, self.properties))?;
        VarInt(id as i32).encode(w)
    }
}

impl DecodeCtx<'_> for BlockStateValue {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let (block, properties) = u32::try_from(id)
            .ok()
            .and_then(|id| ctx.block_state(id))
            .with_context(|| format!("no block state has id {id}"))?;
        Ok(BlockStateValue {
            block: ResourceKey::from_location(block),
            properties: properties.into_iter().collect(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockParticle {
    pub block_state: BlockStateValue,
}

impl EncodeCtx for BlockParticle {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.block_state.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for BlockParticle {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BlockParticle {
            block_state: BlockStateValue::decode_ctx(ctx, r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeyserParticle {
    pub water_blocks: PositiveInt,
}

impl Encode for GeyserParticle {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.water_blocks.0.encode(w)
    }
}

impl Decode<'_> for GeyserParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(GeyserParticle {
            water_blocks: codec::Bounded(i32::decode(r)?),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeyserBaseParticle {
    pub water_blocks: PositiveInt,
    #[serde(deserialize_with = "float_value")]
    pub burst_impulse_base: f32,
}

impl Encode for GeyserBaseParticle {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.water_blocks.0.encode(&mut w)?;
        self.burst_impulse_base.encode(w)
    }
}

impl Decode<'_> for GeyserBaseParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(GeyserBaseParticle {
            water_blocks: codec::Bounded(i32::decode(r)?),
            burst_impulse_base: f32::decode(r)?,
        })
    }
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct PowerParticle {
    #[serde(default = "one", deserialize_with = "float_value")]
    pub power: f32,
}

/// Rejected outside 0.01..=4 when read from data, clamped into it from the
/// wire.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ParticleScale(pub f32);

impl ParticleScale {
    pub const MIN: f32 = 0.01;
    pub const MAX: f32 = 4.0;

    pub fn clamped(scale: f32) -> Self {
        ParticleScale(scale.clamp(Self::MIN, Self::MAX))
    }
}

impl<'de> Deserialize<'de> for ParticleScale {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let scale = float_value(d)?;
        if !(Self::MIN..=Self::MAX).contains(&scale) {
            return Err(D::Error::custom(format_args!(
                "Value must be within range [{:?};{:?}]: {scale:?}",
                Self::MIN,
                Self::MAX
            )));
        }
        Ok(ParticleScale(scale))
    }
}

impl Encode for ParticleScale {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.0.encode(w)
    }
}

impl Decode<'_> for ParticleScale {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Self::clamped(f32::decode(r)?))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct DustParticle {
    pub color: RgbInt,
    pub scale: ParticleScale,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct DustColorTransitionParticle {
    pub from_color: RgbInt,
    pub to_color: RgbInt,
    pub scale: ParticleScale,
}

fn white() -> RgbInt {
    RgbInt(-1)
}

fn is_white(color: &RgbInt) -> bool {
    *color == white()
}

fn is_one(power: &f32) -> bool {
    *power == 1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct SpellParticle {
    #[serde(default = "white", skip_serializing_if = "is_white")]
    pub color: RgbInt,
    #[serde(
        default = "one",
        deserialize_with = "float_value",
        skip_serializing_if = "is_one"
    )]
    pub power: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct ColorParticle {
    pub color: ArgbInt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct SculkChargeParticle {
    #[serde(deserialize_with = "float_value")]
    pub roll: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShriekParticle {
    #[serde(deserialize_with = "int_value")]
    pub delay: i32,
}

impl Encode for ShriekParticle {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.delay).encode(w)
    }
}

impl Decode<'_> for ShriekParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ShriekParticle {
            delay: VarInt::decode(r)?.0,
        })
    }
}

ctx_free!(
    GeyserParticle,
    GeyserBaseParticle,
    PowerParticle,
    DustParticle,
    DustColorTransitionParticle,
    SpellParticle,
    ColorParticle,
    SculkChargeParticle,
    ShriekParticle,
);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemParticle {
    pub item: Template,
}

impl EncodeCtx for ItemParticle {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.item.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for ItemParticle {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ItemParticle {
            item: Template::decode_ctx(ctx, r)?,
        })
    }
}

/// `PositionSource`, dispatched on the `position_source_type` registry. An
/// entity source names the entity by network id, which no datapack can
/// address, so only a block source has a persistent form.
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
pub enum PositionSource {
    Block { pos: BlockPos },
    Entity { entity_id: VarInt, y_offset: f32 },
}

impl PositionSource {
    pub const BLOCK_TYPE: ResourceLocation<&'static str> =
        ResourceLocation::new_static("minecraft:block");
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum PositionSourceRepr {
    #[serde(rename = "minecraft:block", alias = "block")]
    Block { pos: [i32; 3] },
    #[serde(rename = "minecraft:entity", alias = "entity")]
    Entity {},
}

impl Serialize for PositionSource {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let Self::Block { pos } = self else {
            return Err(serde::ser::Error::custom(
                "Entity position sources are not allowed",
            ));
        };
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("pos", &[pos.x, pos.y, pos.z])?;
        map.serialize_entry("type", Self::BLOCK_TYPE.as_str())?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for PositionSource {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match PositionSourceRepr::deserialize(d)? {
            PositionSourceRepr::Block { pos: [x, y, z] } => Ok(Self::Block {
                pos: BlockPos::new(x, y, z),
            }),
            PositionSourceRepr::Entity {} => {
                Err(D::Error::custom("Entity position sources are not allowed"))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VibrationParticle {
    pub destination: PositionSource,
    #[serde(deserialize_with = "int_value")]
    pub arrival_in_ticks: i32,
}

impl Encode for VibrationParticle {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.destination.encode(&mut w)?;
        VarInt(self.arrival_in_ticks).encode(w)
    }
}

impl Decode<'_> for VibrationParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(VibrationParticle {
            destination: PositionSource::decode(r)?,
            arrival_in_ticks: VarInt::decode(r)?.0,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrailParticle {
    pub target: [f64; 3],
    pub color: RgbInt,
    pub duration: PositiveInt,
}

impl Encode for TrailParticle {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.target.encode(&mut w)?;
        self.color.encode(&mut w)?;
        VarInt(self.duration.0).encode(w)
    }
}

impl Decode<'_> for TrailParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(TrailParticle {
            target: <[f64; 3]>::decode(r)?,
            color: RgbInt::decode(r)?,
            duration: codec::Bounded(VarInt::decode(r)?.0),
        })
    }
}

ctx_free!(VibrationParticle, TrailParticle);

/// The exact wire bytes of one particle, kept so a packet can carry it
/// without the registries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawParticle(pub Bytes);

impl RawParticle {
    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<ParticleOptions> {
        let mut r = &self.0[..];
        let particle = ParticleOptions::decode_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after a particle", r.len());
        Ok(particle)
    }

    pub fn from_options(
        particle: &ParticleOptions,
        ctx: &dyn RegistryLookup,
    ) -> anyhow::Result<RawParticle> {
        let mut bytes = Vec::new();
        particle.encode_ctx(ctx, &mut bytes)?;
        Ok(RawParticle(bytes.into()))
    }
}

impl Encode for RawParticle {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl Decode<'_> for RawParticle {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        ParticleOptions::decode_ctx(&Opaque, r)?;
        Ok(RawParticle(Bytes::copy_from_slice(
            &start[..start.len() - r.len()],
        )))
    }
}
