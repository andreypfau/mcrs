use std::any::{Any, type_name};
use std::sync::Arc;

use mcrs_minecraft_core::{Registered, ResourceLocation};
use mcrs_minecraft_datapack::{PackFiles, read_json};
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::refs::{Resolve, ResolveError};
use crate::registries::{Erased, Kind, Registries, Stage};
use crate::registry::{DataValue, Names, Registry};

/// Where one entry is decoded from: a pack file or a value built in its place, both in the file
/// form, or what a server sent, in the network form.
pub(crate) enum Source<'f> {
    File { path: &'f str, bytes: &'f [u8] },
    Value(Box<dyn Any + Send + Sync>),
    Sent(NbtTag),
}

impl<'f> Source<'f> {
    pub(crate) fn file(&self) -> Option<&'f str> {
        match self {
            Source::File { path, .. } => Some(path),
            Source::Value(_) | Source::Sent(_) => None,
        }
    }
}

/// Reads the named form of `T` from `source` and resolves it against `registries`.
fn decode<T: Resolve<Named: DeserializeOwned + 'static>>(
    source: Source<'_>,
    registries: &Registries,
) -> Result<T, ResolveError> {
    let named = match source {
        Source::File { bytes, .. } => read_json::<T::Named>(bytes)
            .map_err(|error| ResolveError::in_field(error.field, error.message))?,
        Source::Value(value) => *value.downcast::<T::Named>().map_err(|_| {
            ResolveError::new(format!("the value is not a {}", type_name::<T::Named>()))
        })?,
        Source::Sent(tag) => mcrs_minecraft_nbt::from_tag(tag)
            .map_err(|error| ResolveError::new(error.to_string()))?,
    };
    T::resolve(named, registries)
}

fn encode<T: Resolve<Named: Serialize>>(
    value: &T,
    registries: &Registries,
) -> Result<NbtTag, String> {
    let named = value.name(registries).map_err(|error| error.to_string())?;
    mcrs_minecraft_nbt::to_nbt_tag(&named).map_err(|error| error.to_string())
}

pub(crate) trait Codec: Send + Sync {
    fn non_empty(&self) -> bool;
    fn synced(&self) -> bool;
    fn decode(
        &self,
        names: &Arc<Names>,
        sources: Vec<Source<'_>>,
        registries: &Registries,
    ) -> Result<Erased, Vec<(usize, ResolveError)>>;
    fn validate(&self, registry: &Erased, registries: &Registries) -> Vec<(usize, String)>;
    fn encode(
        &self,
        registry: &Erased,
        at: usize,
        registries: &Registries,
    ) -> Result<NbtTag, String>;
}

struct Network<V> {
    encode: fn(&V, &Registries) -> Result<NbtTag, String>,
    decode: fn(Source<'_>, &Registries) -> Result<V, ResolveError>,
}

type Validator<V> = Box<dyn Fn(&Registry<V>, &Registries) -> Vec<(usize, String)> + Send + Sync>;

pub struct DataDeclaration<V: DataValue> {
    non_empty: bool,
    validator: Option<Validator<V>>,
    network: Option<Network<V>>,
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
        validator: impl Fn(&Registry<V>, &Registries) -> Vec<(usize, String)> + Send + Sync + 'static,
    ) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// Sends the registry to clients, each entry in its named form.
    pub fn synced(mut self) -> Self
    where
        V::Named: Serialize,
    {
        self.network = Some(Network {
            encode: encode::<V>,
            decode: decode::<V>,
        });
        self
    }

    /// Sends the registry to clients as `N` and reads what a server sent back through it, for a
    /// registry whose network form differs from its file form.
    pub fn synced_via<N>(mut self) -> Self
    where
        N: Resolve<Named: Serialize + DeserializeOwned + 'static> + for<'v> From<&'v V>,
        V: From<N>,
    {
        self.network = Some(Network {
            encode: |value, registries| encode(&N::from(value), registries),
            decode: |source, registries| decode::<N>(source, registries).map(V::from),
        });
        self
    }
}

impl<V: DataValue> Codec for DataDeclaration<V> {
    fn non_empty(&self) -> bool {
        self.non_empty
    }

    fn synced(&self) -> bool {
        self.network.is_some()
    }

    fn decode(
        &self,
        names: &Arc<Names>,
        sources: Vec<Source<'_>>,
        registries: &Registries,
    ) -> Result<Erased, Vec<(usize, ResolveError)>> {
        let mut values = Vec::with_capacity(sources.len());
        let mut failures = Vec::new();
        for (at, source) in sources.into_iter().enumerate() {
            let decoded = match (&self.network, source) {
                (Some(network), source @ Source::Sent(_)) => (network.decode)(source, registries),
                (_, source) => decode::<V>(source, registries),
            };
            match decoded {
                Ok(value) => values.push(value),
                Err(error) => failures.push((at, error)),
            }
        }
        if !failures.is_empty() {
            return Err(failures);
        }
        Ok(Arc::new(Registry::<V>::new(Arc::clone(names), values)))
    }

    fn validate(&self, registry: &Erased, registries: &Registries) -> Vec<(usize, String)> {
        match (&self.validator, registry.downcast_ref::<Registry<V>>()) {
            (Some(validator), Some(registry)) => validator(registry, registries),
            _ => Vec::new(),
        }
    }

    fn encode(
        &self,
        registry: &Erased,
        at: usize,
        registries: &Registries,
    ) -> Result<NbtTag, String> {
        let value = (registry.downcast_ref::<Registry<V>>())
            .and_then(|registry| registry.values().get(at))
            .ok_or("the entry is absent")?;
        let network = self.network.as_ref().ok_or("the registry is not sent")?;
        (network.encode)(value, registries)
    }
}

/// The path of the file of the entry `name` of the registry whose path is `registry`.
pub(crate) fn entry_path(registry: &str, name: &ResourceLocation) -> String {
    format!("data/{}/{registry}/{}.json", name.namespace(), name.path())
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
    files.insert_value(pack, entry_path(registry.path(), name), Box::new(value));
}

pub struct Declaration {
    pub(crate) key: ResourceLocation,
    pub(crate) stage: Stage,
    pub(crate) kind: Kind,
    /// The entries of a static registry, in id order.
    pub(crate) names: Vec<ResourceLocation>,
}

impl Declaration {
    /// A data registry of `V`, under the key of the registry `V` belongs to, loaded in the world
    /// stage unless [`Declaration::in_stage`] moves it.
    pub fn data<V: DataValue>() -> DataDeclaration<V> {
        DataDeclaration {
            non_empty: false,
            validator: None,
            network: None,
        }
    }

    /// A data registry whose entries are the names of its files, which are never read.
    pub fn names_only(key: ResourceLocation, stage: Stage) -> Declaration {
        Declaration {
            key,
            stage,
            kind: Kind::Names,
            names: Vec::new(),
        }
    }

    /// A static registry: its entries are `names`, in id order.
    pub fn fixed(key: ResourceLocation, names: Vec<ResourceLocation>) -> Declaration {
        Declaration {
            key,
            stage: Stage::Static,
            kind: Kind::Fixed,
            names,
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
        if !matches!(self.kind, Kind::Fixed) {
            self.stage = stage;
        }
        self
    }
}

impl<V: DataValue> From<DataDeclaration<V>> for Declaration {
    fn from(declaration: DataDeclaration<V>) -> Self {
        Declaration {
            key: <V::Registry as Registered>::REGISTRY.location().into(),
            stage: Stage::World,
            kind: Kind::Data(Arc::new(declaration)),
            names: Vec::new(),
        }
    }
}
