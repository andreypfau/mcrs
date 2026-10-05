use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_core::value_provider::{BoundedIntProvider, IntProvider};
use mcrs_minecraft_environment::attribute::EnvironmentAttributeMap;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{EntrySet, Id};

const Y_SIZE: i32 = (1 << 12) - 32;
const MAX_Y: i32 = (Y_SIZE >> 1) - 1;
const MIN_Y: i32 = MAX_Y - Y_SIZE + 1;
const MIN_COORDINATE_SCALE: f64 = 1.0e-5_f32 as f64;
const MAX_COORDINATE_SCALE: f64 = 3.0e7;

fn coordinate_scale<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let scale = f64::deserialize(d)?;
    if !(MIN_COORDINATE_SCALE..=MAX_COORDINATE_SCALE).contains(&scale) {
        return Err(D::Error::custom(format!(
            "Value {scale} outside of range [{MIN_COORDINATE_SCALE}:{MAX_COORDINATE_SCALE}]"
        )));
    }
    Ok(scale)
}

macro_rules! bounded_int {
    ($name:ident -> $ty:ty, $min:expr, $max:expr) => {
        fn $name<'de, D: Deserializer<'de>>(d: D) -> Result<$ty, D::Error> {
            Bounded::<{ $min }, { $max }>::deserialize(d).map(|bounded| bounded.0 as $ty)
        }
    };
}

bounded_int!(min_y -> i32, MIN_Y, MAX_Y);
bounded_int!(height -> u32, 16, Y_SIZE);
bounded_int!(logical_height -> u32, 0, Y_SIZE);
bounded_int!(monster_spawn_block_light_limit -> u32, 0, 15);

fn monster_spawn_light_level<'de, D: Deserializer<'de>>(d: D) -> Result<IntProvider, D::Error> {
    BoundedIntProvider::<0, 15>::deserialize(d).map(|provider| provider.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct DimensionType {
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub has_ender_dragon_fight: bool,
    #[serde(deserialize_with = "coordinate_scale")]
    pub coordinate_scale: f64,
    #[serde(deserialize_with = "min_y")]
    pub min_y: i32,
    #[serde(deserialize_with = "height")]
    pub height: u32,
    #[serde(deserialize_with = "logical_height")]
    pub logical_height: u32,
    pub infiniburn: EntrySet<keys::Block>,
    pub ambient_light: f32,
    #[serde(deserialize_with = "monster_spawn_block_light_limit")]
    pub monster_spawn_block_light_limit: u32,
    #[serde(deserialize_with = "monster_spawn_light_level")]
    pub monster_spawn_light_level: IntProvider,
    #[serde(default, skip_serializing_if = "is_default")]
    pub skybox: Skybox,
    #[serde(default, skip_serializing_if = "is_default")]
    pub cardinal_light: CardinalLight,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_fixed_time: Option<bool>,
    #[serde(default, skip_serializing_if = "EnvironmentAttributeMap::is_empty")]
    pub attributes: EnvironmentAttributeMap,
    #[serde(default, skip_serializing_if = "is_default")]
    pub timelines: EntrySet<keys::Timeline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_clock: Option<Id<keys::WorldClock>>,
}

impl<'de> Deserialize<'de> for DimensionType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let dimension_type = DimensionType::deserialize(d)?;
        let (min_y, height) = (dimension_type.min_y, dimension_type.height as i32);
        let refused = if min_y + height > MAX_Y + 1 {
            Some(format!(
                "min_y + height cannot be higher than: {}",
                MAX_Y + 1
            ))
        } else if dimension_type.logical_height > dimension_type.height {
            Some("logical_height cannot be higher than height".to_owned())
        } else if height % 16 != 0 {
            Some("height has to be multiple of 16".to_owned())
        } else if min_y % 16 != 0 {
            Some("min_y has to be a multiple of 16".to_owned())
        } else {
            None
        };
        match refused {
            Some(reason) => Err(D::Error::custom(reason)),
            None => Ok(dimension_type),
        }
    }
}

impl Serialize for DimensionType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        DimensionType::serialize(self, s)
    }
}

