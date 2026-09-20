use crate::item::RawStack;
use crate::text::Text;
use crate::{Direction, GlobalPos, VarInt, VarLong};
use bevy_math::{Vec3, Vec4};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_protocol::entity::player::HumanoidArm;
use mcrs_minecraft_protocol_macros::{Decode, Encode};
use mcrs_minecraft_registry::BlockStateId;
use std::io::Write;
use uuid::Uuid;

pub mod minecart;
pub mod player;
mod sniffer;

#[derive(Debug, Clone, PartialEq)]
pub struct MetadataEntry<'a> {
    pub index: u8,
    pub value: MetaDataValue<'a>,
}

impl crate::Encode for MetadataEntry<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.index != METADATA_END,
            "entity data index {METADATA_END} is the end marker"
        );
        self.index.encode(&mut w)?;
        self.value.encode(w)
    }
}

impl<'a> crate::Decode<'a> for MetadataEntry<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self {
            index: u8::decode(r)?,
            value: MetaDataValue::decode(r)?,
        })
    }
}

const METADATA_END: u8 = 0xFF;

/// A list of entity data entries terminated by the `0xFF` index instead of a
/// length prefix.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Metadata<'a>(pub Vec<MetadataEntry<'a>>);

impl crate::Encode for Metadata<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        for entry in &self.0 {
            entry.encode(&mut w)?;
        }
        METADATA_END.encode(w)
    }
}

impl<'a> crate::Decode<'a> for Metadata<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let mut entries = Vec::new();
        loop {
            let index = u8::decode(r)?;
            if index == METADATA_END {
                return Ok(Self(entries));
            }
            entries.push(MetadataEntry {
                index,
                value: MetaDataValue::decode(r)?,
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum MetaDataValue<'a> {
    Byte(i8),
    VarInt(VarInt),
    VarLong(VarLong),
    Float(f32),
    String(&'a str),
    Text(Text),
    OptionalText(Option<Text>),
    Slot(RawStack),
    Boolean(bool),
    Rotations(Vec3),
    BlockPos(BlockPos),
    OptionalBlockPos(Option<BlockPos>),
    Direction(Direction),
    OptionalLivingEntityReference(Option<Uuid>),
    BlockState(BlockStateId),
    OptionalBlockState(OptionalBlockState),
    Particle,
    Particles,
    VillagerData(VillagerData),
    OptionalUnsignedInt(OptionalUnsignedInt),
    Pose(Pose),
    CatVariant(VarInt),
    CatSoundVariant(VarInt),
    CowVariant(VarInt),
    CowSoundVariant(VarInt),
    WolfVariant(VarInt),
    WolfSoundVariant(VarInt),
    FrogVariant(VarInt),
    PigVariant(VarInt),
    PigSoundVariant(VarInt),
    ChickenVariant(VarInt),
    ChickenSoundVariant(VarInt),
    ZombieNautilusVariant(VarInt),
    OptionalGlobalPos(Option<GlobalPos<'a>>),
    PaintingVariant(VarInt),
    SnifferState(SnifferState),
    ArmadilloState(ArmadilloState),
    CopperGolemState(CopperGolemState),
    WeatheringCopperState(WeatheringCopperState),
    Vec3(Vec3),
    Quaternion(Vec4),
    ResolvableProfile,
    HumanoidArm(HumanoidArm),
    DyeColor(DyeColor),
}

/// `Optional<BlockState>` on the wire: the state id, with 0 (air) standing for
/// absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptionalBlockState(pub Option<BlockStateId>);

impl crate::Encode for OptionalBlockState {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0.map_or(0, |state| i32::from(state.0))).encode(w)
    }
}

impl crate::Decode<'_> for OptionalBlockState {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        if id == 0 {
            return Ok(Self(None));
        }
        Ok(Self(Some(BlockStateId(u16::try_from(id)?))))
    }
}

/// `OptionalInt` on the wire: the value plus one, with 0 standing for absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptionalUnsignedInt(pub Option<u32>);

impl crate::Encode for OptionalUnsignedInt {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0.map_or(0, |value| value as i32 + 1)).encode(w)
    }
}

impl crate::Decode<'_> for OptionalUnsignedInt {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let raw = VarInt::decode(r)?.0;
        Ok(Self((raw != 0).then(|| raw as u32 - 1)))
    }
}

/// `DyeColor.STREAM_CODEC`: the id, out of range reading as white (`ZERO`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Encode, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DyeColor {
    White,
    Orange,
    Magenta,
    LightBlue,
    Yellow,
    Lime,
    Pink,
    Gray,
    LightGray,
    Cyan,
    Purple,
    Blue,
    Brown,
    Green,
    Red,
    Black,
}

impl DyeColor {
    pub const ALL: [Self; 16] = [
        Self::White,
        Self::Orange,
        Self::Magenta,
        Self::LightBlue,
        Self::Yellow,
        Self::Lime,
        Self::Pink,
        Self::Gray,
        Self::LightGray,
        Self::Cyan,
        Self::Purple,
        Self::Blue,
        Self::Brown,
        Self::Green,
        Self::Red,
        Self::Black,
    ];
}

impl crate::Decode<'_> for DyeColor {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        Ok(usize::try_from(id)
            .ok()
            .and_then(|id| Self::ALL.get(id))
            .copied()
            .unwrap_or(Self::White))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Encode, Decode)]
pub enum Pose {
    #[default]
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Crouching,
    LongJumping,
    Dying,
    Croaking,
    UsingTongue,
    Sitting,
    Roaring,
    Sniffing,
    Emerging,
    Digging,
    Sliding,
    Shooting,
    Inhaling,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum WeatheringCopperState {
    Unaffected,
    Exposed,
    Weathered,
    Oxidized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum SnifferState {
    Idling,
    FeelingHappy,
    Scenting,
    Sniffing,
    Searching,
    Digging,
    Rising,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum ArmadilloState {
    Idle,
    Rolling,
    Scared,
    Unrolling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum CopperGolemState {
    Idle,
    GettingItem,
    GettingNoItem,
    DroppingItem,
    DroppingNoItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct VillagerData {
    pub kind: VarInt,
    pub profession: VarInt,
    pub level: VarInt,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Feet,
    Legs,
    Chest,
    Head,
    Body,
    Saddle,
}

impl EquipmentSlot {
    pub const ALL: [Self; 8] = [
        Self::MainHand,
        Self::OffHand,
        Self::Feet,
        Self::Legs,
        Self::Chest,
        Self::Head,
        Self::Body,
        Self::Saddle,
    ];

    pub fn from_id(id: u8) -> anyhow::Result<Self> {
        Self::ALL
            .get(id as usize)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("invalid equipment slot {id}"))
    }

    /// `EquipmentSlot.STREAM_CODEC` numbers the slots differently from the
    /// equipment packet's ordinal: hands first, then armour, offhand at 5.
    pub const fn equippable_id(self) -> u8 {
        match self {
            Self::MainHand => 0,
            Self::Feet => 1,
            Self::Legs => 2,
            Self::Chest => 3,
            Self::Head => 4,
            Self::OffHand => 5,
            Self::Body => 6,
            Self::Saddle => 7,
        }
    }

    /// Out of range reads as the first slot (`ByIdMap` `ZERO`).
    pub const fn from_equippable_id(id: u8) -> Self {
        match id {
            1 => Self::Feet,
            2 => Self::Legs,
            3 => Self::Chest,
            4 => Self::Head,
            5 => Self::OffHand,
            6 => Self::Body,
            7 => Self::Saddle,
            _ => Self::MainHand,
        }
    }
}
