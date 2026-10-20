use crate::{Diagnostic, Diagnostics, Registries, Registry, RegistryType, Tags};
use mcrs_minecraft_core::{Id, RegistryKey, ResourceKey, ResourceLocation, TagId, TagKey};

pub fn require_registry<'a, K, R: RegistryType>(
    registries: &'a Registries,
    key: RegistryKey<K>,
    diagnostics: &mut Diagnostics,
) -> Option<&'a Registry<R>> {
    let registry = registries.registry::<R>();
    if registry.is_none() {
        diagnostics.extend([Diagnostic::Declaration {
            registry: key.location().into(),
            message: format!("registry {key} is absent from the loaded set"),
        }]);
    }
    registry
}

pub fn require_tags<R: RegistryType>(
    registries: &Registries,
    key: RegistryKey<R>,
    diagnostics: &mut Diagnostics,
) -> Option<Tags<R>> {
    let tags = registries.tags::<R>();
    if tags.is_none() {
        diagnostics.extend([Diagnostic::Declaration {
            registry: key.location().into(),
            message: format!("tags of registry {key} are absent from the loaded set"),
        }]);
    }
    tags
}

pub fn require_id<K, R: RegistryType>(
    registry: &Registry<R>,
    name: ResourceKey<K, &'static str>,
    diagnostics: &mut Diagnostics,
) -> Option<Id<K>> {
    let entry: ResourceLocation = (*name.location()).into();
    let found = registry.by_name(&entry);
    if found.is_none() {
        diagnostics.extend([Diagnostic::Entry {
            registry: registry.key().clone(),
            message: format!("registry {} holds no entry named {entry}", registry.key()),
            entry,
            pack: None,
            file: None,
            field: None,
        }]);
    }
    // The numbering of an entry in the registry is the numbering the key type uses.
    found.and_then(|id| Id::checked(id.number(), registry.len()))
}

pub fn require_tag<R: RegistryType>(
    tags: &Tags<R>,
    registry: RegistryKey<R>,
    tag: &TagKey<R, &'static str>,
    diagnostics: &mut Diagnostics,
) -> Option<TagId<R>> {
    let name: ResourceLocation = (*tag.location()).into();
    let found = tags.by_name(&name);
    if found.is_none() {
        diagnostics.extend([Diagnostic::MissingTag {
            registry: registry.location().into(),
            tag: name,
        }]);
    }
    found
}
