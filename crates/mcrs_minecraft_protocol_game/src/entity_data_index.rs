//! The indices of the entity data entries that every entity, every living entity, every mob and
//! every ageable mob has, and the bits of the flag entries. The entries of a single type, such as
//! its variant, are in the type table of `mcrs_minecraft_entity`.

pub const SHARED_FLAGS: u8 = 0;
pub const SHARED_FLAG_ON_FIRE: i8 = 1 << 0;
pub const SHARED_FLAG_CROUCHING: i8 = 1 << 1;
pub const SHARED_FLAG_SPRINTING: i8 = 1 << 3;
pub const SHARED_FLAG_SWIMMING: i8 = 1 << 4;
pub const SHARED_FLAG_INVISIBLE: i8 = 1 << 5;
pub const SHARED_FLAG_GLOWING: i8 = 1 << 6;
pub const SHARED_FLAG_FLYING: i8 = 1 << 7;
pub const AIR_SUPPLY: u8 = 1;
pub const CUSTOM_NAME: u8 = 2;
pub const NAME_VISIBLE: u8 = 3;
pub const POSE: u8 = 6;
pub const TICKS_FROZEN: u8 = 7;
pub const LIVING_FLAGS: u8 = 8;
pub const HEALTH: u8 = 9;
pub const MOB_FLAGS: u8 = 15;
pub const BABY: u8 = 16;
pub const AGE_LOCKED: u8 = 17;

/// The bit of [`MOB_FLAGS`] of a mob without AI.
pub const MOB_FLAG_NO_AI: i8 = 1 << 0;
/// The bit of [`MOB_FLAGS`] of a left-handed mob.
pub const MOB_FLAG_LEFT_HANDED: i8 = 1 << 1;
