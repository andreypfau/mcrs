use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32};

const TIME_OF_DAY_AND_MOON_PERIOD: i64 = 192_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x0a)]
pub struct SetTime {
    pub time: VarI32,
}

impl SetTime {
    #[must_use]
    pub fn from_world_time(time: i64) -> Self {
        let time = match i32::try_from(time) {
            Ok(time) => time,
            Err(_) => time.rem_euclid(TIME_OF_DAY_AND_MOON_PERIOD) as i32,
        };
        Self { time: VarI32(time) }
    }
}
