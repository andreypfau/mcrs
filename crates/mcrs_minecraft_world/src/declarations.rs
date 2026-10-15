use std::collections::{BTreeMap, HashSet};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, PoisonError};

use mcrs_minecraft_biome::parameter_list::MultiNoiseBiomeSourceParameterList;
use mcrs_minecraft_biome_file::{BiomeFile, BiomeGenerationSettings, check_feature_domains};
use mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_enchantment::file::EnchantmentFile;
use mcrs_minecraft_entity::variant::{
    CatSoundVariant, ChickenSoundVariant, CowSoundVariant, PigSoundVariant, WolfSoundVariant,
};
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::{WorldClock, check_time_markers};
use mcrs_minecraft_item::block_transformer::BlockTransformer;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_item::decorated_pot_pattern::DecoratedPotPattern;
use mcrs_minecraft_item::dialog::Dialog;
use mcrs_minecraft_item::recipe::Recipe;
use mcrs_minecraft_item::{
    BannerPattern, InstrumentValue, JukeboxSong, PaintingVariantValue, TrimMaterial, TrimPattern,
};
use mcrs_minecraft_loot::LootTableFile;
use mcrs_minecraft_loot::condition::LootCondition;
use mcrs_minecraft_loot::function::LootItemFunction;
use mcrs_minecraft_loot::number::{FloatExpression, IntExpression};
use mcrs_minecraft_loot::slot::SlotSource;
use mcrs_minecraft_registries::{
    DataDeclaration, Declaration, LoadCtx, PackStack, ReadValue, Registries, Registry,
    RegistryType, Stage, StageView,
};
use mcrs_minecraft_registry::{
    AssemblyError, NameTable, RegistrySet, TagTable, VANILLA_PACK, WorldRegistries,
};
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature, StructureProcessorList};
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};
use mcrs_minecraft_worldgen_surface::proto::{MaterialConditionHolder, MaterialRuleHolder};
use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

use crate::chat_type::ChatType;
use crate::dimension::DimensionEntry;
use crate::enchantment_provider::EnchantmentProvider;
use crate::sulfur_cube_archetype::SulfurCubeArchetype;
use crate::test_types::{TestEnvironment, TestInstance};
use crate::variant::{
    CatVariantFile, ChickenVariantFile, CowVariantFile, FrogVariantFile, PigVariantFile,
    WolfVariantFile, ZombieNautilusVariantFile,
};
use crate::villager_trade::{TradeSet, VillagerTrade};
use crate::worldgen::world_preset::WorldPreset;

pub(crate) const BETA_PACK: &str = "file/beta";

const DIMENSION: &str = "minecraft:dimension";
const BIOME: &str = "minecraft:worldgen/biome";

mcrs_minecraft_registries::read_with_serde!(
    ChatType,
    DimensionEntry,
    EnchantmentProvider,
    SulfurCubeArchetype,
    TestEnvironment,
    TestInstance,
    CatVariantFile,
    ChickenVariantFile,
    CowVariantFile,
    FrogVariantFile,
    PigVariantFile,
    WolfVariantFile,
    ZombieNautilusVariantFile,
    TradeSet,
    VillagerTrade,
    WorldPreset,
);

pub(crate) struct RegistryOf<V>(PhantomData<fn() -> V>);

impl<V: Send + Sync + 'static> RegistryType for RegistryOf<V> {
    type Value = V;
}

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
// capped by this list; a key-addressed declaration in the loader lifts the cap.
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

fn failures<R: RegistryType>(
    registry: &Registry<R>,
    failures: Vec<(usize, String)>,
) -> Result<(), String> {
    if failures.is_empty() {
        return Ok(());
    }
    let names: Vec<&ResourceLocation> = registry.iter().map(|(_, name, _)| name).collect();
    let lines: Vec<String> = failures
        .into_iter()
        .map(|(index, message)| match names.get(index) {
            Some(name) => format!("{name}: {message}"),
            None => message,
        })
        .collect();
    Err(lines.join("; "))
}

struct Planner {
    stages: BTreeMap<ResourceLocation, Stage>,
    shimmed: BTreeMap<ResourceLocation, (OriginsFn, AttachFn)>,
}

impl Planner {
    fn data<V: ReadValue + Send + Sync + 'static>(
        &mut self,
        key: &str,
    ) -> Result<DataDeclaration<RegistryOf<V>>, String> {
        let not_declared = || format!("{key} is not a data registry of the data pack report");
        let key = ResourceLocation::read(key).map_err(|_| not_declared())?;
        let stage = *self.stages.get(&key).ok_or_else(not_declared)?;
        self.shimmed
            .insert(key.clone(), (origins::<RegistryOf<V>>, attach::<V>));
        Ok(Declaration::data(key, stage))
    }
}

