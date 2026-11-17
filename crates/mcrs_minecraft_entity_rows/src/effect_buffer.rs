use mcrs_minecraft_entity_key::TileKey;

use crate::Keyed;

#[derive(Clone, Debug, PartialEq)]
pub struct EffectBuffer<E> {
    bins: Vec<EffectBin<E>>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EffectBin<E> {
    pub(crate) tile: TileKey,
    pub(crate) entries: Vec<Keyed<E>>,
}

impl<E> Default for EffectBuffer<E> {
    fn default() -> Self {
        Self { bins: Vec::new() }
    }
}

impl<E> EffectBuffer<E> {
    pub fn push(&mut self, tile: TileKey, entry: Keyed<E>) {
        if let Some(bin) = self.bins.iter_mut().find(|bin| bin.tile == tile) {
            bin.entries.push(entry);
        } else {
            self.bins.push(EffectBin {
                tile,
                entries: vec![entry],
            });
        }
    }

    pub(crate) fn drain(&mut self) -> impl Iterator<Item = EffectBin<E>> + '_ {
        self.bins.drain(..)
    }
}
