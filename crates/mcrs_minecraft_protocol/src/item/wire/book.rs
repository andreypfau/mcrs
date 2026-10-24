use mcrs_minecraft_registries::Wire;
use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec;

use crate::item::component::book::*;
use crate::item::component::common::Filterable;
use crate::item::wire::record_wire;
use crate::{Decode, Encode, VarInt};

record_wire!(WritableBookContent { pages });

impl Encode for WrittenBookContent<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.title.encode(&mut w)?;
        self.author.encode(&mut w)?;
        VarInt(self.generation.0).encode(&mut w)?;
        self.pages.encode(&mut w)?;
        self.resolved.encode(w)
    }
}

impl Decode<'_> for WrittenBookContent<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let title = Filterable::decode(r)?;
        let author = String::decode(r)?;
        let generation = VarInt::decode(r)?.0;
        ensure!(
            (0..=3).contains(&generation),
            "Generation was {generation}, but must be between 0 and 3"
        );
        Ok(WrittenBookContent {
            title,
            author,
            generation: codec::Bounded(generation),
            pages: Vec::decode(r)?,
            resolved: bool::decode(r)?,
        })
    }
}
