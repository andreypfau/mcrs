use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct TestEnvironment {
    #[serde(rename = "type")]
    pub env_type: String,
    #[serde(default)]
    pub definitions: Vec<serde_json::Value>,
}

impl Asset for TestEnvironment {}

impl VisitAssetDependencies for TestEnvironment {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct TestEnvironmentLoader;

#[derive(Debug, thiserror::Error)]
pub enum TestEnvironmentLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for TestEnvironmentLoader {
    type Asset = TestEnvironment;
    type Settings = ();
    type Error = TestEnvironmentLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<TestEnvironment, TestEnvironmentLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct TestInstance {
    #[serde(rename = "type")]
    pub test_type: String,
    pub function: String,
    pub max_ticks: u32,
    pub setup_ticks: u32,
    pub required: bool,
    pub environment: String,
    pub structure: String,
}

impl Asset for TestInstance {}

impl VisitAssetDependencies for TestInstance {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct TestInstanceLoader;

#[derive(Debug, thiserror::Error)]
pub enum TestInstanceLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for TestInstanceLoader {
    type Asset = TestInstance;
    type Settings = ();
    type Error = TestInstanceLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<TestInstance, TestInstanceLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

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

    #[test]
    fn deserialize_all_test_environments() {
        let dir = assets_dir().join("minecraft/test_environment");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("test_environment dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<TestEnvironment>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} test environments failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no test_environment files found");
        eprintln!("successfully deserialized {count} test environments");
    }

    #[test]
    fn deserialize_all_test_instances() {
        let dir = assets_dir().join("minecraft/test_instance");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("test_instance dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<TestInstance>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} test instances failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no test_instance files found");
        eprintln!("successfully deserialized {count} test instances");
    }
}
