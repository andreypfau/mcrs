use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Reply {
    Ready(Ready),
    Player(PlayerState),
    Target(Target),
    Entities(Vec<EntityState>),
    Block(BlockState),
    Attacked(Target),
    Done,
    Error(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Meshing {
    NotRendered,
    Pending,
    Done,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Ready {
    pub joined: bool,
    pub placed: bool,
    pub terrain: bool,
    pub meshing: Meshing,
    pub world_ready: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PlayerState {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub health: Option<f32>,
    pub food: Option<i32>,
    pub game_mode: Option<String>,
    pub on_ground: bool,
    pub slot: Option<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EntityState {
    pub id: i32,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub distance: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BlockState {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block: String,
    pub state: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Target {
    None,
    Entity(EntityState),
    Block {
        #[serde(flatten)]
        block: BlockState,
        face: String,
    },
}
