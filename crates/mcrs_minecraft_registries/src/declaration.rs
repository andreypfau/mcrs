use std::any::{Any, type_name};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use mcrs_minecraft_datapack::{PackFiles, read_json};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::{Registered, ResourceLocationBuf};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::refs::{Resolve, ResolveError};
use crate::registries::{Erased, Kind, Registries, Stage};
use crate::registry::{DataValue, Names, Registry};

pub(crate) enum Source<'f> {
    File { path: &'f str, bytes: &'f [u8] },
    Value(Box<dyn Any + Send + Sync>),
    Sent(&'f NbtTag),
    Known(Vec<u8>),
}

impl<'f> Source<'f> {
    pub(crate) fn file(&self) -> Option<&'f str> {
        match self {
            Source::File { path, .. } => Some(path),
            Source::Value(_) | Source::Sent(_) | Source::Known(_) => None,
        }
    }
}

fn decode<T: Resolve<Named: DeserializeOwned + 'static>>(
    source: Source<'_>,
    registries: &Registries,
) -> Result<T, ResolveError> {
    let named = match source {
        Source::File { bytes, .. } => read_json::<T::Named>(bytes).map_err(ResolveError::json)?,
        Source::Value(value) => *value.downcast::<T::Named>().map_err(|_| {
            ResolveError::new(format!("the value is not a {}", type_name::<T::Named>()))
        })?,
        Source::Known(bytes) => read_json::<T::Named>(&bytes).map_err(ResolveError::json)?,
        Source::Sent(tag) => mcrs_minecraft_nbt::from_tag(tag.clone())
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

    pub fn validate(
        mut self,
        validator: impl Fn(&Registry<V>, &Registries) -> Vec<(usize, String)> + Send + Sync + 'static,
    ) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

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
                (Some(network), source @ (Source::Sent(_) | Source::Known(_))) => {
                    (network.decode)(source, registries)
                }
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

pub(crate) fn entry_path(registry: &str, name: &ResourceLocationBuf) -> String {
    format!("data/{}/{registry}/{}.json", name.namespace(), name.path())
}

pub fn insert_entry<V: DataValue>(
    files: &mut PackFiles,
    pack: &str,
    name: &ResourceLocationBuf,
    value: V::Named,
) where
    V::Named: Send + Sync,
{
    let registry = <V::Registry as Registered>::REGISTRY.location();
    files.insert_value(pack, entry_path(registry.path(), name), Box::new(value));
}

pub struct DeclarationSources {
    pub typed: Vec<Declaration>,
    pub extra: Vec<Declaration>,
}

pub struct Declaration {
    pub(crate) key: ResourceLocationBuf,
    pub(crate) stage: Stage,
    pub(crate) kind: Kind,
    pub(crate) names: Vec<ResourceLocationBuf>,
}

impl Declaration {
    pub fn data<V: DataValue>() -> DataDeclaration<V> {
        DataDeclaration {
            non_empty: false,
            validator: None,
            network: None,
        }
    }

    pub fn names_only(key: ResourceLocationBuf, stage: Stage) -> Declaration {
        Declaration {
            key,
            stage,
            kind: Kind::Names,
            names: Vec::new(),
        }
    }

    pub fn fixed(key: ResourceLocationBuf, names: Vec<ResourceLocationBuf>) -> Declaration {
        Declaration {
            key,
            stage: Stage::Static,
            kind: Kind::Fixed,
            names,
        }
    }

    pub fn key(&self) -> &ResourceLocationBuf {
        &self.key
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn in_stage(mut self, stage: Stage) -> Self {
        if !matches!(self.kind, Kind::Fixed) {
            self.stage = stage;
        }
        self
    }
}

impl DeclarationSources {
    pub fn assemble(
        self,
        fixed: Vec<Declaration>,
        mut stages: BTreeMap<ResourceLocationBuf, Stage>,
    ) -> Vec<Declaration> {
        let DeclarationSources { typed, extra } = self;
        let mut declarations = fixed;
        let replaced: BTreeSet<&ResourceLocationBuf> =
            extra.iter().map(|declaration| &declaration.key).collect();
        let kept: Vec<Declaration> = typed
            .into_iter()
            .filter(|declaration| !replaced.contains(&declaration.key))
            .collect();
        for declaration in kept.into_iter().chain(extra) {
            declarations.push(match stages.remove(&declaration.key) {
                Some(stage) => declaration.in_stage(stage),
                None => declaration,
            });
        }
        declarations.extend(
            stages
                .into_iter()
                .map(|(key, stage)| Declaration::names_only(key, stage)),
        );
        declarations
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

#[derive(Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::prelude::Resource))]
pub struct RegistryAdditions {
    pub declarations: Vec<Declaration>,
    pub embedded_packs: Vec<String>,
    pub files: PackFiles,
    pub local_packs: Vec<String>,
}
