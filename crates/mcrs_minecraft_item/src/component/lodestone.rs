use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_core::{BlockPos, ResourceKey};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, INT_ARRAY_ID, STRING_ID};
use serde::{Deserialize, Serialize};

use crate::component::common::IntArray;
use crate::harness::Sample;

mod pos {
    use super::*;

    pub(super) fn serialize<S: serde::Serializer>(pos: &BlockPos, s: S) -> Result<S::Ok, S::Error> {
        IntArray([pos.x, pos.y, pos.z]).serialize(s)
    }

    pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<BlockPos, D::Error> {
        let IntArray([x, y, z]) = IntArray::deserialize(d)?;
        Ok(BlockPos::new(x, y, z))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalPos {
    pub dimension: ResourceKey<Dimension>,
    #[serde(with = "pos")]
    pub pos: BlockPos,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LodestoneTracker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<GlobalPos>,
    #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
    pub tracked: bool,
}

impl Default for LodestoneTracker {
    fn default() -> Self {
        LodestoneTracker {
            target: None,
            tracked: true,
        }
    }
}

impl Sample for LodestoneTracker {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        if self.target.is_some() {
            tags.extend([
                ("target", COMPOUND_ID),
                ("target.dimension", STRING_ID),
                ("target.pos", INT_ARRAY_ID),
                ("tracked", BYTE_ID),
            ]);
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            LodestoneTracker::default(),
            LodestoneTracker {
                target: Some(GlobalPos {
                    dimension: mcrs_minecraft_dimension::keys::dimension::THE_NETHER.into(),
                    pos: BlockPos::new(1, -2, 3),
                }),
                tracked: false,
            },
        ]
    }
}
