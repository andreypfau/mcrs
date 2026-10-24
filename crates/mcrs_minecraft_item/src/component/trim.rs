use std::sync::Arc;

use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_registries::{Ref, Refs, Resolved, registry_value};
use serde::{Deserialize, Serialize};

use crate::Text;
use crate::harness::Sample;
use crate::harness::entry;

pub type TrimMaterialRef<M> = Ref<M, TrimMaterial, TrimMaterial<M>>;

pub type TrimPatternRef<M> = Ref<M, TrimPattern, TrimPattern<M>>;

#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimMaterial<M: Refs = Resolved> {
    pub palette_id: ResourceLocation,
    pub description: Text<M::Nbt>,
}

/// `decal`: absent reads as false, always written.
#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimPattern<M: Refs = Resolved> {
    pub asset_id: ResourceLocation,
    pub description: Text<M::Nbt>,
    #[serde(default)]
    pub decal: bool,
}

#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trim<M: Refs = Resolved> {
    pub material: TrimMaterialRef<M>,
    pub pattern: TrimPatternRef<M>,
}

#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProvidesTrimMaterial<M: Refs = Resolved>(pub TrimMaterialRef<M>);

fn amethyst() -> TrimMaterialRef<Resolved> {
    Ref::Id(entry("amethyst"))
}

fn pal() -> TrimMaterialRef<Resolved> {
    Ref::Inline(Arc::new(TrimMaterial {
        palette_id: rl!("mcrs:pal").to_arc(),
        description: Text::text("Pal"),
    }))
}

impl Sample for Trim {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, STRING_ID};
        let mut tags = vec![("", COMPOUND_ID)];
        tags.push(match self.material {
            Ref::Id(_) => ("material", STRING_ID),
            Ref::Inline(_) => ("material", COMPOUND_ID),
        });
        match self.pattern {
            Ref::Id(_) => tags.push(("pattern", STRING_ID)),
            Ref::Inline(_) => tags.extend([("pattern", COMPOUND_ID), ("pattern.decal", BYTE_ID)]),
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            Trim {
                material: amethyst(),
                pattern: Ref::Id(entry("coast")),
            },
            Trim {
                material: pal(),
                pattern: Ref::Inline(Arc::new(TrimPattern {
                    asset_id: rl!("mcrs:pat").to_arc(),
                    description: Text::translate("trim.mcrs.pat", Vec::new()),
                    decal: true,
                })),
            },
            Trim {
                material: amethyst(),
                pattern: Ref::Inline(Arc::new(TrimPattern {
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
            Ref::Id(_) => vec![("", STRING_ID)],
            Ref::Inline(_) => vec![("", COMPOUND_ID), ("palette_id", STRING_ID)],
        }
    }

    fn samples() -> Vec<Self> {
        vec![
            ProvidesTrimMaterial(amethyst()),
            ProvidesTrimMaterial(pal()),
        ]
    }
}
