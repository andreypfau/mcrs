use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_stack::{Builtin, EnchantmentLevel, ItemStack, Kind, PatchEntry};
use mcrs_minecraft_predicate::ItemSubject;
use mcrs_minecraft_resource::Id;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool<'a> {
    Empty,
    Stack(&'a ItemStack),
}

impl<'a> Tool<'a> {
    #[must_use]
    pub fn item(&self) -> Option<Kind> {
        match self {
            Tool::Empty => None,
            Tool::Stack(stack) => Some(stack.kind),
        }
    }

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

    #[must_use]
    pub fn level(&self, enchantment: Id<Enchantment>) -> u32 {
        self.enchantments()
            .iter()
            .find(|entry| entry.enchantment.0 == enchantment.number())
            .map_or(0, |entry| u32::from(entry.level.get()))
    }
}

impl<'a> From<Tool<'a>> for ItemSubject<'a> {
    fn from(tool: Tool<'a>) -> Self {
        match tool {
            Tool::Empty => ItemSubject::Empty,
            Tool::Stack(stack) => ItemSubject::Stack(stack),
        }
    }
}
