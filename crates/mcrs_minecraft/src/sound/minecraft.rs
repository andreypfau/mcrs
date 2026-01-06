use mcrs_protocol::{Ident, ident};

pub const EMPTY: Ident<&str> = ident!("intentionally_empty");

pub const WOOD_BREAK: Ident<&str> = ident!("block.wood.break");
pub const WOOD_FALL: Ident<&str> = ident!("block.wood.fall");
pub const WOOD_HIT: Ident<&str> = ident!("block.wood.hit");
pub const WOOD_PLACE: Ident<&str> = ident!("block.wood.place");
pub const WOOD_STEP: Ident<&str> = ident!("block.wood.step");

pub const STONE_BREAK: Ident<&str> = ident!("block.stone.break");
pub const STONE_FALL: Ident<&str> = ident!("block.stone.fall");
pub const STONE_HIT: Ident<&str> = ident!("block.stone.hit");
pub const STONE_PLACE: Ident<&str> = ident!("block.stone.place");
pub const STONE_PRESSURE_PLATE_CLICK_OFF: Ident<&str> =
    ident!("block.stone_pressure_plate.click_off");
pub const STONE_PRESSURE_PLATE_CLICK_ON: Ident<&str> =
    ident!("block.stone_pressure_plate.click_on");
pub const STONE_STEP: Ident<&str> = ident!("block.stone.step");
