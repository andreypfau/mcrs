//! Blocks that carry data, as a village leaves them: empty and unlit.

use mcrs_minecraft_core::Direction;
use mcrs_minecraft_keys as keys;

use super::{Canvas, Fields, Tag, block};

#[rustfmt::skip]
const BARREL: Fields = &[("Items", Tag::List(&[])), ("id", Tag::String(keys::block_entity_type::BARREL.as_static_str()))];

#[rustfmt::skip]
const BELL: Fields = &[("id", Tag::String(keys::block_entity_type::BELL.as_static_str()))];

#[rustfmt::skip]
const BLAST_FURNACE: Fields = &[("lit_total_time", Tag::Short(0)), ("cooking_time_spent", Tag::Short(0)), ("Items", Tag::List(&[])), ("cooking_total_time", Tag::Short(0)), ("id", Tag::String(keys::block_entity_type::BLAST_FURNACE.as_static_str())), ("lit_time_remaining", Tag::Short(0)), ("RecipesUsed", Tag::Compound(&[]))];

#[rustfmt::skip]
const BREWING_STAND: Fields = &[("Fuel", Tag::Byte(0)), ("Items", Tag::List(&[])), ("id", Tag::String(keys::block_entity_type::BREWING_STAND.as_static_str())), ("BrewTime", Tag::Short(0))];

#[rustfmt::skip]
const FURNACE: Fields = &[("lit_total_time", Tag::Short(0)), ("cooking_time_spent", Tag::Short(0)), ("Items", Tag::List(&[])), ("cooking_total_time", Tag::Short(0)), ("id", Tag::String(keys::block_entity_type::FURNACE.as_static_str())), ("lit_time_remaining", Tag::Short(0)), ("RecipesUsed", Tag::Compound(&[]))];

#[rustfmt::skip]
const LECTERN: Fields = &[("id", Tag::String(keys::block_entity_type::LECTERN.as_static_str()))];

#[rustfmt::skip]
const SMOKER: Fields = &[("lit_total_time", Tag::Short(0)), ("cooking_time_spent", Tag::Short(0)), ("Items", Tag::List(&[])), ("cooking_total_time", Tag::Short(0)), ("id", Tag::String(keys::block_entity_type::SMOKER.as_static_str())), ("lit_time_remaining", Tag::Short(0)), ("RecipesUsed", Tag::Compound(&[]))];

impl Canvas {
    /// A block and the data it carries.
    pub fn machine(&mut self, at: [i32; 3], state: &str, data: Fields) {
        self.place(&block(state), at[0], at[1], at[2]);
        self.block_entity(at, data);
    }

    /// A `furnace`, `blast_furnace` or `smoker`.
    pub fn furnace(&mut self, at: [i32; 3], kind: &str, facing: Direction) {
        let data = match kind {
            "furnace" => FURNACE,
            "blast_furnace" => BLAST_FURNACE,
            "smoker" => SMOKER,
            other => panic!("`{other}` is not a furnace"),
        };
        let state = format!("minecraft:{kind}[facing={},lit=false]", facing.name());
        self.machine(at, &state, data);
    }

    pub fn lectern(&mut self, at: [i32; 3], facing: Direction) {
        let facing = facing.name();
        let state = format!("minecraft:lectern[facing={facing},has_book=false,powered=false]");
        self.machine(at, &state, LECTERN);
    }

    pub fn brewing_stand(&mut self, at: [i32; 3]) {
        let state =
            "minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]";
        self.machine(at, state, BREWING_STAND);
    }

    pub fn bell(&mut self, at: [i32; 3], attachment: &str, facing: Direction) {
        let facing = facing.name();
        let state =
            format!("minecraft:bell[attachment={attachment},facing={facing},powered=false]");
        self.machine(at, &state, BELL);
    }

    /// A barrel whose lid faces `facing`.
    pub fn barrel(&mut self, at: [i32; 3], facing: Direction) {
        let state = format!("minecraft:barrel[facing={},open=false]", facing.name());
        self.machine(at, &state, BARREL);
    }
}
