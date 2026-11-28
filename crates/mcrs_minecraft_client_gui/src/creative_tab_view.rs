use mcrs_minecraft_item_stack::ItemStack;

#[derive(Clone, Copy, Debug)]
pub enum CreativeTabView<'a> {
    Stacks(&'a [ItemStack]),
    Search {
        list: &'a [ItemStack],
        result: &'a [usize],
    },
}

impl<'a> CreativeTabView<'a> {
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Stacks(stacks) => stacks.len(),
            Self::Search { result, .. } => result.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&'a ItemStack> {
        match self {
            Self::Stacks(stacks) => stacks.get(index),
            Self::Search { list, result } => list.get(*result.get(index)?),
        }
    }
}
