use crate::block::BlockStateNumber;
use crate::entity::player::HumanoidArm;
use crate::item::WireLookup;
pub use crate::item::component::enums::DyeColor;
pub use crate::item::component::equippable::EquipmentSlot;
use crate::text::Text;
use crate::{Direction, GlobalPos, Result, VarInt, VarLong};
use glam::{Vec3, Vec4};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::Registered;
use mcrs_minecraft_entity::variant::{
    CatSoundVariant, CatVariant, ChickenSoundVariant, ChickenVariant, CowSoundVariant, CowVariant,
    FrogVariant, PigSoundVariant, PigVariant, WolfSoundVariant, WolfVariant, ZombieNautilusVariant,
};
pub use mcrs_minecraft_entity::villager::VillagerData;
use mcrs_minecraft_item::PaintingVariantValue;
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_registries::{FromWire, ResolveError, Wire, WireId};
use mcrs_minecraft_registries::{decode_registry_id, encode_registry_id};
use mcrs_minecraft_wire::{Decode, Encode};
use std::io::Write;
use uuid::Uuid;

pub mod minecart;
pub mod player;

#[derive(Debug, Clone, PartialEq)]
pub struct MetadataEntry<'a> {
    pub index: u8,
    pub value: MetaDataValue<'a>,
}

impl crate::Encode for MetadataEntry<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        crate::wire::ensure!(
            self.index != METADATA_END,
            "entity data index {METADATA_END} is the end marker"
        );
        self.index.encode(&mut w)?;
        self.value.encode(w)
    }
}

impl<'a> crate::Decode<'a> for MetadataEntry<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
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
    fn encode(&self, mut w: impl Write) -> Result<()> {
        for entry in &self.0 {
            entry.encode(&mut w)?;
        }
        METADATA_END.encode(w)
    }
}

impl<'a> crate::Decode<'a> for Metadata<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
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

impl Metadata<'_> {
    /// Checks every registry id the entries hold against `registries`.
    pub fn check(self, registries: &Registries) -> Result<(), ResolveError> {
        fn named<R: Registered>(lookup: &WireLookup<'_>, id: Id<R>) -> Result<(), ResolveError> {
            Id::<R>::from_wire(WireId::new(id.number()), lookup).map(drop)
        }
        let lookup = WireLookup::new(registries);
        self.0.into_iter().try_for_each(|entry| match entry.value {
            MetaDataValue::Slot(stack) => ProtoStack::from_wire(stack, &lookup).map(drop),
            MetaDataValue::CatVariant(id) => named(&lookup, id),
            MetaDataValue::CatSoundVariant(id) => named(&lookup, id),
            MetaDataValue::CowVariant(id) => named(&lookup, id),
            MetaDataValue::CowSoundVariant(id) => named(&lookup, id),
            MetaDataValue::WolfVariant(id) => named(&lookup, id),
            MetaDataValue::WolfSoundVariant(id) => named(&lookup, id),
            MetaDataValue::FrogVariant(id) => named(&lookup, id),
            MetaDataValue::PigVariant(id) => named(&lookup, id),
            MetaDataValue::PigSoundVariant(id) => named(&lookup, id),
            MetaDataValue::ChickenVariant(id) => named(&lookup, id),
            MetaDataValue::ChickenSoundVariant(id) => named(&lookup, id),
            MetaDataValue::ZombieNautilusVariant(id) => named(&lookup, id),
            MetaDataValue::PaintingVariant(id) => named(&lookup, id),
            _ => Ok(()),
        })
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
    Slot(ProtoStack<Wire>),
    Boolean(bool),
    Rotations(Vec3),
    BlockPos(BlockPos),
    OptionalBlockPos(Option<BlockPos>),
    Direction(Direction),
    OptionalLivingEntityReference(Option<Uuid>),
    BlockState(BlockStateNumber),
    OptionalBlockState(OptionalBlockState),
    Particle,
    Particles,
    VillagerData(VillagerData),
    OptionalUnsignedInt(OptionalUnsignedInt),
    Pose(Pose),
    CatVariant(Id<CatVariant>),
    CatSoundVariant(Id<CatSoundVariant>),
    CowVariant(Id<CowVariant>),
    CowSoundVariant(Id<CowSoundVariant>),
    WolfVariant(Id<WolfVariant>),
    WolfSoundVariant(Id<WolfSoundVariant>),
    FrogVariant(Id<FrogVariant>),
    PigVariant(Id<PigVariant>),
    PigSoundVariant(Id<PigSoundVariant>),
    ChickenVariant(Id<ChickenVariant>),
    ChickenSoundVariant(Id<ChickenSoundVariant>),
    ZombieNautilusVariant(Id<ZombieNautilusVariant>),
    OptionalGlobalPos(Option<GlobalPos>),
    PaintingVariant(Id<PaintingVariantValue>),
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
pub struct OptionalBlockState(pub Option<BlockStateNumber>);

impl crate::Encode for OptionalBlockState {
    fn encode(&self, w: impl Write) -> Result<()> {
        encode_registry_id(self.0.map_or(0, |state| state.0), w)
    }
}

impl crate::Decode<'_> for OptionalBlockState {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let id = decode_registry_id(r)?;
        Ok(Self((id != 0).then_some(BlockStateNumber(id))))
    }
}

/// `OptionalInt` on the wire: the value plus one, with 0 standing for absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptionalUnsignedInt(pub Option<u32>);

impl crate::Encode for OptionalUnsignedInt {
    fn encode(&self, w: impl Write) -> Result<()> {
        VarInt(self.0.map_or(0, |value| value as i32 + 1)).encode(w)
    }
}

impl crate::Decode<'_> for OptionalUnsignedInt {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let raw = VarInt::decode(r)?.0;
        Ok(Self((raw != 0).then(|| raw as u32 - 1)))
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
