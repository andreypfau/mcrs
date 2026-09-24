use bevy_asset::Asset;
use bevy_reflect::TypePath;
use mcrs_minecraft_protocol::item;
use serde::{Deserialize, Serialize};

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(transparent)]
pub struct TrimPattern(pub item::TrimPattern);

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(transparent)]
pub struct TrimMaterial(pub item::TrimMaterial);

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! deserialize_all_test {
        ($test_name:ident, $ty:ty, $dir:expr) => {
            #[test]
            fn $test_name() {
                mcrs_minecraft_worldgen_testing::parse_all::<$ty>($dir);
            }
        };
    }

    deserialize_all_test!(
        deserialize_all_trim_patterns,
        TrimPattern,
        "minecraft/trim_pattern"
    );
    deserialize_all_test!(
        deserialize_all_trim_materials,
        TrimMaterial,
        "minecraft/trim_material"
    );
}
