use crate::{Diagnostic, Diagnostics, Registries, Registry, Resolver, Tags};
use mcrs_minecraft_core::{Id, Registered, RegistryValue, ResourceKey};

pub fn require_registry<'a, V: RegistryValue + 'static>(
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

pub fn require_tags<R: Registered>(
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

pub fn require_id<R: Registered>(
    registries: &Registries,
    name: ResourceKey<R, &'static str>,
    diagnostics: &mut Diagnostics,
) -> Option<Id<R>> {
    let found = registries.id(&name.into());
    if let Err(message) = &found {
        diagnostics.extend([Diagnostic::Entry {
            registry: R::REGISTRY.location().into(),
            entry: (*name.location()).into(),
            message: message.to_string(),
            pack: None,
            file: None,
            field: None,
        }]);
    }
    found.ok()
}
