use bevy_asset::{Asset, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TypePath)]
pub struct JukeboxSong {
    pub sound_event: String,
    pub description: serde_json::Value,
    pub length_in_seconds: f32,
    pub comparator_output: u32,
}

impl Asset for JukeboxSong {}

impl VisitAssetDependencies for JukeboxSong {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn deserialize_all_jukebox_songs() {
        mcrs_minecraft_worldgen_testing::parse_all::<super::JukeboxSong>("minecraft/jukebox_song");
    }
}
