use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;

use super::ctx::LoadCtx;
use super::{STATIC_RANK, Stage, Table, TagEntry};
use crate::json;
use crate::pack::PackIndex;
use crate::registry::{Names, ReadValue, Registry, RegistryType};
use crate::tags::{TagTable, Tags};

pub(crate) enum Source<'f> {
    File { path: &'f str, bytes: &'f [u8] },
    BuiltIn,
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
    pub(crate) fn rank(&self) -> u8 {
        match self {
            Kind::Fixed(_) => STATIC_RANK,
            Kind::Data(data) => data.stage().rank(),
        }
    }
}

pub(crate) trait Erased: Send + Sync {
    fn key(&self) -> &ResourceLocation;
    fn registry_type(&self) -> TypeId;
    fn kind(&self) -> Kind<'_>;
    fn wrap_tags(&self, table: Arc<TagTable>) -> TagEntry;
}

pub(crate) trait ErasedFixed {
    fn names(&self) -> &[ResourceLocation];
    fn assemble(&self, names: Arc<Names>) -> Table;
}

pub(crate) trait ErasedData {
    fn stage(&self) -> Stage;
    fn built_in_names(&self) -> &[ResourceLocation];
    fn must_not_be_empty(&self) -> bool;
    fn build(
        &self,
        ctx: &LoadCtx<'_>,
        names: &Arc<Names>,
        sources: Vec<EntrySource<'_>>,
    ) -> Result<Table, Vec<EntryFailure>>;
    fn validate(&self, table: &Table, ctx: &LoadCtx<'_>) -> Result<(), String>;
}

type Validator<R> = Box<dyn Fn(&Registry<R>, &LoadCtx<'_>) -> Result<(), String> + Send + Sync>;
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
    built_in: Option<(Vec<ResourceLocation>, BuiltIn<R>)>,
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

    pub fn built_in(
        mut self,
        names: Vec<ResourceLocation>,
        value: impl Fn(&ResourceLocation, &LoadCtx<'_>) -> Result<R::Value, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.built_in = Some((names, Box::new(value)));
        self
    }

    fn decode(
        &self,
        ctx: &LoadCtx<'_>,
        name: &ResourceLocation,
        source: &Source<'_>,
    ) -> Decoded<R> {
        match source {
            Source::File { bytes, .. } => {
                json::read_seed(bytes, ctx.value::<R::Value>()).map_err(|e| (e.field, e.message))
            }
            Source::BuiltIn => match &self.built_in {
                Some((_, value)) => value(name, ctx).map_err(|message| (None, message)),
                None => Err((None, "no built-in value is declared".to_owned())),
            },
        }
    }
}

type Decoded<R> = Result<<R as RegistryType>::Value, (Option<String>, String)>;

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
            built_in: None,
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

fn wrap<R: RegistryType>(table: Arc<TagTable>) -> TagEntry {
    TagEntry {
        typed: Box::new(Tags::<R>::new(Arc::clone(&table))),
        table,
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

    fn kind(&self) -> Kind<'_> {
        Kind::Data(self)
    }

    fn wrap_tags(&self, table: Arc<TagTable>) -> TagEntry {
        wrap::<R>(table)
    }
}

impl<R: RegistryType> ErasedData for DataDeclaration<R>
where
    R::Value: ReadValue,
{
    fn stage(&self) -> Stage {
        self.stage
    }

    fn built_in_names(&self) -> &[ResourceLocation] {
        self.built_in.as_ref().map_or(&[], |(names, _)| names)
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
                        Source::BuiltIn => None,
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

    fn wrap_tags(&self, table: Arc<TagTable>) -> TagEntry {
        wrap::<R>(table)
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
