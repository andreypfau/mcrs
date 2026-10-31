use mcrs_minecraft_resource::ResourceLocationBuf;
use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::discriminator::{source_block, source_entity, source_storage};

/// Where an [`TextContent::Nbt`] component reads its data from.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DataSource {
    Entity {
        #[serde(
            rename = "source",
            default,
            deserialize_with = "source_entity",
            skip_serializing
        )]
        typed: (),
        entity: Cow<'static, str>,
    },
    Block {
        #[serde(
            rename = "source",
            default,
            deserialize_with = "source_block",
            skip_serializing
        )]
        typed: (),
        block: Cow<'static, str>,
    },
    Storage {
        #[serde(
            rename = "source",
            default,
            deserialize_with = "source_storage",
            skip_serializing
        )]
        typed: (),
        storage: ResourceLocationBuf,
    },
}