/// DimensionType data subset for NETWORK_CODEC.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkDimensionType {
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub has_ender_dragon_fight: bool,
    pub coordinate_scale: f64,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub infiniburn: EntrySet<keys::Block>,
    pub ambient_light: f32,
    pub monster_spawn_block_light_limit: u32,
    pub monster_spawn_light_level: IntProvider,
    #[serde(skip_serializing_if = "is_default")]
    pub skybox: Skybox,
    #[serde(skip_serializing_if = "is_default")]
    pub cardinal_light: CardinalLight,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_fixed_time: Option<bool>,
    #[serde(skip_serializing_if = "EnvironmentAttributeMap::is_empty")]
    pub attributes: EnvironmentAttributeMap,
    #[serde(skip_serializing_if = "is_default")]
    pub timelines: EntrySet<keys::Timeline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_clock: Option<Id<keys::WorldClock>>,
}

impl From<&DimensionType> for NetworkDimensionType {
    fn from(dt: &DimensionType) -> Self {
        NetworkDimensionType {
            has_skylight: dt.has_skylight,
            has_ceiling: dt.has_ceiling,
            has_ender_dragon_fight: dt.has_ender_dragon_fight,
            coordinate_scale: dt.coordinate_scale,
            min_y: dt.min_y,
            height: dt.height,
            logical_height: dt.logical_height,
            infiniburn: dt.infiniburn.clone(),
            ambient_light: dt.ambient_light,
            monster_spawn_block_light_limit: dt.monster_spawn_block_light_limit,
            monster_spawn_light_level: dt.monster_spawn_light_level.clone(),
            skybox: dt.skybox,
            cardinal_light: dt.cardinal_light.clone(),
            has_fixed_time: dt.has_fixed_time,
            attributes: dt.attributes.filter_syncable(),
            timelines: dt.timelines.clone(),
            default_clock: dt.default_clock,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skybox {
    #[default]
    #[serde(rename = "overworld")]
    Overworld,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "end")]
    End,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CardinalLight {
    #[default]
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "nether")]
    Nether,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_testing::{assets_dir, dimension_type_set, packs, reencode};
    use serde_json::Value;
    use std::path::PathBuf;

    fn dimension_type_dirs() -> Vec<PathBuf> {
        std::iter::once(assets_dir())
            .chain(packs())
            .map(|root| root.join("minecraft/dimension_type"))
            .filter(|dir| dir.is_dir())
            .collect()
    }

    fn read(name: &str) -> DimensionType {
        let path = dimension_type_dirs()
            .into_iter()
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .unwrap();
        dimension_type_set()
            .scope(|| serde_json::from_slice(&std::fs::read(path).unwrap()))
            .unwrap()
    }

    #[test]
    fn the_network_dimension_type_is_the_games() {
        let sent = |name: &str| {
            dimension_type_set()
                .scope(|| serde_json::to_value(NetworkDimensionType::from(&read(name))))
                .unwrap()
        };

        let overworld = sent("overworld.json");
        assert!(overworld.get("skybox").is_none(), "{overworld}");
        assert!(overworld.get("cardinal_light").is_none(), "{overworld}");
        assert_eq!(overworld["infiniburn"], "#minecraft:infiniburn_overworld");
        assert_eq!(overworld["timelines"], "#minecraft:in_overworld");
        assert_eq!(overworld["default_clock"], "minecraft:overworld");
        let attributes = overworld["attributes"].as_object().unwrap();
        assert!(!attributes.is_empty());
        for id in attributes.keys() {
            assert!(
                mcrs_minecraft_environment::attribute::is_syncable(id),
                "{id} is not syncable"
            );
        }
        assert!(
            !attributes.contains_key("minecraft:gameplay/bed_rule"),
            "{overworld}"
        );

        let nether = sent("the_nether.json");
        assert_eq!(nether["skybox"], "none");
        assert_eq!(nether["cardinal_light"], "nether");
        assert!(nether.get("default_clock").is_none(), "{nether}");
    }

    #[test]
    fn every_dimension_type_parses_through_the_registry() {
        let mut count = 0;
        for entry in dimension_type_dirs().into_iter().flat_map(|dir| {
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        }) {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let raw: Value = serde_json::from_slice(&bytes).unwrap();
            let parsed: DimensionType = dimension_type_set()
                .scope(|| serde_json::from_slice(&bytes))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

            assert!(
                !parsed.attributes.is_empty(),
                "{} has attributes",
                path.display()
            );
            assert_eq!(
                reencode(&parsed.attributes),
                raw["attributes"],
                "{} attributes must round-trip unchanged",
                path.display()
            );
            let written: Value = serde_json::from_str(
                &dimension_type_set().scope(|| serde_json::to_string(&parsed).unwrap()),
            )
            .unwrap();
            assert_eq!(written, raw, "{} must round-trip unchanged", path.display());
            count += 1;
        }
        assert_eq!(count, 5);
    }
}
