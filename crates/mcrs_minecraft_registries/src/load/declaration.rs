use std::any::{Any, type_name};
use std::sync::Arc;

use mcrs_minecraft_core::{Registered, ResourceLocation};
use mcrs_minecraft_datapack::{PackFiles, PackIndex, read_json};
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::ctx::LoadCtx;
use crate::network::SyncedEntry;
use crate::registries::{Erased, Registries, Stage};
use crate::diagnostics::Diagnostic;
use crate::refs::{Resolve, ResolveError};
use crate::registry::{DataValue, Names, Registry};

pub(crate) enum Source<'f> {
    File {
        path: &'f str,
        bytes: &'f [u8],
    },
    Value(Box<dyn Any + Send + Sync>),
    /// An entry a server sent with its data.
    Network(&'f NbtTag),
}

pub(crate) struct EntrySource<'f> {
    pub(crate) pack: PackIndex,
    pub(crate) source: Source<'f>,
}

pub(crate) struct EntryFailure {
    pub(crate) entry: ResourceLocation,
    pub(crate) pack: PackIndex,
    pub(crate) file: Option<String>,
    pub(crate) error: ResolveError,
}

pub(crate) trait ErasedData: Send + Sync {
    fn must_not_be_empty(&self) -> bool;
    fn build(
        &self,
        ctx: &LoadCtx<'_>,
        names: &Arc<Names>,
        sources: Vec<EntrySource<'_>>,
    ) -> Result<Erased, Vec<EntryFailure>>;
    /// The entries the validator refuses, by their position in the registry.
    fn validate(&self, registry: &Erased, ctx: &LoadCtx<'_>) -> Vec<(usize, String)>;
    fn is_synced(&self) -> bool;
    /// `None` for a registry that is not sent to clients.
    fn synced_entries<'r>(
        &self,
        registries: &'r Registries,
    ) -> Option<Result<Vec<SyncedEntry<'r>>, Diagnostic>>;
}

