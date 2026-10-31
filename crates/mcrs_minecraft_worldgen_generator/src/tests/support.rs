use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks, load_block_definitions};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_voxel::BlockStateId;

/// The corpus, loaded once per process. Worldgen resolves every block it
/// places against it, so a stub would fail at the first lookup.
pub fn corpus() -> &'static BlockDefinitions {
    &blocks().0
}

/// The corpus state a worldgen test voxel names.
pub fn corpus_state_of(voxel: VoxelId) -> BlockStateId {
    BlockStateId::checked(voxel.0, corpus().state_count()).expect("a voxel the corpus holds")
}

/// The corpus as the generation systems take it, sharing the one load above.
pub fn blocks() -> &'static Blocks {
    static CORPUS: OnceLock<Blocks> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        Blocks(std::sync::Arc::new(
            load_block_definitions(&asset_server)
                .expect("the corpus loads")
                .0,
        ))
    })
}

use mcrs_minecraft_worldgen_testing::registry;

use mcrs_minecraft_worldgen_density::compile::build_router;
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter, RouterBlocks};

pub fn router_blocks(blocks: &BlockDefinitions) -> RouterBlocks {
    RouterBlocks {
        default_block: blocks
            .default_state("minecraft:stone")
            .unwrap()
            .number()
            .into(),
        default_fluid: blocks
            .default_state("minecraft:water")
            .unwrap()
            .number()
            .into(),
        water: blocks
            .default_state("minecraft:water")
            .unwrap()
            .number()
            .into(),
        lava: blocks
            .default_state("minecraft:lava")
            .unwrap()
            .number()
            .into(),
    }
}

pub fn build_settings_router(settings_name: &str, seed: u64) -> NoiseRouter {
    let settings: NoiseGeneratorSettings = mcrs_minecraft_worldgen_testing::read(
        "noise_settings",
        &ResourceLocationBuf::minecraft(settings_name).unwrap(),
    );
    build_router(
        &settings,
        &registry("density_function"),
        &registry("noise"),
        seed,
        router_blocks(corpus()),
    )
    .unwrap_or_else(|e| panic!("{settings_name}: {e}"))
}

pub fn build_beta_router() -> NoiseRouter {
    build_settings_router("beta", 12345)
}

/// The router of the noise settings `settings_name` with its material rules over the biome
/// numbers of `registries`, every block state the rules name taken as its block's default.
pub fn material_router(
    settings_name: &str,
    seed: u64,
    registries: &Registries,
) -> (
    NoiseRouter,
    mcrs_minecraft_worldgen_surface::compile::MaterialProgram,
) {
    use mcrs_minecraft_worldgen_surface::MaterialInputs;
    use mcrs_minecraft_worldgen_surface::compile::build_router_and_material;

    let settings: NoiseGeneratorSettings = mcrs_minecraft_worldgen_testing::read(
        "noise_settings",
        &ResourceLocationBuf::minecraft(settings_name).unwrap(),
    );
    let biomes = material_biomes(registries);
    let inputs = MaterialInputs {
        rules: &registry("material_rule"),
        conditions: &registry("material_condition"),
        block: &|state| {
            corpus()
                .block(state.name.as_str())
                .map(|block| block.default_state_id.number().into())
        },
        biomes: &biomes,
    };
    build_router_and_material(
        &settings,
        &registry("density_function"),
        &registry("noise"),
        seed,
        router_blocks(corpus()),
        &inputs,
    )
    .unwrap_or_else(|e| panic!("{settings_name}: {e}"))
}

/// A block state or biome name the registries do not resolve fails the whole
/// compile, and the router the app then never inserts is what gates column
/// generation, so a corpus rename would otherwise cost every chunk silently.
#[test]
fn every_shipped_noise_settings_compiles_its_material_rules() {
    use std::collections::BTreeMap;

    use mcrs_minecraft_worldgen_surface::compile::build_router_and_material;
    use mcrs_minecraft_worldgen_surface::{
        MaterialConditionHolder, MaterialInputs, MaterialRuleHolder,
    };

    use crate::block_state::try_resolve_state;

    let rules: BTreeMap<ResourceLocationBuf, MaterialRuleHolder> = registry("material_rule");
    let conditions: BTreeMap<ResourceLocationBuf, MaterialConditionHolder> =
        registry("material_condition");
    let biomes = material_biomes(test_registries());
    let functions = registry("density_function");
    let noises = registry("noise");

    let mut seen = 0;
    for (id, settings) in registry::<NoiseGeneratorSettings>("noise_settings") {
        let name = id.path();
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &|state| try_resolve_state(corpus(), state).map(|state| state.number().into()),
            biomes: &biomes,
        };
        build_router_and_material(
            &settings,
            &functions,
            &noises,
            42,
            router_blocks(corpus()),
            &inputs,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        seen += 1;
    }
    assert!(seen >= 8, "only {seen} noise settings were checked");
}

use std::sync::Arc;

use mcrs_minecraft_biome::{Biome, TemperatureModifier};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_registries::{IdSet, KeySet, Registries, Registry, Resolve, Resolver, Tags};
use mcrs_minecraft_resource::{Id, ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_worldgen_feature_place::terrain_skin::BiomeClimate;

pub type Biomes = Registry<BiomeFile>;

pub fn test_registries() -> &'static Registries {
    mcrs_minecraft_world::registry_load::test_registries()
}

pub fn biomes_of(registries: &Registries) -> &Biomes {
    registries
        .registry::<BiomeFile>()
        .expect("the registries hold the biomes")
}

