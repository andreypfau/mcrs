#[rustfmt::skip]
pub mod keys;

use std::collections::BTreeMap;

use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::codec::{PositiveInt, float_value, int_value};
use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation};
use mcrs_minecraft_item::Template;
use mcrs_minecraft_item::component::{ArgbInt, RgbInt};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! particle_types {
    ($($variant:ident $(($payload:ty))?),* $(,)?) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(remote = "Self")]
        pub enum ParticleOptions {
            $($variant $(($payload))?),*
        }

        mcrs_minecraft_registry::dispatch! {
            ParticleOptions, key = "type", registry = crate::keys::ParticleType,
            {
                $($variant => $variant),*
            }
        }
    };
}

/// Calls `$callback!` with every particle type's `ParticleType` variant and
/// its payload, if it has one.
#[macro_export]
macro_rules! for_each_particle_type {
    ($callback:ident) => {
        $callback! {
        AngryVillager,
        Block(BlockParticle),
        BlockMarker(BlockParticle),
        Bubble,
        SulfurBubbles,
        NoxiousGas,
        NoxiousGasCloud,
        Geyser(GeyserParticle),
        GeyserBase(GeyserBaseParticle),
        GeyserPoof(GeyserBaseParticle),
        GeyserPlume(GeyserParticle),
        Cloud,
        CopperFireFlame,
        Crit,
        DamageIndicator,
        DragonBreath(PowerParticle),
        DrippingLava,
        FallingLava,
        LandingLava,
        DrippingWater,
        FallingWater,
        Dust(DustParticle),
        DustColorTransition(DustColorTransitionParticle),
        Effect(SpellParticle),
        ElderGuardian,
        EnchantedHit,
        Enchant,
        EndRod,
        EntityEffect(ColorParticle),
        ExplosionEmitter,
        Explosion,
        Gust,
        SmallGust,
        GustEmitterLarge,
        GustEmitterSmall,
        SonicBoom,
        FallingDust(BlockParticle),
        Firework,
        Fishing,
        Flame,
        Infested,
        CherryLeaves,
        PaleOakLeaves,
        RedPoplarLeaves,
        OrangePoplarLeaves,
        YellowPoplarLeaves,
        TintedLeaves(ColorParticle),
        SculkSoul,
        SculkCharge(SculkChargeParticle),
        SculkChargePop,
        SoulFireFlame,
        Soul,
        Flash(ColorParticle),
        HappyVillager,
        Composter,
        Heart,
        InstantEffect(SpellParticle),
        Item(ItemParticle),
        Vibration(VibrationParticle),
        Trail(TrailParticle),
        PauseMobGrowth,
        ResetMobGrowth,
        ItemSlime,
        ItemCobweb,
        ItemSnowball,
        LargeSmoke,
        Lava,
        Mycelium,
        Note,
        Poof,
        Portal,
        Rain,
        Smoke,
        WhiteSmoke,
        Sneeze,
        Spit,
        SquidInk,
        SweepAttack,
        TotemOfUndying,
        Underwater,
        Splash,
        Witch,
        BubblePop,
        CurrentDown,
        BubbleColumnUp,
        Nautilus,
        Dolphin,
        CampfireCosySmoke,
        CampfireSignalSmoke,
        DrippingHoney,
        FallingHoney,
        LandingHoney,
        FallingNectar,
        FallingSporeBlossom,
        Ash,
        CrimsonSpore,
        WarpedSpore,
        SporeBlossomAir,
        DrippingObsidianTear,
        FallingObsidianTear,
        LandingObsidianTear,
        ReversePortal,
        WhiteAsh,
        SmallFlame,
        Snowflake,
        DrippingDripstoneLava,
        FallingDripstoneLava,
        DrippingDripstoneWater,
        FallingDripstoneWater,
        GlowSquidInk,
        Glow,
        WaxOn,
        WaxOff,
        ElectricSpark,
        Scrape,
        Shriek(ShriekParticle),
        EggCrack,
        DustPlume,
        TrialSpawnerDetection,
        TrialSpawnerDetectionOminous,
        VaultConnection,
        DustPillar(BlockParticle),
        OminousSpawning,
        RaidOmen,
        TrialOmen,
        BlockCrumble(BlockParticle),
        Firefly,
        SulfurCubeGoo,
        }
    };
}

for_each_particle_type!(particle_types);

/// A bare block id names the block's default state, which is what empty
/// `properties` mean; any other state is `{id, properties}`. Properties read
/// from a datapack stay as written, so a partial set is completed only when
/// the id is resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockStateValue {
    pub block: ResourceKey<Block>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum BlockStateRepr {
    Id(ResourceKey<Block>),
    State {
        id: ResourceKey<Block>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        properties: BTreeMap<String, String>,
    },
}

impl Serialize for BlockStateValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.properties.is_empty() {
            return BlockStateRepr::Id(self.block.clone()).serialize(s);
        }
        BlockStateRepr::State {
            id: self.block.clone(),
            properties: self.properties.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for BlockStateValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match BlockStateRepr::deserialize(d)? {
            BlockStateRepr::Id(block) => BlockStateValue {
                block,
                properties: BTreeMap::new(),
            },
            BlockStateRepr::State { id, properties } => BlockStateValue {
                block: id,
                properties,
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockParticle {
    pub block_state: BlockStateValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeyserParticle {
    pub water_blocks: PositiveInt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeyserBaseParticle {
    pub water_blocks: PositiveInt,
    #[serde(deserialize_with = "float_value")]
    pub burst_impulse_base: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DustParticle {
    pub color: RgbInt,
    pub scale: ParticleScale,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorParticle {
    pub color: ArgbInt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemParticle {
    pub item: Template,
}

/// `PositionSource`, dispatched on the `position_source_type` registry. An
/// entity source names the entity by network id, which no datapack can
/// address, so only a block source has a persistent form.
#[derive(Clone, Debug, PartialEq)]
pub enum PositionSource {
    Block { pos: BlockPos },
    Entity { entity_id: i32, y_offset: f32 },
}

impl PositionSource {
    pub const BLOCK_TYPE: ResourceLocation<&'static str> =
        ResourceLocation::new_static("minecraft:block");
}

#[derive(Deserialize)]
#[serde(remote = "Self")]
enum PositionSourceRepr {
    Block { pos: [i32; 3] },
    Entity {},
}

mcrs_minecraft_registry::dispatch! {
    reads_only PositionSourceRepr, key = "type", registry = crate::keys::PositionSourceType,
    {
        Block => Block,
        Entity => Entity,
    }
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
        match <PositionSourceRepr as Deserialize>::deserialize(d)? {
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrailParticle {
    pub target: [f64; 3],
    pub color: RgbInt,
    pub duration: PositiveInt,
}
