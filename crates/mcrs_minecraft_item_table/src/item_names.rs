use alloc::vec::Vec;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

/// The identifier of each item, indexed by the item id, and the item ids in the order of the
/// identifiers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ItemNames<'names> {
    names: &'names [ResourceLocationBuf],
    by_name: &'names [u16],
}

impl<'names> ItemNames<'names> {
    pub(crate) const EMPTY: ItemNames<'static> = ItemNames {
        names: &[],
        by_name: &[],
    };

    pub(crate) fn new(names: &'names [ResourceLocationBuf], by_name: &'names [u16]) -> Self {
        Self { names, by_name }
    }

    /// The item ids of `names` in the order of the identifiers, or the first identifier that
    /// occurs two times.
    pub(crate) fn sort(names: &[ResourceLocationBuf]) -> Result<Vec<u16>, &ResourceLocationBuf> {
        let mut by_name: Vec<u16> = (0..=u16::MAX).take(names.len()).collect();
        by_name.sort_by_key(|id| {
            names
                .get(usize::from(*id))
                .map(ResourceLocationBuf::as_location)
        });
        let repeated = by_name.windows(2).find_map(|pair| {
            let [first, second] = pair else {
                return None;
            };
            let first = names.get(usize::from(*first))?;
            (Some(first) == names.get(usize::from(*second))).then_some(first)
        });
        match repeated {
            Some(name) => Err(name),
            None => Ok(by_name),
        }
    }

    pub(crate) fn id(&self, name: ResourceLocation<'_>) -> Option<u16> {
        let index = self
            .by_name
            .binary_search_by(|id| {
                let entry = self
                    .names
                    .get(usize::from(*id))
                    .map(ResourceLocationBuf::as_location);
                entry.cmp(&Some(name))
            })
            .ok()?;
        self.by_name.get(index).copied()
    }

    pub(crate) fn name(&self, id: u16) -> Option<ResourceLocation<'names>> {
        self.names
            .get(usize::from(id))
            .map(ResourceLocationBuf::as_location)
    }
}
