use crate::value::IntValueProvider;
use mcrs_nbt::compound::NbtCompound;
use mcrs_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Dimension type definition matching the Minecraft 1.21.11+ format.
///
/// Fields like `ultrawarm`, `natural`, `bed_works`, `respawn_anchor_works`,
/// `piglin_safe`, `has_raids`, and `cloud_height` have been moved to the
/// `attributes` map. The `effects` field has been replaced by `skybox` and
/// `cardinal_light`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DimensionType {
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub coordinate_scale: f64,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub infiniburn: String,
    pub ambient_light: f32,
    pub monster_spawn_block_light_limit: u32,
    pub monster_spawn_light_level: IntValueProvider,
    #[serde(default = "default_skybox")]
    pub skybox: String,
    #[serde(default = "default_cardinal_light")]
    pub cardinal_light: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_fixed_time: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_attributes"
    )]
    pub attributes: Option<NbtCompound>,
    #[serde(default, skip_serializing)]
    pub timelines: Option<Value>,
}

fn default_skybox() -> String {
    "overworld".to_string()
}

fn default_cardinal_light() -> String {
    "default".to_string()
}

impl Default for DimensionType {
    fn default() -> Self {
        overworld_dimension_type()
    }
}

fn overworld_dimension_type() -> DimensionType {
    DimensionType {
        has_skylight: true,
        has_ceiling: false,
        coordinate_scale: 1.0,
        min_y: -64,
        height: 384,
        logical_height: 384,
        infiniburn: "#minecraft:infiniburn_overworld".to_string(),
        ambient_light: 0.0,
        monster_spawn_block_light_limit: 0,
        monster_spawn_light_level: IntValueProvider::Tagged(
            crate::value::TaggedIntValueProvider::Uniform {
                min_inclusive: 0,
                max_inclusive: 7,
            },
        ),
        skybox: "overworld".to_string(),
        cardinal_light: "default".to_string(),
        has_fixed_time: None,
        attributes: None,
        timelines: None,
    }
}

fn json_to_nbt_tag(value: &Value) -> NbtTag {
    match value {
        Value::Null => NbtTag::End,
        Value::Bool(b) => NbtTag::Byte(if *b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    NbtTag::Int(i as i32)
                } else {
                    NbtTag::Long(i)
                }
            } else if let Some(f) = n.as_f64() {
                NbtTag::Double(f)
            } else {
                NbtTag::Int(0)
            }
        }
        Value::String(s) => {
            // Hex color strings like "#c0d8ff" or "#FFc0d8ff" → NbtTag::Int
            if (s.len() == 7 || s.len() == 9)
                && s.starts_with('#')
                && s[1..].chars().all(|c| c.is_ascii_hexdigit())
            {
                let parsed = u32::from_str_radix(&s[1..], 16).unwrap();
                NbtTag::Int(parsed as i32)
            } else {
                NbtTag::String(s.clone())
            }
        }
        Value::Object(map) => {
            let mut compound = NbtCompound::default();
            for (k, v) in map {
                compound.child_tags.push((k.clone(), json_to_nbt_tag(v)));
            }
            NbtTag::Compound(compound)
        }
        Value::Array(arr) => {
            NbtTag::List(arr.iter().map(json_to_nbt_tag).collect())
        }
    }
}

fn deserialize_attributes<'de, D>(deserializer: D) -> Result<Option<NbtCompound>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<Value> = Option::deserialize(deserializer)?;
    Ok(opt.map(|v| match json_to_nbt_tag(&v) {
        NbtTag::Compound(c) => c,
        _ => NbtCompound::default(),
    }))
}
