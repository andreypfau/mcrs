use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::dimension_keys::{OVERWORLD, THE_END, THE_NETHER};
use mcrs_minecraft_resource::ResourceKeyBuf;

/// The rank of a dimension in the dimension order: the three vanilla dimensions first, then
/// the others by identifier.
pub(crate) fn rank(key: &ResourceKeyBuf<Dimension>) -> (u8, &str) {
    let vanilla = [OVERWORLD, THE_NETHER, THE_END]
        .iter()
        .position(|vanilla| *vanilla == *key)
        .unwrap_or(3) as u8;
    (vanilla, key.as_str())
}
