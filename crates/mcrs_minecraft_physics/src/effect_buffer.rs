use mcrs_engine_muxer::TileKey;

use crate::Keyed;

/// The writes or the claims of one task, binned by the tile of the target. Each task owns its
/// buffer, so a push takes no lock.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectBuffer<E> {
    bins: Vec<EffectBin<E>>,
}

/// The entries of one buffer whose target is in `tile`.
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
        // A task touches a few tiles, so a linear search beats a map.
        if let Some(bin) = self.bins.iter_mut().find(|bin| bin.tile == tile) {
            bin.entries.push(entry);
        } else {
            self.bins.push(EffectBin {
                tile,
                entries: vec![entry],
            });
        }
    }

    /// Takes every bin out of the buffer.
    pub(crate) fn drain(&mut self) -> impl Iterator<Item = EffectBin<E>> + '_ {
        self.bins.drain(..)
    }
}
