use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_stack::{Builtin, EnchantmentLevel, ItemStack, Kind, PatchEntry};
use mcrs_minecraft_resource::Id;

/// The stack of the parameter `tool`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool<'a> {
    /// The empty stack, for example an empty hand.
    Empty,
    /// A stack that is not empty.
    Stack(&'a ItemStack),
}

impl<'a> Tool<'a> {
    /// The item of the stack; `None` for the empty stack.
    #[must_use]
    pub fn item(&self) -> Option<Kind> {
        match self {
            Tool::Empty => None,
            Tool::Stack(stack) => Some(stack.kind),
        }
    }

    /// The entries of the component `minecraft:enchantments` in the patch of the stack.
    #[must_use]
    pub fn enchantments(&self) -> &'a [EnchantmentLevel] {
        let Tool::Stack(stack) = *self else {
            return &[];
        };
        stack
            .patch()
            .iter()
            .find_map(|entry| match entry {
                PatchEntry::Value(Builtin::Enchantments(enchantments)) => {
                    Some(enchantments.entries())
                }
                _ => None,
            })
            .unwrap_or(&[])
    }

    /// The level of `enchantment` on the tool; 0 without it (LOOT-CND-022).
    #[must_use]
    pub fn level(&self, enchantment: Id<Enchantment>) -> u32 {
        self.enchantments()
            .iter()
            .find(|entry| entry.enchantment.0 == enchantment.number())
            .map_or(0, |entry| u32::from(entry.level.get()))
    }
}
