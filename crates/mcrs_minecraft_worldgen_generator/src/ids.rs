use crate::structures::index::EndBiomes;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block::keys::block_tags;
use mcrs_minecraft_registries::{Diagnostic, Diagnostics, Registries, Registry, Resolver, Tags};
use mcrs_minecraft_resource::{Id, Registered, RegistryValue, ResourceKey, TagId};

pub struct FillIds {
    pub end: EndBiomes,
    pub uncarvable: Option<TagId<Block>>,
    pub block_tags: Tags<Block>,
}

impl FillIds {
    pub fn resolve(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Self> {
        let end = EndBiomes::from_lookup(|key| require_id(registries, key, diagnostics));
        let block_tags = require_tags::<Block>(registries, diagnostics)?;
        Some(Self {
            end: end?,
            uncarvable: block_tags.id(block_tags::UNCARVABLE),
            block_tags,
        })
    }
}

pub(crate) fn require_registry<'a, V: RegistryValue + 'static>(
    registries: &'a Registries,
    diagnostics: &mut Diagnostics,
) -> Option<&'a Registry<V>> {
    let registry = registries.registry::<V>();
    if registry.is_none() {
        let key = <V::Registry as Registered>::REGISTRY;
        diagnostics.extend([Diagnostic::Declaration {
            registry: key.location().into(),
            message: format!("registry {key} is absent from the loaded set"),
        }]);
    }
    registry
}

pub(crate) fn require_tags<R: Registered>(
    registries: &Registries,
    diagnostics: &mut Diagnostics,
) -> Option<Tags<R>> {
    let tags = registries.tags::<R>();
    if tags.is_none() {
        diagnostics.extend([Diagnostic::Declaration {
            registry: R::REGISTRY.location().into(),
            message: format!(
                "tags of registry {} are absent from the loaded set",
                R::REGISTRY
            ),
        }]);
    }
    tags
}

pub(crate) fn require_id<R: Registered>(
    registries: &Registries,
    name: ResourceKey<'static, R>,
    diagnostics: &mut Diagnostics,
) -> Option<Id<R>> {
    let found = registries.id(&name.into());
    if let Err(message) = &found {
        diagnostics.extend([Diagnostic::Entry {
            registry: R::REGISTRY.location().into(),
            entry: name.location().into(),
            message: message.to_string(),
            pack: None,
            file: None,
            field: None,
        }]);
    }
    found.ok()
}
