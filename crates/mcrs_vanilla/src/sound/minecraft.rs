use mcrs_core::{rl, ResourceLocation, StaticRegistry};

use super::SoundEvent;

pub const EMPTY: ResourceLocation<&'static str> = rl!("intentionally_empty");

pub const WOOD_BREAK: ResourceLocation<&'static str> = rl!("block.wood.break");
pub const WOOD_FALL: ResourceLocation<&'static str> = rl!("block.wood.fall");
pub const WOOD_HIT: ResourceLocation<&'static str> = rl!("block.wood.hit");
pub const WOOD_PLACE: ResourceLocation<&'static str> = rl!("block.wood.place");
pub const WOOD_STEP: ResourceLocation<&'static str> = rl!("block.wood.step");

pub const STONE_BREAK: ResourceLocation<&'static str> = rl!("block.stone.break");
pub const STONE_FALL: ResourceLocation<&'static str> = rl!("block.stone.fall");
pub const STONE_HIT: ResourceLocation<&'static str> = rl!("block.stone.hit");
pub const STONE_PLACE: ResourceLocation<&'static str> = rl!("block.stone.place");
pub const STONE_PRESSURE_PLATE_CLICK_OFF: ResourceLocation<&'static str> =
    rl!("block.stone_pressure_plate.click_off");
pub const STONE_PRESSURE_PLATE_CLICK_ON: ResourceLocation<&'static str> =
    rl!("block.stone_pressure_plate.click_on");
pub const STONE_STEP: ResourceLocation<&'static str> = rl!("block.stone.step");

pub static EMPTY_EVENT: SoundEvent = SoundEvent::new(EMPTY, None);
pub static WOOD_BREAK_EVENT: SoundEvent = SoundEvent::new(WOOD_BREAK, None);
pub static WOOD_FALL_EVENT: SoundEvent = SoundEvent::new(WOOD_FALL, None);
pub static WOOD_HIT_EVENT: SoundEvent = SoundEvent::new(WOOD_HIT, None);
pub static WOOD_PLACE_EVENT: SoundEvent = SoundEvent::new(WOOD_PLACE, None);
pub static WOOD_STEP_EVENT: SoundEvent = SoundEvent::new(WOOD_STEP, None);
pub static STONE_BREAK_EVENT: SoundEvent = SoundEvent::new(STONE_BREAK, None);
pub static STONE_FALL_EVENT: SoundEvent = SoundEvent::new(STONE_FALL, None);
pub static STONE_HIT_EVENT: SoundEvent = SoundEvent::new(STONE_HIT, None);
pub static STONE_PLACE_EVENT: SoundEvent = SoundEvent::new(STONE_PLACE, None);
pub static STONE_PRESSURE_PLATE_CLICK_OFF_EVENT: SoundEvent =
    SoundEvent::new(STONE_PRESSURE_PLATE_CLICK_OFF, None);
pub static STONE_PRESSURE_PLATE_CLICK_ON_EVENT: SoundEvent =
    SoundEvent::new(STONE_PRESSURE_PLATE_CLICK_ON, None);
pub static STONE_STEP_EVENT: SoundEvent = SoundEvent::new(STONE_STEP, None);

pub fn register_all_sounds(registry: &mut StaticRegistry<SoundEvent>) {
    registry.register(EMPTY, &EMPTY_EVENT);
    registry.register(WOOD_BREAK, &WOOD_BREAK_EVENT);
    registry.register(WOOD_FALL, &WOOD_FALL_EVENT);
    registry.register(WOOD_HIT, &WOOD_HIT_EVENT);
    registry.register(WOOD_PLACE, &WOOD_PLACE_EVENT);
    registry.register(WOOD_STEP, &WOOD_STEP_EVENT);
    registry.register(STONE_BREAK, &STONE_BREAK_EVENT);
    registry.register(STONE_FALL, &STONE_FALL_EVENT);
    registry.register(STONE_HIT, &STONE_HIT_EVENT);
    registry.register(STONE_PLACE, &STONE_PLACE_EVENT);
    registry.register(
        STONE_PRESSURE_PLATE_CLICK_OFF,
        &STONE_PRESSURE_PLATE_CLICK_OFF_EVENT,
    );
    registry.register(
        STONE_PRESSURE_PLATE_CLICK_ON,
        &STONE_PRESSURE_PLATE_CLICK_ON_EVENT,
    );
    registry.register(STONE_STEP, &STONE_STEP_EVENT);
}
