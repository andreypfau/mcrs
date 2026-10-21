use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;

use super::ctx::LoadCtx;
use super::network::SyncedEntry;
use super::{Registries, Stage, Table};
use crate::diagnostics::Diagnostic;
use crate::pack::PackIndex;
use crate::registry::{Names, ReadValue, Registry, RegistryType};

pub(crate) enum Source<'f> {
    File { path: &'f str, bytes: &'f [u8] },
    BuiltIn { set: usize },
}

pub(crate) struct EntrySource<'f> {
    pub(crate) pack: PackIndex,
    pub(crate) source: Source<'f>,
}

pub(crate) struct EntryFailure {
    pub(crate) entry: ResourceLocation,
    pub(crate) pack: PackIndex,
    pub(crate) file: Option<String>,
    pub(crate) field: Option<String>,
    pub(crate) message: String,
    pub(crate) reported_as_unresolved: bool,
}

pub(crate) enum Kind<'a> {
    Fixed(&'a dyn ErasedFixed),
    Data(&'a dyn ErasedData),
}

impl Kind<'_> {
    pub(crate) fn stage(&self) -> Option<Stage> {
        match self {
            Kind::Fixed(_) => None,
            Kind::Data(data) => Some(data.stage()),
        }
    }
}

pub(crate) trait Erased: Send + Sync {
    fn key(&self) -> &ResourceLocation;
    fn registry_type(&self) -> TypeId;
    fn aliases(&self) -> &[TypeId] {
        &[]
    }
    fn kind(&self) -> Kind<'_>;
}

pub(crate) trait ErasedFixed {
    fn names(&self) -> &[ResourceLocation];
    fn assemble(&self, names: Arc<Names>) -> Table;
}

pub(crate) trait ErasedData {
    fn stage(&self) -> Stage;
    fn built_ins(&self) -> Vec<(&str, &[ResourceLocation])>;
    fn must_not_be_empty(&self) -> bool;
    fn build(
        &self,
        ctx: &LoadCtx<'_>,
        names: &Arc<Names>,
        sources: Vec<EntrySource<'_>>,
    ) -> Result<Table, Vec<EntryFailure>>;
    fn validate(&self, table: &Table, ctx: &LoadCtx<'_>) -> Result<(), String>;
    fn is_synced(&self) -> bool;
    /// `None` for a registry that is not sent to clients.
    fn synced_entries<'r>(
        &self,
        registries: &'r Registries,
    ) -> Option<Result<Vec<SyncedEntry<'r>>, Diagnostic>>;
}

