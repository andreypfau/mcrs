use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::keys::{BLOCK, Block, FLUID, Fluid};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::resource_location::StaticResourceLocation;
use mcrs_minecraft_entity::keys::{
    ATTRIBUTE, Attribute, ENTITY_TYPE, EntityType, VILLAGER_TYPE, VillagerType,
};
use mcrs_minecraft_item::keys::{ITEM, Item, MOB_EFFECT, MobEffect, POTION, Potion};
use mcrs_minecraft_registries::{
    DataDeclaration, Declaration, LoadCtx, PackStack, ReadValue, Registries, RegistryOf,
    RegistryType, Stage, StageView,
};
use mcrs_minecraft_registry::{
    AssemblyError, NameTable, RegistrySet, TagTable, VANILLA_PACK, WorldRegistries,
};
use mcrs_minecraft_sound::SoundEvent;
use mcrs_minecraft_sound::keys::SOUND_EVENT;
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

use crate::registries::{reloadable_declarations, world_declarations};

pub(crate) const BETA_PACK: &str = "file/beta";

const DIMENSION: &str = "minecraft:dimension";

// chisle: a names-only registry still parses every file through `IgnoredAny` (about 1,900
// advancements on each load) because the loader only knows registries that decode values; a
// names-only declaration in the loader lifts this.
pub(crate) struct Unparsed;

impl ReadValue for Unparsed {
    fn read<'de, D: Deserializer<'de>>(_: &LoadCtx<'_>, deserializer: D) -> Result<Self, D::Error> {
        IgnoredAny::deserialize(deserializer).map(|_| Unparsed)
    }
}

pub(crate) struct StaticRegistry<const N: usize>;

impl<const N: usize> RegistryType for StaticRegistry<N> {
    type Value = ();
}

pub(crate) struct NamesOnly<const N: usize>;

impl<const N: usize> RegistryType for NamesOnly<N> {
    type Value = Unparsed;
}

pub(crate) type OriginsFn = fn(&Registries, &PackStack) -> Option<Vec<u32>>;
pub(crate) type AttachFn =
    fn(&Registries, &WorldRegistries, RegistrySet, &str) -> Result<RegistrySet, AssemblyError>;

fn origins<R: RegistryType>(registries: &Registries, stack: &PackStack) -> Option<Vec<u32>> {
    let registry = registries.registry::<R>()?;
    registry
        .iter()
        .map(|(id, _, _)| {
            let pack = stack.position(registries.origin(id)?.id())?;
            u32::try_from(pack.index()).ok()
        })
        .collect()
}

fn attach<V: Send + Sync + 'static>(
    registries: &Registries,
    table: &WorldRegistries,
    set: RegistrySet,
    key: &str,
) -> Result<RegistrySet, AssemblyError> {
    match registries.registry::<RegistryOf<V>>() {
        Some(registry) => table.attach(set, key, Arc::clone(registry.values())),
        None => Err(AssemblyError::UnknownRegistry {
            registry: key.to_owned(),
        }),
    }
}

// chisle: each static or names-only registry needs its own type to be declared, so their count is
// capped by this list; a key-addressed declaration in the loader lifts the cap. A static registry
// is typed when something reads it.
macro_rules! numbered {
    ($($n:literal)*) => {
        fn fixed(at: usize, key: ResourceLocation, names: Vec<ResourceLocation>) -> Option<Declaration> {
            match at {
                $($n => Some(Declaration::fixed::<StaticRegistry<$n>>(key, names)),)*
                _ => None,
            }
        }

        fn names_only(at: usize, key: ResourceLocation, stage: Stage) -> Option<(Declaration, OriginsFn)> {
            match at {
                $($n => Some((
                    Declaration::data::<NamesOnly<$n>>(key, stage).into(),
                    origins::<NamesOnly<$n>> as OriginsFn,
                )),)*
                _ => None,
            }
        }
    };
}

numbered!(0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63 64 65 66 67 68 69 70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87 88 89 90 91 92 93 94 95 96 97 98 99 100 101 102 103 104 105 106 107 108 109 110 111 112 113 114 115 116 117 118 119 120 121 122 123 124 125 126 127);

type DeclareStatic = fn(ResourceLocation, Vec<ResourceLocation>) -> Declaration;

const TYPED_STATICS: [(StaticResourceLocation, DeclareStatic); 9] = [
    (BLOCK.location(), Declaration::fixed::<Block>),
    (ITEM.location(), Declaration::fixed::<Item>),
    (FLUID.location(), Declaration::fixed::<Fluid>),
    (SOUND_EVENT.location(), Declaration::fixed::<SoundEvent>),
    (ATTRIBUTE.location(), Declaration::fixed::<Attribute>),
    (MOB_EFFECT.location(), Declaration::fixed::<MobEffect>),
    (POTION.location(), Declaration::fixed::<Potion>),
    (ENTITY_TYPE.location(), Declaration::fixed::<EntityType>),
    (VILLAGER_TYPE.location(), Declaration::fixed::<VillagerType>),
];

