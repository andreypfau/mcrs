use std::io::Write;

use crate::item::component::common::TypedEntityData;
use crate::item::component::entity_data::*;
use crate::item::wire::newtype_wire;
use crate::{Decode, Encode, VarInt};

newtype_wire!(EntityData, BlockEntityData, Bees);

impl Encode for BeeOccupant {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.entity_data.encode(&mut w)?;
        VarInt(self.ticks_in_hive).encode(&mut w)?;
        VarInt(self.min_ticks_in_hive).encode(w)
    }
}

impl Decode<'_> for BeeOccupant {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BeeOccupant {
            entity_data: TypedEntityData::decode(r)?,
            ticks_in_hive: VarInt::decode(r)?.0,
            min_ticks_in_hive: VarInt::decode(r)?.0,
        })
    }
}
