use bevy_app::{App, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetApp, AssetLoader, LoadContext};
use bevy_reflect::TypePath;
use mcrs_minecraft_assets::asset::read_all;
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_core::{ResourceLocation, VERSION};
use mcrs_minecraft_registry::{Registered, RegistrySet};
use mcrs_minecraft_worldgen_feature::pool::{PoolElement, TemplatePool};
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature};
use mcrs_minecraft_worldgen_feature::template::Template;
use mcrs_minecraft_worldgen_structure::Structure;
use std::collections::BTreeSet;
use thiserror::Error;

/// Registers the structure template asset and its loader. Every other worldgen
/// registry is a column of the loaded registry set.
pub struct WorldgenAssetsPlugin;

impl Plugin for WorldgenAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<TemplateAsset>()
            .register_asset_loader(TemplateLoader);
    }
}

#[derive(Debug, Error)]
#[error("{registry} has no parsed column in the registry set")]
pub struct MissingColumn {
    pub registry: &'static str,
}

fn column<R: Registered, T: 'static>(
    set: &RegistrySet,
) -> Result<mcrs_minecraft_registry::Entries<R, T>, MissingColumn> {
    set.entries::<R, T>().ok_or(MissingColumn {
        registry: R::REGISTRY.location().as_static_str(),
    })
}

/// The structure templates a template pool names, in its elements and in the
/// features it writes inline.
pub fn pool_templates(pool: &TemplatePool) -> BTreeSet<ResourceLocation> {
    let mut names = TemplateNames::default();
    names.visit_template_pool(pool);
    names.0
}

/// Every structure template that a feature, a placed feature, a structure or a
/// template pool of `set` names.
pub fn named_templates(set: &RegistrySet) -> Result<BTreeSet<ResourceLocation>, MissingColumn> {
    let mut names = TemplateNames::default();
    for feature in column::<Feature, Feature>(set)?.as_slice() {
        names.visit_feature(feature);
    }
    for placed in column::<PlacedFeature, PlacedFeature>(set)?.as_slice() {
        names.visit_placed_feature(placed);
    }
    for structure in column::<Structure, Structure>(set)?.as_slice() {
        names.visit_structure(structure);
    }
    for pool in column::<TemplatePool, TemplatePool>(set)?.as_slice() {
        names.visit_template_pool(pool);
    }
    Ok(names.0)
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct TemplateAsset {
    pub template: Template,
}

#[derive(Debug, Error)]
pub enum TemplateLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Nbt(#[from] mcrs_minecraft_nbt::Error),
    #[error("{path}: DataVersion {found}, expected {expected}")]
    DataVersion {
        path: String,
        found: i32,
        expected: i32,
    },
}

#[derive(Default, TypePath)]
pub struct TemplateLoader;

impl AssetLoader for TemplateLoader {
    type Asset = TemplateAsset;
    type Settings = ();
    type Error = TemplateLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let template =
            mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes::<Template, _>(bytes.as_slice())?;
        if template.data_version != VERSION.world_version {
            return Err(TemplateLoaderError::DataVersion {
                path: load_context.path().to_string(),
                found: template.data_version,
                expected: VERSION.world_version,
            });
        }
        Ok(TemplateAsset { template })
    }

    fn extensions(&self) -> &[&str] {
        &["nbt"]
    }
}

#[derive(Default)]
struct TemplateNames(BTreeSet<ResourceLocation>);

impl TemplateNames {
    fn visit_structure(&mut self, structure: &Structure) {
        self.0.extend(
            structure
                .templates()
                .iter()
                .map(|path| ResourceLocation::minecraft(path).expect("a hardcoded template name")),
        );
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