type Validator<R> = Box<dyn Fn(&Registry<R>, &LoadCtx<'_>) -> Result<(), String> + Send + Sync>;
type Project<R> =
    Box<dyn Fn(&<R as RegistryType>::Value, &Registries) -> Result<NbtTag, String> + Send + Sync>;
type BuiltIn<R> = Box<
    dyn Fn(&ResourceLocation, &LoadCtx<'_>) -> Result<<R as RegistryType>::Value, String>
        + Send
        + Sync,
>;

pub struct DataDeclaration<R: RegistryType> {
    key: ResourceLocation,
    stage: Stage,
    non_empty: bool,
    validator: Option<Validator<R>>,
    built_in: Vec<BuiltInSet<R>>,
    aliases: Vec<TypeId>,
    synced: Option<Project<R>>,
}

struct BuiltInSet<R: RegistryType> {
    pack: String,
    names: Vec<ResourceLocation>,
    value: BuiltIn<R>,
}

impl<R: RegistryType> DataDeclaration<R> {
    pub fn key(&self) -> &ResourceLocation {
        &self.key
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn is_non_empty(&self) -> bool {
        self.non_empty
    }
}

impl<R: RegistryType> DataDeclaration<R>
where
    R::Value: ReadValue,
{
    pub fn non_empty(mut self) -> Self {
        self.non_empty = true;
        self
    }

    pub fn validate(
        mut self,
        validator: impl Fn(&Registry<R>, &LoadCtx<'_>) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// Lets a crate below the one that holds the values name this registry by the marker `K`.
    pub fn named_by<K: RegistryType>(mut self) -> Self {
        self.aliases.push(TypeId::of::<K>());
        self
    }

    /// Sends the registry to clients, each entry as the network NBT `project` makes of it.
    pub fn synced(
        mut self,
        project: impl Fn(&R::Value, &Registries) -> Result<NbtTag, String> + Send + Sync + 'static,
    ) -> Self {
        self.synced = Some(Box::new(project));
        self
    }

    pub fn built_in(
        mut self,
        pack: &str,
        names: Vec<ResourceLocation>,
        value: impl Fn(&ResourceLocation, &LoadCtx<'_>) -> Result<R::Value, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.built_in.push(BuiltInSet {
            pack: pack.to_owned(),
            names,
            value: Box::new(value),
        });
        self
    }

    fn decode(
        &self,
        ctx: &LoadCtx<'_>,
        name: &ResourceLocation,
        source: &Source<'_>,
    ) -> Result<R::Value, (Option<String>, String)> {
        match source {
            Source::File { bytes, .. } => R::Value::read_file(ctx, bytes),
            Source::BuiltIn { set } => match self.built_in.get(*set) {
                Some(set) => (set.value)(name, ctx).map_err(|message| (None, message)),
                None => Err((None, "no built-in value is declared".to_owned())),
            },
        }
    }
}

pub struct Declaration {
    pub(crate) inner: Box<dyn Erased>,
}

impl Declaration {
    pub fn data<R: RegistryType>(key: ResourceLocation, stage: Stage) -> DataDeclaration<R>
    where
        R::Value: ReadValue,
    {
        DataDeclaration {
            key,
            stage,
            non_empty: false,
            validator: None,
            built_in: Vec::new(),
            aliases: Vec::new(),
            synced: None,
        }
    }

    pub fn fixed<R: RegistryType<Value = ()>>(
        key: ResourceLocation,
        names: Vec<ResourceLocation>,
    ) -> Declaration {
        Declaration {
            inner: Box::new(Fixed::<R> {
                key,
                names,
                registry: PhantomData,
            }),
        }
    }
}

impl<R: RegistryType> From<DataDeclaration<R>> for Declaration
where
    R::Value: ReadValue,
{
    fn from(declaration: DataDeclaration<R>) -> Self {
        Declaration {
            inner: Box::new(declaration),
        }
    }
}

impl<R: RegistryType> Erased for DataDeclaration<R>
where
    R::Value: ReadValue,
{
    fn key(&self) -> &ResourceLocation {
        &self.key
    }

    fn registry_type(&self) -> TypeId {
        TypeId::of::<R>()
    }

    fn aliases(&self) -> &[TypeId] {
        &self.aliases
    }

    fn kind(&self) -> Kind<'_> {
        Kind::Data(self)
    }
}

impl<R: RegistryType> ErasedData for DataDeclaration<R>
where
    R::Value: ReadValue,
{
    fn stage(&self) -> Stage {
        self.stage
    }

    fn built_ins(&self) -> Vec<(&str, &[ResourceLocation])> {
        self.built_in
            .iter()
            .map(|set| (set.pack.as_str(), set.names.as_slice()))
            .collect()
    }

    fn must_not_be_empty(&self) -> bool {
        self.non_empty
    }

    fn build(
        &self,
        ctx: &LoadCtx<'_>,
        names: &Arc<Names>,
        sources: Vec<EntrySource<'_>>,
    ) -> Result<Table, Vec<EntryFailure>> {
        let mut values = Vec::with_capacity(sources.len());
        let mut origins = Vec::with_capacity(sources.len());
        let mut failures = Vec::new();
        for (name, entry) in names.all().iter().zip(sources) {
            let before = ctx.enter(&self.key, name);
            match self.decode(ctx, name, &entry.source) {
                Ok(value) => {
                    ctx.settle(before, false);
                    values.push(value);
                    origins.push(entry.pack);
                }
                Err((field, message)) => failures.push(EntryFailure {
                    entry: name.clone(),
                    pack: entry.pack,
                    file: match entry.source {
                        Source::File { path, .. } => Some(path.to_owned()),
                        Source::BuiltIn { .. } => None,
                    },
                    reported_as_unresolved: ctx.settle(before, true),
                    field,
                    message,
                }),
            }
        }
        if !failures.is_empty() {
            return Err(failures);
        }
        Ok(Table {
            names: Arc::clone(names),
            registry: Arc::new(Registry::<R>::new(
                self.key.clone(),
                Arc::clone(names),
                values,
                origins,
            )),
        })
    }

    fn validate(&self, table: &Table, ctx: &LoadCtx<'_>) -> Result<(), String> {
        let Some(validator) = &self.validator else {
            return Ok(());
        };
        match table.registry.downcast_ref::<Registry<R>>() {
            Some(registry) => validator(registry, ctx),
            None => Err("the registry table holds another value type".to_owned()),
        }
    }

    fn is_synced(&self) -> bool {
        self.synced.is_some()
    }

    fn synced_entries<'r>(
        &self,
        registries: &'r Registries,
    ) -> Option<Result<Vec<SyncedEntry<'r>>, Diagnostic>> {
        let project = self.synced.as_ref()?;
        let Some(registry) = registries.registry::<R>() else {
            return Some(Err(Diagnostic::Declaration {
                registry: self.key.clone(),
                message: "the synced registry was not loaded".to_owned(),
            }));
        };
        let entries = registry.iter().map(|(id, name, value)| {
            let tag = project(value, registries).map_err(|message| (name, message))?;
            let mut data = Vec::new();
            tag.serialize(&mut WriteAdaptor::new(&mut data))
                .map_err(|error| (name, error.to_string()))?;
            Ok(SyncedEntry {
                name,
                data,
                known: registries
                    .origin(id)
                    .is_some_and(|pack| pack.known_pack().is_some()),
            })
        });
        Some(
            entries
                .collect::<Result<_, (&ResourceLocation, String)>>()
                .map_err(|(entry, message)| Diagnostic::Entry {
                    registry: self.key.clone(),
                    entry: entry.clone(),
                    pack: None,
                    file: None,
                    field: None,
                    message: format!("cannot be sent to a client: {message}"),
                }),
        )
    }
}

struct Fixed<R> {
    key: ResourceLocation,
    names: Vec<ResourceLocation>,
    registry: PhantomData<fn() -> R>,
}

impl<R: RegistryType<Value = ()>> Erased for Fixed<R> {
    fn key(&self) -> &ResourceLocation {
        &self.key
    }

    fn registry_type(&self) -> TypeId {
        TypeId::of::<R>()
    }

    fn kind(&self) -> Kind<'_> {
        Kind::Fixed(self)
    }
}

impl<R: RegistryType<Value = ()>> ErasedFixed for Fixed<R> {
    fn names(&self) -> &[ResourceLocation] {
        &self.names
    }

    fn assemble(&self, names: Arc<Names>) -> Table {
        let values = vec![(); names.len()];
        Table {
            registry: Arc::new(Registry::<R>::new(
                self.key.clone(),
                Arc::clone(&names),
                values,
                Vec::new(),
            )),
            names,
        }
    }
}
