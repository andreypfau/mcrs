use mcrs_minecraft_resource::ResourceLocation;

pub(crate) const RING_SLOTS: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HomeSlot(u8);

impl HomeSlot {
    pub(crate) fn index(self) -> usize {
        usize::from(self.0)
    }
}

fn text_hash(text: &str) -> u32 {
    text.bytes().fold(0, |hash: u32, byte| {
        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
    })
}

impl From<ResourceLocation<'_>> for HomeSlot {
    fn from(id: ResourceLocation<'_>) -> Self {
        let hash = text_hash(id.namespace())
            .wrapping_mul(31)
            .wrapping_add(text_hash(id.path()));
        let mixed = hash.wrapping_mul(2_654_435_769);
        let [low, ..] = (mixed ^ (mixed >> 16)).to_le_bytes();
        Self(low % RING_SLOTS)
    }
}