struct Grown {
    set: RegistrySet,
    tagged: HashSet<ResourceLocation>,
}

#[derive(Clone)]
pub(crate) struct StageSet(Arc<Mutex<Grown>>);

impl StageSet {
    pub(crate) fn new(statics: RegistrySet) -> Self {
        Self(Arc::new(Mutex::new(Grown {
            set: statics,
            tagged: HashSet::new(),
        })))
    }

    pub(crate) fn get(&self) -> RegistrySet {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set
            .clone()
    }

    pub(crate) fn scope(&self) -> impl Fn() -> RegistrySet + Clone + Send + Sync + 'static {
        let stage = self.clone();
        move || stage.get()
    }

    pub(crate) fn grow(&self, view: &StageView) -> Result<RegistrySet, AssemblyError> {
        let mut guard = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let grown = &mut *guard;
        let mut added = Vec::new();
        for registry in view.registries() {
            if grown.set.table(registry.key().as_str()).is_none() {
                let names = registry.names().iter().cloned();
                added.push(Arc::new(NameTable::new(registry.key().clone(), names)?));
            }
        }
        let mut set = grown.set.clone().with_tables(added)?;
        for registry in view.registries() {
            if !grown.tagged.insert(registry.key().clone()) {
                continue;
            }
            let Some(table) = set.table(registry.key().as_str()).cloned() else {
                continue;
            };
            let tags = registry
                .tags()
                .map(|(_, name, members)| (name.clone(), members.iter().copied()));
            set = set.with_tags(Arc::new(TagTable::numbered(&table, tags)?));
        }
        grown.set = set.clone();
        Ok(set)
    }
}

struct Taken<V>(Mutex<BTreeMap<ResourceLocation, V>>);

impl<V> Taken<V> {
    fn new(values: BTreeMap<ResourceLocation, V>) -> Self {
        Self(Mutex::new(values))
    }

    fn names(&self) -> Vec<ResourceLocation> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }

    fn take(&self, name: &ResourceLocation) -> Result<V, String> {
        let mut values = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        values
            .remove(name)
            .ok_or_else(|| format!("no built-in value is named {name}"))
    }
}

type BuiltBiomes = Option<Result<BTreeMap<ResourceLocation, BiomeFile>, String>>;

struct Biomes {
    stage: StageSet,
    built: Mutex<BuiltBiomes>,
}

impl Biomes {
    fn take(&self, name: &ResourceLocation) -> Result<BiomeFile, String> {
        let mut built = self.built.lock().unwrap_or_else(PoisonError::into_inner);
        let biomes = built.get_or_insert_with(|| {
            mcrs_minecraft_worldgen_builtin::biomes(&self.stage.get()).map_err(|failures| {
                let lines: Vec<String> = failures
                    .into_iter()
                    .map(|(index, message)| format!("built-in biome {index}: {message}"))
                    .collect();
                lines.join("; ")
            })
        });
        match biomes {
            Ok(biomes) => biomes
                .remove(name)
                .ok_or_else(|| format!("no built-in biome is named {name}")),
            Err(message) => Err(message.clone()),
        }
    }
}

pub(crate) struct Planner {
    stages: BTreeMap<ResourceLocation, Stage>,
    shimmed: BTreeMap<ResourceLocation, (OriginsFn, AttachFn)>,
}

impl Planner {
    pub(crate) fn add<V: ReadValue + Send + Sync + 'static>(
        &mut self,
        declaration: DataDeclaration<RegistryOf<V>>,
    ) -> Result<Declaration, String> {
        let key = declaration.key();
        let stage = *self
            .stages
            .get(key)
            .ok_or_else(|| format!("{key} is not a data registry of the data pack report"))?;
        if stage != declaration.stage() {
            return Err(format!(
                "{key} is declared for the {:?} stage and the data pack report loads it in the {stage:?} stage",
                declaration.stage()
            ));
        }
        self.shimmed
            .insert(key.clone(), (origins::<RegistryOf<V>>, attach::<V>));
        Ok(declaration.into())
    }
}

fn with_built_ins<V: ReadValue + Send + Sync + 'static>(
    declaration: DataDeclaration<RegistryOf<V>>,
    vanilla: BTreeMap<ResourceLocation, V>,
    beta: BTreeMap<ResourceLocation, V>,
) -> DataDeclaration<RegistryOf<V>> {
    let (vanilla, beta) = (Taken::new(vanilla), Taken::new(beta));
    let (vanilla_names, beta_names) = (vanilla.names(), beta.names());
    declaration
        .built_in(VANILLA_PACK, vanilla_names, move |name, _| {
            vanilla.take(name)
        })
        .built_in(BETA_PACK, beta_names, move |name, _| beta.take(name))
}

