use std::sync::Arc;

use mcrs_minecraft_registries::{Ref, Refs, Resolved, registry_value};
use mcrs_minecraft_resource::{ResourceLocation, rl};
use serde::{Deserialize, Serialize};

use crate::component::enums::DyeColor;
use crate::harness::Sample;
use crate::harness::entry;
use mcrs_minecraft_wire::{Decode, Encode};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct BannerPattern {
    pub asset_id: ResourceLocation,
    pub translation_key: String,
}

mcrs_minecraft_registries::resolve_as_is!(BannerPattern);

#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(deny_unknown_fields)]
pub struct BannerLayer<M: Refs = Resolved> {
    pub pattern: Ref<M, BannerPattern, BannerPattern>,
    pub color: DyeColor,
}

#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire,
    Clone,
    Debug,
    PartialEq,
    Default,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(transparent)]
pub struct BannerPatterns<M: Refs = Resolved>(pub Vec<BannerLayer<M>>);

impl Sample for BannerPatterns {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            BannerPatterns::default(),
            BannerPatterns(vec![
                BannerLayer {
                    pattern: Ref::Id(entry("creeper")),
                    color: DyeColor::Red,
                },
                BannerLayer {
                    pattern: Ref::Inline(Arc::new(BannerPattern {
                        asset_id: rl!("mcrs:x").to_arc(),
                        translation_key: "block.mcrs.banner.x".into(),
                    })),
                    color: DyeColor::LightBlue,
                },
            ]),
        ]
    }
}