fn built_in<V: ReadValue + Send + Sync + 'static>(
    planner: &mut Planner,
    key: &'static str,
    vanilla: BTreeMap<ResourceLocation, V>,
    beta: BTreeMap<ResourceLocation, V>,
) -> Result<Declaration, String> {
    let (vanilla, beta) = (Taken::new(vanilla), Taken::new(beta));
    let (vanilla_names, beta_names) = (vanilla.names(), beta.names());
    Ok(planner
        .data::<V>(key)?
        .built_in(VANILLA_PACK, vanilla_names, move |name, _| {
            vanilla.take(name)
        })
        .built_in(BETA_PACK, beta_names, move |name, _| beta.take(name))
        .into())
}

fn check_biome_features(
    registry: &Registry<RegistryOf<BiomeFile>>,
    ctx: &LoadCtx<'_>,
    stage: &StageSet,
) -> Result<(), String> {
    let placed = ctx
        .registry::<RegistryOf<PlacedFeature>>()
        .ok_or_else(|| "the placed features are not loaded".to_owned())?;
    let set = stage
        .get()
        .with_column(
            "minecraft:worldgen/placed_feature",
            Arc::clone(placed.values()),
        )
        .map_err(|error| error.to_string())?;
    let settings: Vec<BiomeGenerationSettings> = registry
        .values()
        .iter()
        .map(|biome| BiomeGenerationSettings {
            carvers: biome.carvers.clone(),
            features: biome.features.clone(),
        })
        .collect();
    failures(registry, check_feature_domains(&settings, &set))
}

fn biome_shim(planner: &mut Planner, stage: &StageSet) -> Result<Declaration, String> {
    let biomes = Biomes {
        stage: stage.clone(),
        built: Mutex::new(None),
    };
    let stage = stage.clone();
    Ok(planner
        .data::<BiomeFile>(BIOME)?
        .built_in(
            VANILLA_PACK,
            mcrs_minecraft_worldgen_builtin::biome_names(),
            move |name, _| biomes.take(name),
        )
        .validate(move |registry, ctx| check_biome_features(registry, ctx, &stage))
        .into())
}

fn timeline_shim(planner: &mut Planner, stage: &StageSet) -> Result<Declaration, String> {
    let stage = stage.clone();
    Ok(planner
        .data::<Timeline>("minecraft:timeline")?
        .validate(move |registry, _| {
            failures(
                registry,
                check_time_markers(registry.values(), &stage.get()),
            )
        })
        .into())
}

fn worldgen_shims(p: &mut Planner, stage: &StageSet) -> Result<Vec<Declaration>, String> {
    use mcrs_minecraft_worldgen_builtin as builtin;
    Ok(vec![
        biome_shim(p, stage)?,
        p.data::<CarverConfig>("minecraft:worldgen/carver")?.into(),
        p.data::<Feature>("minecraft:worldgen/feature")?.into(),
        p.data::<PlacedFeature>("minecraft:worldgen/placed_feature")?
            .into(),
        p.data::<StructureProcessorList>("minecraft:worldgen/processor_list")?
            .into(),
        built_in(
            p,
            "minecraft:worldgen/template_pool",
            builtin::template_pools(),
            BTreeMap::new(),
        )?,
        p.data::<Structure>("minecraft:worldgen/structure")?.into(),
        p.data::<StructureSet>("minecraft:worldgen/structure_set")?
            .into(),
        built_in(
            p,
            "minecraft:worldgen/noise",
            builtin::vanilla_noises(),
            builtin::beta_noises(),
        )?,
        built_in(
            p,
            "minecraft:worldgen/density_function",
            builtin::vanilla_density_functions(),
            builtin::beta_density_functions(),
        )?,
        built_in(
            p,
            "minecraft:worldgen/noise_settings",
            builtin::vanilla_noise_settings(),
            builtin::beta_noise_settings(),
        )?,
        p.data::<MaterialRuleHolder>("minecraft:worldgen/material_rule")?
            .into(),
        p.data::<MaterialConditionHolder>("minecraft:worldgen/material_condition")?
            .into(),
        p.data::<DirectBlockStateProvider>("minecraft:worldgen/block_state_provider")?
            .into(),
        p.data::<MultiNoiseBiomeSourceParameterList>(
            "minecraft:worldgen/multi_noise_biome_source_parameter_list",
        )?
        .into(),
        p.data::<WorldPreset>("minecraft:worldgen/world_preset")?
            .into(),
    ])
}