pub(crate) fn built_biomes(
    declaration: DataDeclaration<RegistryOf<BiomeFile>>,
    stage: &StageSet,
) -> DataDeclaration<RegistryOf<BiomeFile>> {
    let biomes = Biomes {
        stage: stage.clone(),
        built: Mutex::new(None),
    };
    declaration.built_in(VANILLA_PACK, builtin::biome_names(), move |name, _| {
        biomes.take(name)
    })
}

pub(crate) fn built_template_pools(
    declaration: DataDeclaration<RegistryOf<TemplatePool>>,
    _: &StageSet,
) -> DataDeclaration<RegistryOf<TemplatePool>> {
    with_built_ins(declaration, builtin::template_pools(), BTreeMap::new())
}

pub(crate) fn built_noises(
    declaration: DataDeclaration<RegistryOf<NoiseParam>>,
    _: &StageSet,
) -> DataDeclaration<RegistryOf<NoiseParam>> {
    with_built_ins(
        declaration,
        builtin::vanilla_noises(),
        builtin::beta_noises(),
    )
}

pub(crate) fn built_density_functions(
    declaration: DataDeclaration<RegistryOf<DensityFunctionHolder>>,
    _: &StageSet,
) -> DataDeclaration<RegistryOf<DensityFunctionHolder>> {
    with_built_ins(
        declaration,
        builtin::vanilla_density_functions(),
        builtin::beta_density_functions(),
    )
}

pub(crate) fn built_noise_settings(
    declaration: DataDeclaration<RegistryOf<NoiseGeneratorSettings>>,
    _: &StageSet,
) -> DataDeclaration<RegistryOf<NoiseGeneratorSettings>> {
    with_built_ins(
        declaration,
        builtin::vanilla_noise_settings(),
        builtin::beta_noise_settings(),
    )
}

pub(crate) struct Planned {
    pub(crate) key: ResourceLocation,
    pub(crate) stage: Stage,
    pub(crate) origins: OriginsFn,
    pub(crate) attach: Option<AttachFn>,
}

pub(crate) struct Plan {
    pub(crate) declarations: Vec<Declaration>,
    pub(crate) registries: Vec<Planned>,
}

pub(crate) fn plan(
    world: &WorldRegistries,
    reloadable: &WorldRegistries,
    stage: &StageSet,
) -> Result<Plan, String> {
    let world_stage = |key: &ResourceLocation| match key.as_str() {
        DIMENSION => Stage::Dimension,
        _ => Stage::World,
    };
    let declared: Vec<(ResourceLocation, Stage, &WorldRegistries)> = world
        .declared()
        .map(|key| (key.clone(), world_stage(key), world))
        .chain(
            reloadable
                .declared()
                .map(|key| (key.clone(), Stage::Reloadable, reloadable)),
        )
        .collect();

    let mut declarations = Vec::new();
    for (at, table) in stage.get().statics().enumerate() {
        let key = table.registry().clone();
        let names = table.names().to_vec();
        let typed = TYPED_STATICS
            .iter()
            .find(|(location, _)| key.as_str() == location.as_static_str());
        let declaration = match typed {
            Some((_, declare)) => declare(key, names),
            None => fixed(at, key.clone(), names).ok_or_else(|| {
                format!("{key} is beyond the number of static registries declared")
            })?,
        };
        declarations.push(declaration);
    }

    let mut planner = Planner {
        stages: declared
            .iter()
            .map(|(key, stage, _)| (key.clone(), *stage))
            .collect(),
        shimmed: BTreeMap::new(),
    };
    declarations.extend(world_declarations(&mut planner, stage)?);
    declarations.extend(reloadable_declarations(&mut planner, stage)?);

    let mut registries = Vec::new();
    let mut unparsed = 0;
    for (key, stage, table) in declared {
        let shimmed = planner.shimmed.remove(&key);
        if shimmed.is_some() != table.parses(key.as_str()) {
            return Err(format!(
                "{key} has values in only one of the shims and the registry declarations"
            ));
        }
        let (origins, attach) = match shimmed {
            Some((origins, attach)) => (origins, Some(attach)),
            None => {
                let (declaration, origins) =
                    names_only(unparsed, key.clone(), stage).ok_or_else(|| {
                        format!("{key} is beyond the number of unparsed registries declared")
                    })?;
                unparsed += 1;
                declarations.push(declaration);
                (origins, None)
            }
        };
        registries.push(Planned {
            key,
            stage,
            origins,
            attach,
        });
    }
    Ok(Plan {
        declarations,
        registries,
    })
}
