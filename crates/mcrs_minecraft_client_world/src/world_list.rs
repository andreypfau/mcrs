use std::fmt;
use std::io;

use bevy::prelude::*;
use mcrs_minecraft_game_registries::world_preset_tag_keys::NORMAL;
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_save::{NewWorld, SaveError, WorldSummary};

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum WorldList {
    #[default]
    Reading,
    Ready(Vec<WorldSummary>),
    Unavailable(WorldStoreError),
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum WorldTypes {
    #[default]
    Loading,
    Ready(Vec<ResourceLocationBuf>),
    Failed(WorldStoreError),
}

impl WorldTypes {
    #[must_use]
    pub fn of(registries: &Registries) -> Self {
        let (Some(presets), Some(tags)) = (
            registries.registry::<WorldPreset>(),
            registries.tags::<WorldPreset>(),
        ) else {
            return Self::Ready(Vec::new());
        };
        let tagged: Vec<ResourceLocationBuf> = tags
            .id(NORMAL)
            .into_iter()
            .flat_map(|tag| tags.members(tag))
            .filter_map(|id| presets.name(id).cloned())
            .collect();
        if !tagged.is_empty() {
            return Self::Ready(tagged);
        }
        Self::Ready(presets.iter().map(|(_, name, _)| name.clone()).collect())
    }
}

#[derive(Message, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadWorldList;

#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct CreateWorld(pub NewWorld);

#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct DeleteWorld {
    pub folder_id: String,
}

#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct PlayWorld {
    pub folder_id: String,
}

#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum WorldStoreFailed {
    Create(WorldStoreError),
    Play {
        folder_id: String,
        error: WorldStoreError,
    },
    Delete {
        folder_id: String,
        error: WorldStoreError,
    },
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldStoreError {
    Held,
    Io(io::ErrorKind),
    Encode,
    UnknownWorldPreset,
    DataPack,
    Unplayable,
}

impl From<&SaveError> for WorldStoreError {
    fn from(error: &SaveError) -> Self {
        match error {
            SaveError::Locked { .. } => Self::Held,
            SaveError::Io { source, .. } => Self::Io(source.kind()),
            SaveError::Missing { .. } => Self::Io(io::ErrorKind::NotFound),
            SaveError::UnknownWorldPreset { .. } => Self::UnknownWorldPreset,
            _ => Self::Encode,
        }
    }
}

impl fmt::Display for WorldStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held => f.write_str("the world is open in another process"),
            Self::Io(kind) => write!(f, "the saves folder: {kind}"),
            Self::Encode => f.write_str("a file of the world could not be written"),
            Self::UnknownWorldPreset => f.write_str("the data pack has no such world type"),
            Self::DataPack => f.write_str("the data pack could not be loaded"),
            Self::Unplayable => f.write_str("the world cannot be played"),
        }
    }
}

impl std::error::Error for WorldStoreError {}