fn shims(p: &mut Planner, stage: &StageSet) -> Result<Vec<Declaration>, String> {
    let mut declarations = worldgen_shims(p, stage)?;
    declarations.extend([
        p.data::<DimensionTypeFile>("minecraft:dimension_type")?
            .into(),
        timeline_shim(p, stage)?,
        p.data::<WorldClock>("minecraft:world_clock")?.into(),
        p.data::<ChatType>("minecraft:chat_type")?.into(),
        p.data::<TrimPattern>("minecraft:trim_pattern")?.into(),
        p.data::<TrimMaterial>("minecraft:trim_material")?.into(),
        p.data::<WolfVariantFile>("minecraft:wolf_variant")?
            .non_empty()
            .into(),
        p.data::<PigVariantFile>("minecraft:pig_variant")?
            .non_empty()
            .into(),
        p.data::<FrogVariantFile>("minecraft:frog_variant")?
            .non_empty()
            .into(),
        p.data::<CatVariantFile>("minecraft:cat_variant")?
            .non_empty()
            .into(),
        p.data::<CowVariantFile>("minecraft:cow_variant")?
            .non_empty()
            .into(),
        p.data::<ChickenVariantFile>("minecraft:chicken_variant")?
            .non_empty()
            .into(),
        p.data::<ZombieNautilusVariantFile>("minecraft:zombie_nautilus_variant")?
            .non_empty()
            .into(),
        p.data::<WolfSoundVariant>("minecraft:wolf_sound_variant")?
            .non_empty()
            .into(),
        p.data::<PigSoundVariant>("minecraft:pig_sound_variant")?
            .non_empty()
            .into(),
        p.data::<CatSoundVariant>("minecraft:cat_sound_variant")?
            .non_empty()
            .into(),
        p.data::<CowSoundVariant>("minecraft:cow_sound_variant")?
            .non_empty()
            .into(),
        p.data::<ChickenSoundVariant>("minecraft:chicken_sound_variant")?
            .non_empty()
            .into(),
        p.data::<PaintingVariantValue>("minecraft:painting_variant")?
            .non_empty()
            .into(),
        p.data::<SulfurCubeArchetype>("minecraft:sulfur_cube_archetype")?
            .into(),
        p.data::<DamageType>("minecraft:damage_type")?.into(),
        p.data::<BannerPattern>("minecraft:banner_pattern")?.into(),
        p.data::<EnchantmentFile>("minecraft:enchantment")?.into(),
        p.data::<JukeboxSong>("minecraft:jukebox_song")?.into(),
        p.data::<InstrumentValue>("minecraft:instrument")?.into(),
        p.data::<TestEnvironment>("minecraft:test_environment")?
            .into(),
        p.data::<TestInstance>("minecraft:test_instance")?.into(),
        p.data::<Dialog>("minecraft:dialog")?.into(),
        p.data::<DecoratedPotPattern>("minecraft:decorated_pot_pattern")?
            .into(),
        p.data::<BlockTransformer>("minecraft:block_transformer")?
            .into(),
        p.data::<EnchantmentProvider>("minecraft:enchantment_provider")?
            .into(),
        p.data::<VillagerTrade>("minecraft:villager_trade")?.into(),
        p.data::<TradeSet>("minecraft:trade_set")?.into(),
        p.data::<DimensionEntry>(DIMENSION)?.into(),
        p.data::<Recipe>("minecraft:recipe")?.into(),
        p.data::<LootCondition>("minecraft:predicate")?.into(),
        p.data::<LootItemFunction>("minecraft:item_modifier")?
            .into(),
        p.data::<SlotSource>("minecraft:slot_source")?.into(),
        p.data::<LootTableFile>("minecraft:loot_table")?.into(),
        p.data::<IntExpression>("minecraft:context_int_provider")?
            .into(),
        p.data::<FloatExpression>("minecraft:context_float_provider")?
            .into(),
    ]);
    Ok(declarations)
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
        let declaration = fixed(at, key.clone(), table.names().to_vec())
            .ok_or_else(|| format!("{key} is beyond the number of static registries declared"))?;
        declarations.push(declaration);
    }

    let mut planner = Planner {
        stages: declared
            .iter()
            .map(|(key, stage, _)| (key.clone(), *stage))
            .collect(),
        shimmed: BTreeMap::new(),
    };
    declarations.extend(shims(&mut planner, stage)?);

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
