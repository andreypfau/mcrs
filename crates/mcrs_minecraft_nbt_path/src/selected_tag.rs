use mcrs_minecraft_nbt::tag::NbtTag;

#[derive(Clone, Debug, PartialEq)]
pub enum SelectedTag<'t> {
    Tag(&'t NbtTag),
    Element(NbtTag),
}

impl SelectedTag<'_> {
    #[must_use]
    pub fn into_owned(self) -> NbtTag {
        match self {
            SelectedTag::Tag(tag) => tag.clone(),
            SelectedTag::Element(element) => element,
        }
    }
}