pub fn biome_id(registries: &Registries, name: &str) -> Id<Biome> {
    let key = ResourceKeyBuf::from_location(
        ResourceLocationBuf::read(name).unwrap_or_else(|e| panic!("{name}: {e}")),
    );
    Resolver::id(registries, &key).unwrap_or_else(|e| panic!("{e}"))
}

/// What the material rules take a `biome_is` set to: the biome numbers of `registries`.
pub fn material_biomes(
    registries: &Registries,
) -> impl Fn(&KeySet<Biome>) -> Option<Vec<u16>> + '_ {
    let tags = registries.tags::<Biome>();
    move |set: &KeySet<Biome>| {
        let set = IdSet::<Biome>::resolve(set.clone(), registries).ok()?;
        Some(set.ids(tags.as_ref()?).map(|id| id.number()).collect())
    }
}

pub fn block_tags() -> &'static Tags<Block> {
    static TAGS: OnceLock<Tags<Block>> = OnceLock::new();
    TAGS.get_or_init(|| {
        test_registries()
            .tags::<Block>()
            .expect("the loaded registries hold the block tags")
    })
}

pub fn corpus_biomes() -> &'static Biomes {
    biomes_of(test_registries())
}

/// Every corpus biome's climate, indexed like [`corpus_biomes`].
pub fn corpus_climate() -> &'static Arc<[BiomeClimate]> {
    static CLIMATE: OnceLock<Arc<[BiomeClimate]>> = OnceLock::new();
    CLIMATE.get_or_init(|| {
        corpus_biomes()
            .values()
            .iter()
            .map(|biome| BiomeClimate {
                base_temperature: biome.temperature,
                frozen: biome.temperature_modifier == Some(TemperatureModifier::Frozen),
                has_precipitation: biome.has_precipitation,
            })
            .collect()
    })
}

/// The carver list a biome actually ships, as the loader reads it.
pub fn carvers_of(
    biome: mcrs_minecraft_resource::ResourceLocation<'_>,
) -> Arc<[mcrs_minecraft_worldgen_carver::config::CarverConfig]> {
    let id = biome_id(test_registries(), &biome.to_string());
    carver_list(id.index())
}

pub fn carver_list(biome: usize) -> Arc<[mcrs_minecraft_worldgen_carver::config::CarverConfig]> {
    let registries = test_registries();
    let configs = registries
        .registry::<mcrs_minecraft_worldgen_carver::config::CarverConfig>()
        .expect("the loaded registries hold the carvers");
    let tags = registries
        .tags::<mcrs_minecraft_biome_file::Carvers>()
        .expect("the loaded registries hold the carver tags");
    corpus_biomes().values()[biome]
        .carvers
        .ids(&tags)
        .map(|id| configs.get(id).expect("a loaded carver").clone())
        .collect()
}

pub fn biome_tags() -> Tags<Biome> {
    test_registries()
        .tags::<Biome>()
        .expect("the loaded registries hold the biome tags")
}

/// The loaded registries as a client holds them after a server sent its biomes with `leading`
/// first, in that order, and every other biome after them in the loaded order. A leading name the
/// loaded registries lack is a biome with the data of `minecraft:plains`.
pub fn registries_over(leading: &[&str]) -> Registries {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use mcrs_minecraft_registries::Received;
    use mcrs_minecraft_world::registry_load::{load_received, sent_by};

    const REPORT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/reports/datapack.json"
    ));
    static SENT: OnceLock<Received> = OnceLock::new();
    static OVER: OnceLock<Mutex<HashMap<Vec<String>, Registries>>> = OnceLock::new();

    let key: Vec<String> = leading.iter().map(|name| (*name).to_owned()).collect();
    let mut over = OVER
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(registries) = over.get(&key) {
        return registries.clone();
    }
    let mut received = SENT
        .get_or_init(|| sent_by(test_registries(), false))
        .clone();
    let biome_key = mcrs_minecraft_biome::keys::BIOME.location().to_buf();
    let (_, entries) = received
        .registries
        .iter_mut()
        .find(|(registry, _)| *registry == biome_key)
        .expect("the server sends its biomes");
    let stand_in = entries
        .iter()
        .find(|(name, _)| name.as_str() == "minecraft:plains")
        .map(|(_, data)| data.clone())
        .expect("the server sends minecraft:plains");
    let loaded = entries.len();
    for name in leading {
        if !entries.iter().any(|(entry, _)| entry.as_str() == *name) {
            let name = ResourceLocationBuf::read(name).unwrap_or_else(|e| panic!("{name}: {e}"));
            entries.push((name, stand_in.clone()));
        }
    }
    let rank = |name: &ResourceLocationBuf| {
        leading
            .iter()
            .position(|leading| *leading == name.as_str())
            .unwrap_or(leading.len())
    };
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by_key(|&index| rank(&entries[index].0));
    let mut renumbered = vec![0; loaded];
    for (new, &old) in order.iter().enumerate() {
        if let Some(number) = renumbered.get_mut(old) {
            *number = new as i32;
        }
    }
    *entries = order.iter().map(|&index| entries[index].clone()).collect();
    for (_, biome_tags) in received
        .tags
        .iter_mut()
        .filter(|(registry, _)| *registry == biome_key)
    {
        for (_, members) in biome_tags {
            for member in members {
                *member = renumbered[*member as usize];
            }
        }
    }
    let (loaded, _) = load_received(REPORT, &received, None)
        .unwrap_or_else(|error| panic!("the renumbered biomes do not load: {error}"));
    over.insert(key, loaded.clone());
    loaded
}
