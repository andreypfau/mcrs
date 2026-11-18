use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_table::ItemTable;

pub(crate) struct Visible<'a, N> {
    pub(crate) stack: &'a ItemStack,
    pub(crate) items: &'a ItemTable,
    pub(crate) names: &'a N,
    pub(crate) display: &'a TooltipDisplay,
}

impl<'a, N: NameLookup> Visible<'a, N> {
    pub(crate) fn component<T: ItemComponent + 'static>(&self) -> Option<&'a T> {
        if self.shown(T::builtin_key()?) {
            self.items.effective::<T>(self.stack)
        } else {
            None
        }
    }

    pub(crate) fn shown(&self, key: BuiltinKey) -> bool {
        key.location()
            .and_then(|name| self.names.id(NameTable::ComponentType, name))
            .is_none_or(|id| {
                !self
                    .display
                    .hidden_components()
                    .contains(&ComponentType(id))
            })
    }
}
