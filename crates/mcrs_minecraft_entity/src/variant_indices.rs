/// The entity data indices of the variant entries of a type (`entity-tracking.md`,
/// TRK-DATA-006), or `None` for a type without that entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct VariantIndices {
    pub variant: Option<u8>,
    pub sound_variant: Option<u8>,
}

impl VariantIndices {
    /// A type without variant entries.
    pub const NONE: Self = Self {
        variant: None,
        sound_variant: None,
    };

    const fn new(variant: u8, sound_variant: u8) -> Self {
        Self {
            variant: Some(variant),
            sound_variant: Some(sound_variant),
        }
    }

    pub(crate) const PIG: Self = Self::new(19, 20);
    pub(crate) const COW: Self = Self::new(18, 19);
    pub(crate) const CHICKEN: Self = Self::new(18, 19);
}
