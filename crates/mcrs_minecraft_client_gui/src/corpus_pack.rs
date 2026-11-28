use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_resource_pack::PackFiles;

pub(crate) fn corpus_pack() -> &'static PackFiles {
    ResourceSet::corpus().files()
}
