pub const SHARED_FLAGS: u8 = 0;
pub const CUSTOM_NAME: u8 = 2;
pub const NAME_VISIBLE: u8 = 3;
pub const NO_GRAVITY: u8 = 5;
pub const POSE: u8 = 6;
pub const TICKS_FROZEN: u8 = 7;
pub const LIVING_FLAGS: u8 = 8;
pub const HEALTH: u8 = 9;
pub const SLEEPING_POSITION: u8 = 14;
pub const MOB_FLAGS: u8 = 15;
pub const BABY: u8 = 16;
pub const AGE_LOCKED: u8 = 17;

pub const MOB_FLAG_NO_AI: i8 = 1 << 0;
pub const MOB_FLAG_LEFT_HANDED: i8 = 1 << 1;

pub const SHARED_FLAG_SHIFT: u8 = 1 << 1;
pub const SHARED_FLAG_SPRINT: u8 = 1 << 3;
