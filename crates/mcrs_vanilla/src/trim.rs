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
pub struct TrimPattern {
    pub asset_id: String,
    pub description: serde_json::Value,
    #[serde(default)]
    pub decal: Option<bool>,
}

leaf_asset!(TrimPattern, TrimPatternLoader, TrimPatternLoaderError);

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct TrimMaterial {
    pub asset_name: String,
    pub description: serde_json::Value,
    #[serde(default)]
    pub override_armor_assets: Option<serde_json::Value>,
}

leaf_asset!(TrimMaterial, TrimMaterialLoader, TrimMaterialLoaderError);

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

    deserialize_all_test!(deserialize_all_trim_patterns, TrimPattern, "minecraft/trim_pattern");
    deserialize_all_test!(deserialize_all_trim_materials, TrimMaterial, "minecraft/trim_material");
}
