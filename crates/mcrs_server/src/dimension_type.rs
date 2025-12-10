use crate::value::IntValueProvider;
use mcrs_protocol::{Ident, ident};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct DimensionType {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_time: Option<i64>,
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub ultrawarm: bool,
    pub natural: bool,
    pub coordinate_scale: f64,
    pub bed_works: bool,
    pub respawn_anchor_works: bool,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub infiniburn: String,
    pub effects: Ident<String>,
    pub ambient_light: f32,
    pub cloud_height: Option<i32>,
    #[serde(flatten)]
    pub monster_settings: MonsterSettings,
}

impl Default for DimensionType {
    fn default() -> Self {
        overworld_dimension_type()
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct MonsterSettings {
    piglin_safe: bool,
    has_raids: bool,
    monster_spawn_light_level: IntValueProvider,
    monster_spawn_block_light_limit: u32,
}

fn overworld_dimension_type() -> DimensionType {
    DimensionType {
        fixed_time: None,
        has_skylight: true,
        has_ceiling: false,
        ultrawarm: false,
        natural: true,
        coordinate_scale: 1.0,
        bed_works: true,
        respawn_anchor_works: false,
        min_y: -64,
        height: 384,
        logical_height: 384,
        infiniburn: "#minecraft:infiniburn_overworld".to_string(),
        effects: ident!("minecraft:overworld").to_string_ident(),
        ambient_light: 0.0,
        cloud_height: Some(192),
        monster_settings: MonsterSettings {
            piglin_safe: false,
            has_raids: true,
            monster_spawn_light_level: IntValueProvider::Tagged(
                crate::value::TaggedIntValueProvider::Uniform {
                    min_inclusive: 0,
                    max_inclusive: 7,
                },
            ),
            monster_spawn_block_light_limit: 0,
        },
    }
}