type Validator<V> =
    Box<dyn Fn(&Registry<V>, &LoadCtx<'_>) -> Vec<(usize, String)> + Send + Sync>;
type Project<V> = Box<dyn Fn(&V, &Registries) -> Result<NbtTag, String> + Send + Sync>;
type Receive<V> = Box<dyn Fn(NbtTag, &LoadCtx<'_>) -> Result<V, ResolveError> + Send + Sync>;

pub struct DataDeclaration<V: DataValue> {
    non_empty: bool,
    validator: Option<Validator<V>>,
    synced: Option<Project<V>>,
    received: Option<Receive<V>>,
}

impl<V: DataValue> DataDeclaration<V> {
    pub fn non_empty(mut self) -> Self {
        self.non_empty = true;
        self
    }

    /// Checks the loaded registry as a whole. `validator` names each entry it refuses by its
    /// position in [`Registry::values`].
    pub fn validate(
        mut self,
        validator: impl Fn(&Registry<V>, &LoadCtx<'_>) -> Vec<(usize, String)> + Send + Sync + 'static,
    ) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// Sends the registry to clients, each entry in its named form.
    pub fn synced(self) -> Self
    where
        V::Named: Serialize,
    {
        self.synced_as(|value, registries| named_nbt(value, registries))
    }

    /// Sends the registry to clients, each entry as the network NBT `project` makes of it.
    pub fn synced_as(
        mut self,
        project: impl Fn(&V, &Registries) -> Result<NbtTag, String> + Send + Sync + 'static,
    ) -> Self {
        self.synced = Some(Box::new(project));
        self
    }

    /// Sends the registry to clients as `N` and reads what a server sent back through it, for a
    /// registry whose network form differs from its file form.
    pub fn synced_via<N>(mut self) -> Self
    where
        N: Resolve<Named: Serialize + DeserializeOwned> + for<'v> From<&'v V>,
        V: From<N>,
    {
        self.received = Some(Box::new(|tag, ctx| {
            let named = mcrs_minecraft_nbt::from_tag(tag)
                .map_err(|error| ResolveError::new(error.to_string()))?;
            Ok(V::from(N::resolve(named, ctx)?))
        }));
        self.synced_as(|value, registries| named_nbt(&N::from(value), registries))
    }

    fn key() -> ResourceLocation {
        <V::Registry as Registered>::REGISTRY.location().into()
    }

    fn decode(&self, ctx: &LoadCtx<'_>, source: Source<'_>) -> Result<V, ResolveError> {
        let named = match source {
            Source::File { bytes, .. } => read_json::<V::Named>(bytes)
                .map_err(|error| ResolveError::in_field(error.field, error.message))?,
            Source::Value(value) => *value.downcast::<V::Named>().map_err(|_| {
                ResolveError::new(format!("the value is not a {}", type_name::<V::Named>()))
            })?,
            Source::Network(tag) => match &self.received {
                Some(read) => return read(tag.clone(), ctx),
                None => mcrs_minecraft_nbt::from_tag(tag.clone())
                    .map_err(|error| ResolveError::new(error.to_string()))?,
            },
        };
        V::resolve(named, ctx)
    }
}

/// Puts `value` in `pack` as the entry `name` of the data registry of `V`, at the path of the
/// entry's file. It replaces a file at that path.
pub fn insert_entry<V: DataValue>(
    files: &mut PackFiles,
    pack: &str,
    name: &ResourceLocation,
    value: V::Named,
) where
    V::Named: Send + Sync,
{
    let registry = <V::Registry as Registered>::REGISTRY.location();
    let path = format!(
        "data/{}/{}/{}.json",
        name.namespace(),
        registry.path(),
        name.path()
    );
    files.insert_value(pack, path, Box::new(value));
}

fn named_nbt<T: Resolve<Named: Serialize>>(
    value: &T,
    registries: &Registries,
) -> Result<NbtTag, String> {
    let named = value.name(registries).map_err(|error| error.to_string())?;
    mcrs_minecraft_nbt::to_nbt_tag(&named).map_err(|error| error.to_string())
}

pub(crate) enum Kind {
    Fixed(Vec<ResourceLocation>),
    Names,
    Data(Box<dyn ErasedData>),
}

pub struct Declaration {
    key: ResourceLocation,
    stage: Stage,
    pub(crate) kind: Kind,
}

impl Declaration {
    /// A data registry of `V`, under the key of the registry `V` belongs to, loaded in the world
    /// stage unless [`Declaration::in_stage`] moves it.
    pub fn data<V: DataValue>() -> DataDeclaration<V> {
        DataDeclaration {
            non_empty: false,
            validator: None,
            synced: None,
            received: None,
        }
    }

    /// A data registry whose entries are the names of its files, which are never read.
    pub fn names_only(key: ResourceLocation, stage: Stage) -> Declaration {
        Declaration {
            key,
            stage,
            kind: Kind::Names,
        }
    }

    /// A static registry: its entries are `names`, in id order.
    pub fn fixed(key: ResourceLocation, names: Vec<ResourceLocation>) -> Declaration {
        Declaration {
            key,
            stage: Stage::Static,
            kind: Kind::Fixed(names),
        }
    }

    pub fn key(&self) -> &ResourceLocation {
        &self.key
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Loads a data registry in `stage`. A static registry stays in [`Stage::Static`].
    pub fn in_stage(mut self, stage: Stage) -> Self {
        if !matches!(self.kind, Kind::Fixed(_)) {
            self.stage = stage;
        }
        self
    }

    pub(crate) fn data_part(&self) -> Option<&dyn ErasedData> {
        match &self.kind {
            Kind::Data(data) => Some(data.as_ref()),
            Kind::Fixed(_) | Kind::Names => None,
        }
    }
}

impl<V: DataValue> From<DataDeclaration<V>> for Declaration {
    fn from(declaration: DataDeclaration<V>) -> Self {
        Declaration {
            key: DataDeclaration::<V>::key(),
            stage: Stage::World,
            kind: Kind::Data(Box::new(declaration)),
        }
    }
}

impl<V: DataValue> ErasedData for DataDeclaration<V> {
    fn must_not_be_empty(&self) -> bool {
        self.non_empty
    }

    fn build(
        &self,
        ctx: &LoadCtx<'_>,
        names: &Arc<Names>,
        sources: Vec<EntrySource<'_>>,
    ) -> Result<Erased, Vec<EntryFailure>> {
        let mut values = Vec::with_capacity(sources.len());
        let mut failures = Vec::new();
        for (name, entry) in names.all().iter().zip(sources) {
            let file = match &entry.source {
                Source::File { path, .. } => Some(*path),
                Source::Value(_) | Source::Network(_) => None,
            };
            match self.decode(ctx, entry.source) {
                Ok(value) => values.push(value),
                Err(error) => failures.push(EntryFailure {
                    entry: name.clone(),
                    pack: entry.pack,
                    file: file.map(str::to_owned),
                    error,
                }),
            }
        }
        if !failures.is_empty() {
            return Err(failures);
        }
        Ok(Arc::new(Registry::<V>::new(Arc::clone(names), values)))
    }

    fn validate(&self, registry: &Erased, ctx: &LoadCtx<'_>) -> Vec<(usize, String)> {
        let (Some(validator), Some(registry)) =
            (&self.validator, registry.downcast_ref::<Registry<V>>())
        else {
            return Vec::new();
        };
        validator(registry, ctx)
    }

    fn is_synced(&self) -> bool {
        self.synced.is_some()
    }

    fn synced_entries<'r>(
        &self,
        registries: &'r Registries,
    ) -> Option<Result<Vec<SyncedEntry<'r>>, Diagnostic>> {
        let project = self.synced.as_ref()?;
        let Some(registry) = registries.registry::<V>() else {
            return Some(Err(Diagnostic::Declaration {
                registry: Self::key(),
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
                    registry: Self::key(),
                    entry: entry.clone(),
                    pack: None,
                    file: None,
                    field: None,
                    message: format!("cannot be sent to a client: {message}"),
                }),
        )
    }
}
