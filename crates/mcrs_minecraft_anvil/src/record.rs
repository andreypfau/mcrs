use std::collections::BTreeMap;

use mcrs_minecraft_nbt::{nbt_int_array, nbt_long_array};
use serde::de::{Error, IgnoredAny};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ticks::{SavedBlockTick, SavedFluidTick};

/// The packed heightmaps a column carries, one long array each. A full column has the four
/// that carry no `_WG` suffix; a column below full has the two `_WG` arrays also.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default, rename_all = "SCREAMING_SNAKE_CASE")]
pub struct Heightmaps {
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "longs")]
    pub world_surface_wg: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "longs")]
    pub world_surface: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "longs")]
    pub ocean_floor_wg: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "longs")]
    pub ocean_floor: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "longs")]
    pub motion_blocking: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "longs")]
    pub motion_blocking_no_leaves: Option<Vec<i64>>,
}

fn longs<S: Serializer>(value: &Option<Vec<i64>>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(words) => nbt_long_array(words, serializer),
        None => serializer.serialize_none(),
    }
}

impl Heightmaps {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The scheduled ticks and block indices that wait for a neighbouring column to upgrade.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeData {
    /// The keys are section numbers as decimal strings.
    #[serde(
        rename = "Indices",
        default,
        skip_serializing_if = "indices_are_empty",
        serialize_with = "int_arrays"
    )]
    pub indices: BTreeMap<String, Vec<i32>>,
    #[serde(rename = "Sides", default)]
    pub sides: i8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neighbor_block_ticks: Vec<SavedBlockTick>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neighbor_fluid_ticks: Vec<SavedFluidTick>,
}

fn indices_are_empty(indices: &BTreeMap<String, Vec<i32>>) -> bool {
    indices.values().all(Vec::is_empty)
}

fn int_arrays<S: Serializer>(
    indices: &BTreeMap<String, Vec<i32>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    struct Ints<'a>(&'a [i32]);

    impl Serialize for Ints<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            nbt_int_array(self.0, serializer)
        }
    }

    serializer.collect_map(indices.iter().map(|(section, ints)| (section, Ints(ints))))
}

impl UpgradeData {
    pub fn is_empty(&self) -> bool {
        self.sides == 0
            && indices_are_empty(&self.indices)
            && self.neighbor_block_ticks.is_empty()
            && self.neighbor_fluid_ticks.is_empty()
    }
}

/// The range of sections that blend with old terrain, and the optional heights of its edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlendingData {
    pub min_section: i32,
    pub max_section: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heights: Option<[f32; 16]>,
}

/// An `entities` list of a column record. Entity save compounds have no typed layout, so the
/// list can only be empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NoEntities;

impl<'de> Deserialize<'de> for NoEntities {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries = Vec::<IgnoredAny>::deserialize(deserializer)?;
        if entries.is_empty() {
            Ok(NoEntities)
        } else {
            Err(D::Error::custom(
                "field `entities` holds entity save compounds, which have no typed layout",
            ))
        }
    }
}

impl Serialize for NoEntities {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(std::iter::empty::<()>())
    }
}
