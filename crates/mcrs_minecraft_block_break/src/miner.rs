#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Miner {
    pub item_speed: f32,
    pub harvests: bool,
    pub on_ground: bool,
    pub eyes_in_water: bool,
}
