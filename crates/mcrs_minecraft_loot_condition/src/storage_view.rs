use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::BlockHost;
use mcrs_minecraft_resource::ResourceLocation;

use crate::nbt_number::NbtNumber;

pub trait StorageView {
    fn storage_number(&self, storage: ResourceLocation<'_>, path: &NbtPath) -> Option<NbtNumber>;
}

impl<F, E> StorageView for BlockHost<F, E> {
    fn storage_number(&self, _: ResourceLocation<'_>, _: &NbtPath) -> Option<NbtNumber> {
        None
    }
}
