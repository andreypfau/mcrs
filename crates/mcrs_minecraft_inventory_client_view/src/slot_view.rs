use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::matches_hashed;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SlotView {
    #[default]
    Unknown,
    Stack(Option<ItemStack>),
    Hashed(Option<HashedStack>),
}

impl SlotView {
    #[must_use]
    pub fn matches<L: NameLookup>(
        &self,
        stack: Option<&ItemStack>,
        names: &L,
        scratch: &mut HashScratch,
    ) -> bool {
        match self {
            Self::Unknown => false,
            Self::Stack(sent) => sent.as_ref() == stack,
            Self::Hashed(hashed) => {
                matches_hashed(hashed.as_ref(), stack, names, scratch).unwrap_or(false)
            }
        }
    }
}
