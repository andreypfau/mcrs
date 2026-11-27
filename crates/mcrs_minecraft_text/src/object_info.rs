use mcrs_minecraft_nbt::nbt_flag;
use mcrs_minecraft_resource::{ResourceLocationBuf, rl};
use serde::{Deserialize, Serialize};

use crate::discriminator::{object_atlas, object_player};
use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_profile::{Profile, ProfileIdentity};

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
        atlas: ResourceLocationBuf,
        sprite: ResourceLocationBuf,
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

impl ObjectInfo {
    pub fn write_default_text(&self, out: &mut String) {
        match self {
            Self::Atlas { atlas, sprite, .. } => {
                out.push('[');
                out.push_str(short_name(sprite));
                if atlas.as_str() != "minecraft:blocks" {
                    out.push('@');
                    out.push_str(short_name(atlas));
                }
                out.push(']');
            }
            Self::Player { player, .. } => {
                let name = match &player.profile {
                    ProfileIdentity::Full(full) => Some(&full.name),
                    ProfileIdentity::Partial { name, .. } => name.as_ref(),
                };
                match name {
                    Some(name) => {
                        out.push('[');
                        out.push_str(&name.0);
                        out.push_str(" head]");
                    }
                    None => out.push_str("[unknown player head]"),
                }
            }
        }
    }
}

fn short_name(location: &ResourceLocationBuf) -> &str {
    if location.namespace() == "minecraft" {
        location.path()
    } else {
        location.as_str()
    }
}

fn default_atlas() -> ResourceLocationBuf {
    rl!("minecraft:blocks").to_buf()
}

fn is_default_atlas(atlas: &ResourceLocationBuf) -> bool {
    *atlas == default_atlas()
}
