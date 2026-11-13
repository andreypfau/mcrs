use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::matches_hashed;

/// What the server knows of the stack that the client holds in one entry or in the cursor. `None`
/// in a variant is the empty stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SlotView {
    #[default]
    Unknown,
    /// The stack that the server sent last, or the stack of a creative slot packet.
    Stack(Option<ItemStack>),
    /// The hashed stack that the last click gave for the entry.
    Hashed(Option<HashedStack>),
}

impl SlotView {
    /// True when the client holds `stack` as far as the server knows. A view that is not known
    /// matches no stack. A hashed stack whose match is not decided does not match, so the
    /// server sends the stack again.
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
