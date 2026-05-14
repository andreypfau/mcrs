use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

macro_rules! leaf_asset {
    ($name:ident, $loader:ident, $error:ident) => {
        impl Asset for $name {}

        impl VisitAssetDependencies for $name {
            fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
        }

        #[derive(Default, TypePath)]
        pub struct $loader;

        #[derive(Debug, thiserror::Error)]
        pub enum $error {
            #[error(transparent)]
            Io(#[from] std::io::Error),
            #[error("JSON parse error: {0}")]
            Json(#[from] serde_json::Error),
        }

        impl AssetLoader for $loader {
            type Asset = $name;
            type Settings = ();
            type Error = $error;

            async fn load(
                &self,
                reader: &mut dyn Reader,
                _settings: &(),
                _load_context: &mut LoadContext<'_>,
            ) -> Result<$name, $error> {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).await?;
                Ok(serde_json::from_slice(&bytes)?)
            }

            fn extensions(&self) -> &[&str] {
                &[]
            }
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfVariantAssets {
    pub wild: String,
    pub tame: String,
    pub angry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfVariant {
    pub assets: WolfVariantAssets,
    pub baby_assets: WolfVariantAssets,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Option<serde_json::Value>,
}

leaf_asset!(WolfVariant, WolfVariantLoader, WolfVariantLoaderError);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfSounds {
    pub ambient_sound: String,
    pub death_sound: String,
    pub growl_sound: String,
    pub hurt_sound: String,
    pub pant_sound: String,
    pub whine_sound: String,
    pub step_sound: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct WolfSoundVariant {
    pub adult_sounds: WolfSounds,
    pub baby_sounds: WolfSounds,
}

leaf_asset!(
    WolfSoundVariant,
    WolfSoundVariantLoader,
    WolfSoundVariantLoaderError
);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct PigSounds {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub eat_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub step_sound: Option<String>,
    #[serde(default)]
    pub saddle_sound: Option<String>,
    #[serde(default)]
    pub boost_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct PigSoundVariant {
    pub adult_sounds: PigSounds,
    pub baby_sounds: PigSounds,
}

leaf_asset!(
    PigSoundVariant,
    PigSoundVariantLoader,
    PigSoundVariantLoaderError
);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CatSounds {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub beg_for_food_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub eat_sound: Option<String>,
    #[serde(default)]
    pub hiss_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub purr_sound: Option<String>,
    #[serde(default)]
    pub purreow_sound: Option<String>,
    #[serde(default)]
    pub stray_ambient_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CatSoundVariant {
    pub adult_sounds: CatSounds,
    pub baby_sounds: CatSounds,
}

leaf_asset!(
    CatSoundVariant,
    CatSoundVariantLoader,
    CatSoundVariantLoaderError
);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CowSoundVariant {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub step_sound: Option<String>,
    #[serde(default)]
    pub milk_sound: Option<String>,
}

leaf_asset!(
    CowSoundVariant,
    CowSoundVariantLoader,
    CowSoundVariantLoaderError
);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ChickenSounds {
    #[serde(default)]
    pub ambient_sound: Option<String>,
    #[serde(default)]
    pub death_sound: Option<String>,
    #[serde(default)]
    pub hurt_sound: Option<String>,
    #[serde(default)]
    pub step_sound: Option<String>,
    #[serde(default)]
    pub egg_sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ChickenSoundVariant {
    pub adult_sounds: ChickenSounds,
    pub baby_sounds: ChickenSounds,
}

leaf_asset!(
    ChickenSoundVariant,
    ChickenSoundVariantLoader,
    ChickenSoundVariantLoaderError
);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct PigVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Option<serde_json::Value>,
}

leaf_asset!(PigVariant, PigVariantLoader, PigVariantLoaderError);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct FrogVariant {
    pub asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Option<serde_json::Value>,
}

leaf_asset!(FrogVariant, FrogVariantLoader, FrogVariantLoaderError);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CatVariant {
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Option<serde_json::Value>,
}

leaf_asset!(CatVariant, CatVariantLoader, CatVariantLoaderError);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct CowVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Option<serde_json::Value>,
}

leaf_asset!(CowVariant, CowVariantLoader, CowVariantLoaderError);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ChickenVariant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub asset_id: String,
    pub baby_asset_id: String,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Option<serde_json::Value>,
}

leaf_asset!(
    ChickenVariant,
    ChickenVariantLoader,
    ChickenVariantLoaderError
);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct ZombieNautilusVariant {
    pub asset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing)]
    pub spawn_conditions: Option<serde_json::Value>,
}

leaf_asset!(
    ZombieNautilusVariant,
    ZombieNautilusVariantLoader,
    ZombieNautilusVariantLoaderError
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
    }

    macro_rules! deserialize_all_test {
        ($test_name:ident, $ty:ty, $dir:expr) => {
            #[test]
            fn $test_name() {
                let dir = assets_dir().join($dir);
                let mut count = 0;
                let mut failures = Vec::new();

                for entry in std::fs::read_dir(&dir).expect(concat!("dir must exist: ", $dir)) {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("json") {
                        continue;
                    }
                    let bytes = std::fs::read(&path).unwrap();
                    match serde_json::from_slice::<$ty>(&bytes) {
                        Ok(_) => count += 1,
                        Err(e) => failures.push((path.display().to_string(), e.to_string())),
                    }
                }

                if !failures.is_empty() {
                    for (path, err) in &failures {
                        eprintln!("FAIL {path}: {err}");
                    }
                    panic!(
                        "{} of {} entries failed to deserialize",
                        failures.len(),
                        count + failures.len()
                    );
                }

                assert!(count > 0, "no files found in {}", $dir);
                eprintln!("successfully deserialized {count} entries from {}", $dir);
            }
        };
    }

    deserialize_all_test!(deserialize_all_wolf_variants, WolfVariant, "minecraft/wolf_variant");
    deserialize_all_test!(deserialize_all_wolf_sound_variants, WolfSoundVariant, "minecraft/wolf_sound_variant");
    deserialize_all_test!(deserialize_all_pig_variants, PigVariant, "minecraft/pig_variant");
    deserialize_all_test!(deserialize_all_frog_variants, FrogVariant, "minecraft/frog_variant");
    deserialize_all_test!(deserialize_all_cat_variants, CatVariant, "minecraft/cat_variant");
    deserialize_all_test!(deserialize_all_cow_variants, CowVariant, "minecraft/cow_variant");
    deserialize_all_test!(deserialize_all_chicken_variants, ChickenVariant, "minecraft/chicken_variant");
    deserialize_all_test!(deserialize_all_zombie_nautilus_variants, ZombieNautilusVariant, "minecraft/zombie_nautilus_variant");
}
