use mcrs_minecraft_nbt::tag::NbtTag;

/// A tag that an [`NbtPath`](crate::NbtPath) selects.
#[derive(Clone, Debug, PartialEq)]
pub enum SelectedTag<'t> {
    /// A tag in the selected tree.
    Tag(&'t NbtTag),
    /// An element of an array, as a byte, an int or a long tag; it is not a tag in the tree.
    Element(NbtTag),
}

impl SelectedTag<'_> {
    /// A copy of the selected tag.
    #[must_use]
    pub fn into_owned(self) -> NbtTag {
        match self {
            SelectedTag::Tag(tag) => tag.clone(),
            SelectedTag::Element(element) => element,
        }
    }
}
