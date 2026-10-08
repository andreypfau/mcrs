use bevy_app::{App, Plugin};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetLoader, Handle, LoadContext, UntypedAssetId, VisitAssetDependencies,
};
use bevy_reflect::TypePath;
use mcrs_minecraft_assets::asset::{JsonLoader, read_all};
use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_core::{ResourceLocation, VERSION};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_feature::pool::{PoolElement, TemplatePool};
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_feature::template::Template;
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use thiserror::Error;

/// Registers the worldgen asset types and the loader of the one that names no
/// registry entry, and nothing else. The loaders of the rest parse inside the
/// loaded registry set's scope, so they are registered once that set exists, by
/// [`register_worldgen_loaders`].
pub struct WorldgenAssetsPlugin;

impl Plugin for WorldgenAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<CarverConfigAsset>()
            .init_asset::<FeatureAsset>()
            .init_asset::<PlacedFeatureAsset>()
            .init_asset::<StructureSetAsset>()
            .init_asset::<StructureAsset>()
            .init_asset::<ProcessorListAsset>()
            .init_asset::<TemplateAsset>()
            .register_asset_loader(TemplateLoader);
    }
}

/// Registers the loaders of the worldgen assets that parse names, each holding
/// `registries` so it can read a file inside the set's scope.
pub fn register_worldgen_loaders(app: &mut App, registries: &RegistrySet) {
    app.register_asset_loader(JsonLoader::<CarverConfigAsset>::new(registries.clone()))
        .register_asset_loader(WorldgenAssetLoader::<FeatureAsset>::new(registries.clone()))
        .register_asset_loader(WorldgenAssetLoader::<PlacedFeatureAsset>::new(
            registries.clone(),
        ))
        .register_asset_loader(JsonLoader::<StructureSetAsset>::new(registries.clone()))
        .register_asset_loader(WorldgenAssetLoader::<StructureAsset>::new(
            registries.clone(),
        ))
        .register_asset_loader(JsonLoader::<ProcessorListAsset>::new(registries.clone()));
}

/// The structure templates a template pool names, in its elements and in the
/// features it writes inline.
pub fn pool_templates(pool: &TemplatePool) -> BTreeSet<ResourceLocation> {
    let mut refs = References::default();
    refs.visit_template_pool(pool);
    refs.templates
}

/// The registries a worldgen asset can name. Each one is spelled here once and
/// turned into the set of ids an asset names, the handles those ids load to, and
/// the dependency visitor over them.
///
/// `leaf` registries hold a value with no references of its own; `nested` ones
/// carry their own [`AssetRefs`].
macro_rules! registries {
    (
        leaf { $(($lname:ident, $lfolder:literal, $lasset:ty)),* $(,)? }
        nested {
            $(($nname:ident, $nfolder:literal, $nasset:ident, $nvalue:ty, $nfield:ident, $nvisit:ident)),* $(,)?
        }
    ) => {
        $(
            #[derive(Asset, TypePath, Debug, Clone)]
            pub struct $nasset {
                pub $nfield: $nvalue,
                #[dependency]
                pub deps: AssetRefs,
            }

            impl WorldgenAsset for $nasset {
                type Proto = $nvalue;

                fn references(proto: &Self::Proto) -> References {
                    let mut refs = References::default();
                    refs.$nvisit(proto);
                    refs
                }

                fn build(proto: Self::Proto, deps: AssetRefs) -> Self {
                    Self { $nfield: proto, deps }
                }
            }
        )*

        /// The ids one asset names, one level deep: a referenced asset's own
        /// references are collected when that asset loads.
        #[derive(Default, Debug, PartialEq, Eq)]
        pub(crate) struct References {
            $(pub(crate) $lname: BTreeSet<ResourceLocation>,)*
            $(pub(crate) $nname: BTreeSet<ResourceLocation>,)*
        }

        /// The handles one asset's references resolve to. Every worldgen asset
        /// names ids from the same registries, so one dependency visitor and one
        /// loader serve all of them.
        #[derive(Default, Debug, Clone)]
        pub struct AssetRefs {
            $(pub $lname: BTreeMap<ResourceLocation, Handle<$lasset>>,)*
            $(pub $nname: BTreeMap<ResourceLocation, Handle<$nasset>>,)*
        }

        impl AssetRefs {
            fn load(refs: &References, load_context: &mut LoadContext<'_>) -> Self {
                Self {
                    $($lname: handles(&refs.$lname, $lfolder, load_context),)*
                    $($nname: handles(&refs.$nname, $nfolder, load_context),)*
                }
            }
        }

        impl VisitAssetDependencies for AssetRefs {
            fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
                $(for handle in self.$lname.values() {
                    visit(handle.id().untyped());
                })*
                $(for handle in self.$nname.values() {
                    visit(handle.id().untyped());
                })*
            }
        }
    };
}

registries! {
    leaf {
        (templates, "structure/{}.nbt", TemplateAsset),
        (processor_lists, "worldgen/processor_list/{}.json", ProcessorListAsset),
    }
    nested {
        (features, "worldgen/feature/{}.json", FeatureAsset, Feature, feature, visit_feature),
        (
            placed_features,
            "worldgen/placed_feature/{}.json",
            PlacedFeatureAsset,
            PlacedFeature,
            placed_feature,
            visit_placed_feature
        ),
    }
}

