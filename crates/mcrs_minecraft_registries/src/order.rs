use std::cmp::Ordering;

use mcrs_minecraft_resource::ResourceLocation;

pub(crate) fn identifier_order(a: &ResourceLocation, b: &ResourceLocation) -> Ordering {
    a.path()
        .cmp(b.path())
        .then_with(|| a.namespace().cmp(b.namespace()))
}

pub(crate) fn entry_file_order(a: &ResourceLocation, b: &ResourceLocation) -> Ordering {
    const SUFFIX: &[u8] = b".json";
    a.path()
        .bytes()
        .chain(SUFFIX.iter().copied())
        .cmp(b.path().bytes().chain(SUFFIX.iter().copied()))
        .then_with(|| a.namespace().cmp(b.namespace()))
}
