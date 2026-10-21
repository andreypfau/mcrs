use std::sync::Arc;

use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_registries::{Refs, RegistryType, Resolved, registry_value};
use serde::{Deserialize, Serialize};

use crate::Text;
use crate::component::common::InlineRef;
use crate::harness::Sample;
use crate::harness::entry;

pub type TrimMaterialRef<M> = InlineRef<M, TrimMaterial, TrimMaterial<M>>;

pub type TrimPatternRef<M> = InlineRef<M, TrimPattern, TrimPattern<M>>;

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimMaterial<M: Refs = Resolved> {
    pub palette_id: ResourceLocation,
    pub description: Text<M>,
}

impl RegistryValue for TrimMaterial {
    type Registry = Self;
}

impl RegistryType for TrimMaterial {
    type Value = ();
}

/// `decal`: absent reads as false, always written.
#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimPattern<M: Refs = Resolved> {
    pub asset_id: ResourceLocation,
    pub description: Text<M>,
    #[serde(default)]
    pub decal: bool,
}

impl RegistryValue for TrimPattern {
    type Registry = Self;
}

impl RegistryType for TrimPattern {
    type Value = ();
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trim<M: Refs = Resolved> {
    pub material: TrimMaterialRef<M>,
    pub pattern: TrimPatternRef<M>,
}

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProvidesTrimMaterial<M: Refs = Resolved>(pub TrimMaterialRef<M>);

fn amethyst() -> TrimMaterialRef<Resolved> {
    InlineRef::Id(entry("amethyst"))
}

fn pal() -> TrimMaterialRef<Resolved> {
    InlineRef::Inline(Arc::new(TrimMaterial {
        palette_id: rl!("mcrs:pal").to_arc(),
        description: Text::text("Pal"),
    }))
}

impl Sample for Trim {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, STRING_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        tags.push(match self.material {
            InlineRef::Id(_) => ("material", STRING_ID),
            InlineRef::Inline(_) => ("material", COMPOUND_ID),
        });
        match self.pattern {
            InlineRef::Id(_) => tags.push(("pattern", STRING_ID)),
            InlineRef::Inline(_) => {
                tags.extend([("pattern", COMPOUND_ID), ("pattern.decal", BYTE_ID)])
            }
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Trim {
                material: amethyst(),
                pattern: InlineRef::Id(entry("coast")),
            },
            Trim {
                material: pal(),
                pattern: InlineRef::Inline(Arc::new(TrimPattern {
                    asset_id: rl!("mcrs:pat").to_arc(),
                    description: Text::translate("trim.mcrs.pat", Vec::new()),
                    decal: true,
                })),
            },
            Trim {
                material: amethyst(),
                pattern: InlineRef::Inline(Arc::new(TrimPattern {
                    asset_id: rl!("mcrs:pat").to_arc(),
                    description: Text::text("Pat"),
                    decal: false,
                })),
            },
        ]
    }
}

impl Sample for ProvidesTrimMaterial {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{COMPOUND_ID, STRING_ID};
        match self.0 {
            InlineRef::Id(_) => vec![("", STRING_ID)],
            InlineRef::Inline(_) => vec![("", COMPOUND_ID), ("palette_id", STRING_ID)],
        }
    }

    fn samples() -> Vec<Self> {
        vec![
            ProvidesTrimMaterial(amethyst()),
            ProvidesTrimMaterial(pal()),
        ]
    }
}
