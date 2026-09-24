pub mod map {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct MapColor {
        pub r: u8,
        pub g: u8,
        pub b: u8,
    }

    impl MapColor {
        pub const fn from_u32(value: u32) -> Self {
            MapColor {
                r: ((value >> 16) & 0xFF) as u8,
                g: ((value >> 8) & 0xFF) as u8,
                b: (value & 0xFF) as u8,
            }
        }

        pub const NONE: MapColor = MapColor::from_u32(0);
    }

    impl From<u32> for MapColor {
        fn from(value: u32) -> Self {
            MapColor::from_u32(value)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum PushReaction {
    #[serde(rename = "push_pull")]
    Normal,
    #[serde(rename = "popped")]
    Destroy,
    #[serde(rename = "immovable")]
    Block,
    #[serde(rename = "ignore_entity")]
    Ignore,
    #[serde(rename = "push")]
    PushOnly,
}
