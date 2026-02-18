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
        pub const GRASS: MapColor = MapColor::from_u32(8368696);
        pub const SAND: MapColor = MapColor::from_u32(16247203);
        pub const WOOL: MapColor = MapColor::from_u32(13092807);
        pub const FIRE: MapColor = MapColor::from_u32(16711680);
        pub const ICE: MapColor = MapColor::from_u32(10526975);
        pub const METAL: MapColor = MapColor::from_u32(10987431);
        pub const PLANT: MapColor = MapColor::from_u32(31744);
        pub const SNOW: MapColor = MapColor::from_u32(16777215);
        pub const CLAY: MapColor = MapColor::from_u32(10791096);
        pub const DIRT: MapColor = MapColor::from_u32(9923917);
        pub const STONE: MapColor = MapColor::from_u32(7368816);
        pub const WATER: MapColor = MapColor::from_u32(4210943);
        pub const WOOD: MapColor = MapColor::from_u32(9402184);
        pub const QUARTZ: MapColor = MapColor::from_u32(16776437);
        pub const COLOR_ORANGE: MapColor = MapColor::from_u32(14188339);
        pub const COLOR_MAGENTA: MapColor = MapColor::from_u32(11685080);
        pub const COLOR_LIGHT_BLUE: MapColor = MapColor::from_u32(6724056);
        pub const COLOR_YELLOW: MapColor = MapColor::from_u32(15066419);
        pub const COLOR_LIGHT_GREEN: MapColor = MapColor::from_u32(8375321);
        pub const COLOR_PINK: MapColor = MapColor::from_u32(15892389);
        pub const COLOR_GRAY: MapColor = MapColor::from_u32(5000268);
        pub const COLOR_LIGHT_GRAY: MapColor = MapColor::from_u32(10066329);
        pub const COLOR_CYAN: MapColor = MapColor::from_u32(5013401);
        pub const COLOR_PURPLE: MapColor = MapColor::from_u32(8339378);
        pub const COLOR_BLUE: MapColor = MapColor::from_u32(3361970);
        pub const COLOR_BROWN: MapColor = MapColor::from_u32(6704179);
        pub const COLOR_GREEN: MapColor = MapColor::from_u32(6717235);
        pub const COLOR_RED: MapColor = MapColor::from_u32(10040115);
        pub const COLOR_BLACK: MapColor = MapColor::from_u32(1644825);
        pub const GOLD: MapColor = MapColor::from_u32(16445005);
        pub const DIAMOND: MapColor = MapColor::from_u32(6085589);
        pub const LAPIS: MapColor = MapColor::from_u32(4882687);
        pub const EMERALD: MapColor = MapColor::from_u32(55610);
        pub const PODZOL: MapColor = MapColor::from_u32(8476209);
        pub const NETHER: MapColor = MapColor::from_u32(7340544);
        pub const TERRACOTTA_WHITE: MapColor = MapColor::from_u32(13742497);
        pub const TERRACOTTA_ORANGE: MapColor = MapColor::from_u32(10441252);
        pub const TERRACOTTA_MAGENTA: MapColor = MapColor::from_u32(9787244);
        pub const TERRACOTTA_LIGHT_BLUE: MapColor = MapColor::from_u32(7367818);
        pub const TERRACOTTA_YELLOW: MapColor = MapColor::from_u32(12223780);
        pub const TERRACOTTA_LIGHT_GREEN: MapColor = MapColor::from_u32(6780213);
        pub const TERRACOTTA_PINK: MapColor = MapColor::from_u32(10505550);
        pub const TERRACOTTA_GRAY: MapColor = MapColor::from_u32(3746083);
        pub const TERRACOTTA_LIGHT_GRAY: MapColor = MapColor::from_u32(8874850);
        pub const TERRACOTTA_CYAN: MapColor = MapColor::from_u32(5725276);
        pub const TERRACOTTA_PURPLE: MapColor = MapColor::from_u32(8014168);
        pub const TERRACOTTA_BLUE: MapColor = MapColor::from_u32(4996700);
        pub const TERRACOTTA_BROWN: MapColor = MapColor::from_u32(4993571);
        pub const TERRACOTTA_GREEN: MapColor = MapColor::from_u32(5001770);
        pub const TERRACOTTA_RED: MapColor = MapColor::from_u32(9321518);
        pub const TERRACOTTA_BLACK: MapColor = MapColor::from_u32(2430480);
        pub const CRIMSON_NYLIUM: MapColor = MapColor::from_u32(12398641);
        pub const CRIMSON_STEM: MapColor = MapColor::from_u32(9715553);
        pub const CRIMSON_HYPHAE: MapColor = MapColor::from_u32(6035741);
        pub const WARPED_NYLIUM: MapColor = MapColor::from_u32(1474182);
        pub const WARPED_STEM: MapColor = MapColor::from_u32(3837580);
        pub const WARPED_HYPHAE: MapColor = MapColor::from_u32(5647422);
        pub const WARPED_WART_BLOCK: MapColor = MapColor::from_u32(1356933);
        pub const DEEPSLATE: MapColor = MapColor::from_u32(6579300);
        pub const RAW_IRON: MapColor = MapColor::from_u32(14200723);
        pub const GLOW_LICHEN: MapColor = MapColor::from_u32(8365974);
    }

    impl From<u32> for MapColor {
        fn from(value: u32) -> Self {
            MapColor::from_u32(value)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PushReaction {
    Normal,
    Destroy,
    Block,
    Ignore,
    PushOnly,
}
