use bevy_asset::Asset;
use bevy_reflect::TypePath;
use mcrs_minecraft_item as item;
use serde::{Deserialize, Serialize};

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(transparent)]
pub struct BannerPattern(pub item::BannerPattern);

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(transparent)]
pub struct Instrument(pub item::InstrumentValue);

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(transparent)]
pub struct JukeboxSong(pub item::JukeboxSong);

#[derive(Asset, Debug, Clone, Serialize, Deserialize, TypePath)]
#[serde(transparent)]
pub struct PaintingVariant(pub item::PaintingVariantValue);

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

    #[test]
    fn a_file_the_game_refuses_fails_to_parse() {
        let instrument = |extra: &str| {
            format!(
                r#"{{
                    "sound_event": "minecraft:item.goat_horn.sound.0",
                    "use_duration": 7.0,
                    "range": 256.0,
                    "description": {{"translate": "instrument.minecraft.ponder_goat_horn"}}
                    {extra}
                }}"#
            )
        };
        assert!(serde_json::from_str::<Instrument>(&instrument("")).is_ok());
        assert!(serde_json::from_str::<Instrument>(&instrument(r#", "volume": 1.0"#)).is_err());

        let painting = |width: u32| {
            format!(r#"{{"asset_id": "minecraft:kebab", "width": {width}, "height": 1}}"#)
        };
        assert!(serde_json::from_str::<PaintingVariant>(&painting(16)).is_ok());
        assert!(serde_json::from_str::<PaintingVariant>(&painting(17)).is_err());
    }

    deserialize_all_test!(
        deserialize_all_banner_patterns,
        BannerPattern,
        "minecraft/banner_pattern"
    );
    deserialize_all_test!(
        deserialize_all_instruments,
        Instrument,
        "minecraft/instrument"
    );
    deserialize_all_test!(
        deserialize_all_jukebox_songs,
        JukeboxSong,
        "minecraft/jukebox_song"
    );
    deserialize_all_test!(
        deserialize_all_painting_variants,
        PaintingVariant,
        "minecraft/painting_variant"
    );
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
