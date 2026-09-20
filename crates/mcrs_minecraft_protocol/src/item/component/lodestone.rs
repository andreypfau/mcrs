use std::io::Write;

use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, INT_ARRAY_ID, STRING_ID};
use serde::{Deserialize, Serialize};

use crate::item::component::common::{DimensionReg, IntArray};
use crate::item::ctx::ctx_free;
use crate::item::harness::Sample;
use crate::{Decode, Encode};

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
pub struct GlobalPosValue {
    pub dimension: ResourceKey<DimensionReg>,
    #[serde(with = "pos")]
    pub pos: BlockPos,
}

impl Encode for GlobalPosValue {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.dimension.location().encode(&mut w)?;
        self.pos.encode(w)
    }
}

impl Decode<'_> for GlobalPosValue {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(GlobalPosValue {
            dimension: ResourceKey::from_location(ResourceLocation::decode(r)?),
            pos: BlockPos::decode(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct LodestoneTracker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<GlobalPosValue>,
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

ctx_free!(LodestoneTracker);

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
                target: Some(GlobalPosValue {
                    dimension: ResourceKey::from_location(ResourceLocation::minecraft(
                        "the_nether",
                    )),
                    pos: BlockPos::new(1, -2, 3),
                }),
                tracked: false,
            },
        ]
    }
}
