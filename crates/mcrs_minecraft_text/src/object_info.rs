use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_nbt::nbt_flag;
use serde::{Deserialize, Serialize};

use crate::discriminator::{object_atlas, object_player};
use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_profile::Profile;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectInfo {
    Atlas {
        #[serde(
            rename = "object",
            default,
            deserialize_with = "object_atlas",
            skip_serializing
        )]
        typed: (),
        #[serde(default = "default_atlas", skip_serializing_if = "is_default_atlas")]
        atlas: ResourceLocation,
        sprite: ResourceLocation,
    },
    Player {
        #[serde(
            rename = "object",
            default,
            deserialize_with = "object_player",
            skip_serializing
        )]
        typed: (),
        player: Profile,
        #[serde(
            default = "default_true",
            deserialize_with = "nbt_flag",
            skip_serializing_if = "Clone::clone"
        )]
        hat: bool,
    },
}

fn default_atlas() -> ResourceLocation {
    rl!("minecraft:blocks").to_arc()
}

fn is_default_atlas(atlas: &ResourceLocation) -> bool {
    *atlas == default_atlas()
}
