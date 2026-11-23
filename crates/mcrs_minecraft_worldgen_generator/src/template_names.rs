use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{RegistryValue, ResourceLocationBuf};
use mcrs_minecraft_worldgen_feature::pool::{PoolElement, TemplatePool};
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature};
use mcrs_minecraft_worldgen_structure::Structure;
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("the registries hold no {registry}")]
pub struct MissingColumn {
    pub registry: &'static str,
}

fn column<T: RegistryValue + Send + Sync + 'static>(
    registries: &Registries,
) -> Result<&[T], MissingColumn> {
    registries
        .registry::<T>()
        .map(|registry| &registry.values()[..])
        .ok_or(MissingColumn {
            registry: std::any::type_name::<T>(),
        })
}

pub fn pool_templates(pool: &TemplatePool) -> BTreeSet<ResourceLocationBuf> {
    let mut names = TemplateNames::default();
    names.visit_template_pool(pool);
    names.0
}

pub fn named_templates(
    registries: &Registries,
) -> Result<BTreeSet<ResourceLocationBuf>, MissingColumn> {
    let mut names = TemplateNames::default();
    for feature in column::<Feature>(registries)? {
        names.visit_feature(feature);
    }
    for placed in column::<PlacedFeature>(registries)? {
        names.visit_placed_feature(placed);
    }
    for structure in column::<Structure>(registries)? {
        names.visit_structure(structure);
    }
    for pool in column::<TemplatePool>(registries)? {
        names.visit_template_pool(pool);
    }
    Ok(names.0)
}

#[derive(Default)]
struct TemplateNames(BTreeSet<ResourceLocationBuf>);

impl TemplateNames {
    fn visit_structure(&mut self, structure: &Structure) {
        self.0
            .extend(structure.templates().iter().map(|name| name.to_buf()));
    }

    fn visit_feature(&mut self, feature: &Feature) {
        self.0.extend(feature.templates().cloned());
        feature.visit_placed_features(&mut |holder| {
            if let Holder::Inline(placed) = holder {
                self.visit_placed_feature(placed);
            }
        });
    }

    fn visit_placed_feature(&mut self, placed: &PlacedFeature) {
        if let Holder::Inline(feature) = &placed.feature {
            self.visit_feature(feature);
        }
    }

    fn visit_template_pool(&mut self, pool: &TemplatePool) {
        for entry in &pool.elements {
            self.visit_pool_element(&entry.element);
        }
    }

    fn visit_pool_element(&mut self, element: &PoolElement) {
        match element {
            PoolElement::Single(single) | PoolElement::LegacySingle(single) => {
                self.0.insert(single.location.clone());
            }
            PoolElement::List { elements, .. } => {
                for element in elements {
                    self.visit_pool_element(element);
                }
            }
            PoolElement::Feature { feature, .. } => {
                if let Holder::Inline(placed) = feature {
                    self.visit_placed_feature(placed);
                }
            }
            PoolElement::Empty {} => {}
        }
    }
}
