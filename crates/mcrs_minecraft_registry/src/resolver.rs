use std::any::{TypeId, type_name};

use mcrs_minecraft_core::{Id, ResourceLocation, TagId};
use mcrs_minecraft_registries::{Key, Namer, RegistryOf, RegistryType, Resolver};

use crate::set::RegistrySet;

// chisle: a data pack registry of the new loader is typed `RegistryOf<V>` and is bound here under
// `V`, so a lookup tries both; it goes with this set once the client loads through the new loader.
impl RegistrySet {
    fn bound<R: RegistryType>(&self) -> Option<&ResourceLocation> {
        let data_registry = TypeId::of::<R>() == TypeId::of::<RegistryOf<R::Value>>();
        self.bound_name(TypeId::of::<R>()).or_else(|| {
            data_registry
                .then(|| self.bound_name(TypeId::of::<R::Value>()))
                .flatten()
        })
    }

    fn bound_or_err<R: RegistryType>(&self) -> Result<&ResourceLocation, String> {
        self.bound::<R>()
            .ok_or_else(|| format!("no registry is bound to {}", type_name::<R>()))
    }
}

impl Resolver for RegistrySet {
    fn id<R: RegistryType>(&self, key: &Key<R>) -> Result<Id<R>, String> {
        let registry = self.bound_or_err::<R>()?;
        let unknown = || format!("unknown entry {} of {registry}", key.location());
        let table = self.table(registry.as_str()).ok_or_else(unknown)?;
        table
            .number(key.location().as_str())
            .and_then(|number| Id::checked(number, table.len()))
            .ok_or_else(unknown)
    }

    // chisle: a linear scan of the tag names per reference; only loading reaches it, and it goes
    // with this set.
    fn tag<R: RegistryType>(&self, tag: &Key<R>) -> Result<TagId<R>, String> {
        let registry = self.bound_or_err::<R>()?;
        let unknown = || format!("unknown tag #{} of {registry}", tag.location());
        let tags = self.tag_table(registry.as_str()).ok_or_else(unknown)?;
        tags.names()
            .iter()
            .position(|name| name == tag.location())
            .and_then(|at| TagId::checked(u16::try_from(at).ok()?, tags.len()))
            .ok_or_else(unknown)
    }
}

impl Namer for RegistrySet {
    fn name<R: RegistryType>(&self, id: Id<R>) -> Option<&ResourceLocation> {
        self.table(self.bound::<R>()?.as_str())?.name(id.index())
    }

    fn tag_name<R: RegistryType>(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.tag_table(self.bound::<R>()?.as_str())?
            .names()
            .get(tag.index())
    }
}
