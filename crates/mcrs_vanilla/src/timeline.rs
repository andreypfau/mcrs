use std::collections::HashMap;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct Timeline {
    pub clock: String,
    #[serde(default)]
    pub period_ticks: Option<u32>,
    #[serde(default)]
    pub tracks: HashMap<String, Track>,
    #[serde(default)]
    pub time_markers: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub keyframes: Vec<Keyframe>,
    #[serde(default)]
    pub modifier: Option<String>,
    #[serde(default)]
    pub ease: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub ticks: u32,
    pub value: serde_json::Value,
}

/// Timeline data subset for NETWORK_CODEC — mirrors the fields the vanilla
/// 26.1 client expects: clock, optional period_ticks, tracks, time_markers.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkTimeline {
    pub clock: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_ticks: Option<u32>,
    pub tracks: HashMap<String, Track>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub time_markers: HashMap<String, serde_json::Value>,
}

impl From<&Timeline> for NetworkTimeline {
    fn from(tl: &Timeline) -> Self {
        NetworkTimeline {
            clock: tl.clock.clone(),
            period_ticks: tl.period_ticks,
            tracks: tl.tracks.clone(),
            time_markers: tl.time_markers.clone(),
        }
    }
}

impl Asset for Timeline {}

impl VisitAssetDependencies for Timeline {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct TimelineLoader;

#[derive(Debug, thiserror::Error)]
pub enum TimelineLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for TimelineLoader {
    type Asset = Timeline;
    type Settings = ();
    type Error = TimelineLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Timeline, TimelineLoaderError> {
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
    fn network_timeline_round_trips_required_fields() {
        let bytes =
            std::fs::read(assets_dir().join("minecraft/timeline/villager_schedule.json")).unwrap();
        let timeline: Timeline = serde_json::from_slice(&bytes).unwrap();
        let network = NetworkTimeline::from(&timeline);
        let json = serde_json::to_value(&network).unwrap();
        assert!(json.get("clock").is_some());
        assert!(json.get("tracks").is_some());
        assert_eq!(
            json.get("period_ticks").and_then(|v| v.as_u64()),
            Some(24000)
        );
    }

    #[test]
    fn deserialize_all_timelines() {
        let dir = assets_dir().join("minecraft/timeline");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("timeline dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<Timeline>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} timelines failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no timeline files found");
        eprintln!("successfully deserialized {count} timelines");
    }
}
