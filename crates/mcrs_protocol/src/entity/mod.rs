use crate::{BlockPos, BlockStateId, Direction, GlobalPos, ItemStack, VarInt, VarLong};
use mcrs_protocol::entity::player::HumanoidArm;
use mcrs_protocol_macros::{Decode, Encode};
use uuid::Uuid;
use valence_math::{Vec3, Vec4};
use valence_text::Text;

pub mod player;
mod sniffer;

#[derive(Debug, Clone, Encode, Decode)]
pub struct MetadataEntry<'a> {
    pub index: u8,
    pub value: MetaDataValue<'a>,
}

#[derive(Debug, Clone, Encode, Decode)]
pub enum MetaDataValue<'a> {
    Byte(i8),
    VarInt(VarInt),
    VarLong(VarLong),
    Float(f32),
    String(&'a str),
    Text(Text),
    OptionalText(Option<Text>),
    Slot(Option<ItemStack>),
    Boolean(bool),
    Rotations(Vec3),
    BlockPos(BlockPos),
    OptionalBlockPos(Option<BlockPos>),
    Direction(Direction),
    OptionalLivingEntityReference(Option<Uuid>),
    BlockState(BlockStateId),
    OptionalBlockState(Option<BlockStateId>),
    Particle,
    Particles,
    VillagerData(VillagerData),
    OptionalVarInt(Option<VarInt>),
    Pose(Pose),
    CatVariant(VarInt),
    CowVariant(VarInt),
    WolfVariant(VarInt),
    WoldSoundVariant(VarInt),
    FrogVariant(VarInt),
    PigVariant(VarInt),
    ChickenVariant(VarInt),
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
    UsingTTongue,
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