#[derive(Asset, TypePath, Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct CarverConfigAsset {
    pub config: CarverConfig,
}

#[derive(Asset, TypePath, Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct StructureSetAsset {
    pub set: StructureSet,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct StructureAsset {
    pub structure: Structure,
    #[dependency]
    pub deps: AssetRefs,
}

impl WorldgenAsset for StructureAsset {
    type Proto = Structure;

    fn references(structure: &Self::Proto) -> References {
        let mut refs = References::default();
        refs.templates.extend(
            structure
                .templates()
                .iter()
                .map(|path| ResourceLocation::minecraft(path).expect("a hardcoded template name")),
        );
        refs
    }

    fn build(structure: Self::Proto, deps: AssetRefs) -> Self {
        Self { structure, deps }
    }
}

#[derive(Asset, TypePath, Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct ProcessorListAsset {
    pub list: StructureProcessorList,
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

/// The JSON a worldgen asset parses from and the ids that JSON names. Loading is
/// the same for every one of them: parse, turn the ids into handles, keep both.
trait WorldgenAsset: Asset {
    type Proto: DeserializeOwned + Send;

    fn references(proto: &Self::Proto) -> References;

    fn build(proto: Self::Proto, deps: AssetRefs) -> Self;
}

/// The JSON loader for an asset that names other worldgen assets: parse inside
/// the loaded registry set's scope, turn the ids into handles, keep both. A leaf
/// takes [`mcrs_minecraft_assets::asset::JsonLoader`] instead.
#[derive(TypePath)]
pub struct WorldgenAssetLoader<A: TypePath> {
    registries: RegistrySet,
    asset: PhantomData<fn() -> A>,
}

impl<A: TypePath> WorldgenAssetLoader<A> {
    pub fn new(registries: RegistrySet) -> Self {
        Self {
            registries,
            asset: PhantomData,
        }
    }
}

impl<A: WorldgenAsset> AssetLoader for WorldgenAssetLoader<A> {
    type Asset = A;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_all(reader).await?;
        let proto = self
            .registries
            .scope(|| serde_json::from_slice::<A::Proto>(&bytes))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut refs = A::references(&proto);
        retain_shipped_templates(&mut refs.templates, load_context).await;
        let deps = AssetRefs::load(&refs, load_context);
        Ok(A::build(proto, deps))
    }
}

impl References {
    pub(crate) fn visit_feature(&mut self, feature: &Feature) {
        self.templates.extend(feature.templates().cloned());
        let processors = match feature {
            Feature::Template { processors, .. } => processors.iter().collect(),
            Feature::Fossil {
                fossil_processors,
                overlay_processors,
                ..
            } => vec![fossil_processors, overlay_processors],
            _ => Vec::new(),
        };
        for processors in processors {
            if let Holder::Reference(id) = processors {
                self.processor_lists.insert(id.clone());
            }
        }
        feature.visit_placed_features(&mut |holder| match holder {
            Holder::Reference(id) => {
                self.placed_features.insert(id.clone());
            }
            Holder::Inline(placed) => self.visit_placed_feature(placed),
        });
    }

    pub(crate) fn visit_placed_feature(&mut self, placed: &PlacedFeature) {
        match &placed.feature {
            Holder::Reference(id) => {
                self.features.insert(id.clone());
            }
            Holder::Inline(feature) => self.visit_feature(feature),
        }
    }

    pub(crate) fn visit_template_pool(&mut self, pool: &TemplatePool) {
        for entry in &pool.elements {
            self.visit_pool_element(&entry.element);
        }
    }

    fn visit_pool_element(&mut self, element: &PoolElement) {
        match element {
            PoolElement::Single(single) | PoolElement::LegacySingle(single) => {
                self.templates.insert(single.location.clone());
                if let Holder::Reference(id) = &single.processors {
                    self.processor_lists.insert(id.clone());
                }
            }
            PoolElement::List { elements, .. } => {
                for element in elements {
                    self.visit_pool_element(element);
                }
            }
            PoolElement::Feature { feature, .. } => match feature {
                Holder::Reference(id) => {
                    self.placed_features.insert(id.clone());
                }
                Holder::Inline(placed) => self.visit_placed_feature(placed),
            },
            PoolElement::Empty {} => {}
        }
    }
}

/// Vanilla's corpus names templates it does not ship (26.3's ancient city pools list a fifth wall
/// staircase), and vanilla places an empty template for them without a word. A missing file
/// requested as a dependency is logged as an error by the asset server, so it is not requested.
async fn retain_shipped_templates(
    templates: &mut BTreeSet<ResourceLocation>,
    load_context: &mut LoadContext<'_>,
) {
    let mut shipped = BTreeSet::new();
    for id in std::mem::take(templates) {
        let path = format!("{}/structure/{}.nbt", id.namespace(), id.path());
        if load_context.read_asset_bytes(path).await.is_ok() {
            shipped.insert(id);
        }
    }
    *templates = shipped;
}

fn handles<A: Asset>(
    ids: &BTreeSet<ResourceLocation>,
    path_template: &str,
    load_context: &mut LoadContext<'_>,
) -> BTreeMap<ResourceLocation, Handle<A>> {
    ids.iter()
        .map(|id| {
            let handle = load_context.load(format!(
                "{}/{}",
                id.namespace(),
                path_template.replace("{}", id.path())
            ));
            (id.clone(), handle)
        })
        .collect()
}
