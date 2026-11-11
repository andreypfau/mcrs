use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32};

const TIME_OF_DAY_AND_MOON_PERIOD: i64 = 192_000;

/// Sets the time of day of the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x0a)]
pub struct SetTime {
    /// The time of day in ticks.
    pub time: VarI32,
}

impl SetTime {
    /// Builds the packet for a world time. A time outside `i32` is sent modulo 192000, which keeps
    /// the time of day and the moon phase.
    #[must_use]
    pub fn from_world_time(time: i64) -> Self {
        let time = match i32::try_from(time) {
            Ok(time) => time,
            Err(_) => time.rem_euclid(TIME_OF_DAY_AND_MOON_PERIOD) as i32,
        };
        Self { time: VarI32(time) }
    }
}
