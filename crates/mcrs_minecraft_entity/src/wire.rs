use std::io::Write;

use mcrs_minecraft_wire::{Decode, Encode, Result, VarInt};

use crate::keys::{VillagerProfession, VillagerType};
use crate::villager::VillagerData;

impl Encode for VillagerData {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.kind.encode(&mut w)?;
        self.profession.encode(&mut w)?;
        VarInt(self.level()).encode(w)
    }
}

impl Decode<'_> for VillagerData {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let kind = VillagerType::decode(r)?;
        let profession = VillagerProfession::decode(r)?;
        Ok(VillagerData::new(kind, profession, VarInt::decode(r)?.0))
    }
}
