use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::NoWorld;
use mcrs_minecraft_resource::ResourceLocation;

use crate::nbt_number::NbtNumber;

/// The command storages of the server, as the provider `storage` reads them (LOOT-NUM-017).
pub trait StorageView {
    /// The number tag that `path` selects in the command storage `storage`, as the command
    /// `data get` selects it. A storage that does not exist is an empty compound. `None` when
    /// the path selects no tag, more than one tag, or a tag that is not a number.
    // chisle: the host resolves a name on each read; a handle that the host resolves one time lifts it.
    fn storage_number(&self, storage: ResourceLocation<'_>, path: &NbtPath) -> Option<NbtNumber>;
}

impl StorageView for NoWorld {
    fn storage_number(&self, _: ResourceLocation<'_>, _: &NbtPath) -> Option<NbtNumber> {
        None
    }
}
