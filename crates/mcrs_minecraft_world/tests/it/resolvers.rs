use bevy_app::App;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::shared::{Resolved, SharedRegistries};
use mcrs_minecraft_registry::{Id, LoadReport, Registry, RegistrySet};
use mcrs_minecraft_world::resolvers::{AddRegistryResolver, run_resolvers};

fn biomes_named(names: &[&str]) -> RegistrySet {
    let biomes = Registry::<keys::Biome>::new(
        names
            .iter()
            .map(|name| mcrs_minecraft_core::ResourceLocation::read(name).unwrap()),
    )
    .unwrap();
    RegistrySet::new().with(biomes).unwrap()
}

struct Needs<const N: usize>(Vec<Id<keys::Biome>>);

fn needs<const N: usize>(
    wanted: [ResourceKey<keys::Biome, &'static str>; N],
) -> impl Fn(&RegistrySet, &mut LoadReport) -> Option<Resolved<Needs<N>>> {
    move |set, report| {
        let biomes = report.registry::<keys::Biome>(set)?;
        let found: Vec<_> = wanted
            .iter()
            .map(|key| report.require(&biomes, key))
            .collect();
        found
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .map(|ids| Resolved::new(Needs(ids)))
    }
}

fn first_consumer(set: &RegistrySet, report: &mut LoadReport) -> Option<Resolved<Needs<2>>> {
    needs([keys::biome::ERODED_BADLANDS, keys::biome::FROZEN_OCEAN])(set, report)
}

fn second_consumer(set: &RegistrySet, report: &mut LoadReport) -> Option<Resolved<Needs<3>>> {
    needs([
        keys::biome::ERODED_BADLANDS,
        keys::biome::DEEP_FROZEN_OCEAN,
        keys::biome::PLAINS,
    ])(set, report)
}

#[test]
fn a_biome_named_in_code_and_missing_gives_one_report_line() {
    let set = biomes_named(&["minecraft:plains", "minecraft:frozen_ocean"]);
    let mut app = App::new();
    app.add_registry_resolver(first_consumer)
        .add_registry_resolver(|set: &RegistrySet, report: &mut LoadReport| {
            needs([keys::biome::PLAINS])(set, report)
        });

    let report = run_resolvers(app.world_mut(), &set).expect_err("a missing biome refuses");

    let text = report.to_string();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("minecraft:worldgen/biome"), "{text}");
    assert!(text.contains("minecraft:eroded_badlands"), "{text}");
    assert!(
        app.world().get_resource::<Resolved<Needs<1>>>().is_none(),
        "a refused start still inserted the result of a resolver that found its names"
    );
}

#[test]
fn report_lines_do_not_depend_on_resolver_order() {
    let set = biomes_named(&["minecraft:plains"]);
    let text = |forward: bool| {
        let mut app = App::new();
        if forward {
            app.add_registry_resolver(first_consumer)
                .add_registry_resolver(second_consumer);
        } else {
            app.add_registry_resolver(second_consumer)
                .add_registry_resolver(first_consumer);
        }
        run_resolvers(app.world_mut(), &set)
            .expect_err("both consumers miss")
            .to_string()
    };

    let forward = text(true);
    assert_eq!(forward, text(false));
    let entries: Vec<_> = forward
        .lines()
        .map(|line| line.split(": ").next().unwrap())
        .collect();
    assert_eq!(
        entries,
        [
            "minecraft:worldgen/biome/minecraft:deep_frozen_ocean",
            "minecraft:worldgen/biome/minecraft:eroded_badlands",
            "minecraft:worldgen/biome/minecraft:frozen_ocean",
        ],
        "{forward}"
    );
}

#[test]
fn no_resolver_and_no_miss_lets_the_app_start() {
    let set = biomes_named(&["minecraft:plains"]);
    let mut empty = App::new();
    run_resolvers(empty.world_mut(), &set).expect("no resolver, no miss");

    let mut app = App::new();
    app.add_registry_resolver(|set: &RegistrySet, report: &mut LoadReport| {
        needs([keys::biome::PLAINS])(set, report)
    });
    run_resolvers(app.world_mut(), &set).expect("every name resolves");

    let plains = set
        .registry::<keys::Biome>()
        .unwrap()
        .require(&keys::biome::PLAINS);
    assert_eq!(
        app.world().resource::<Resolved<Needs<1>>>().0,
        [plains.unwrap()]
    );
    let shared = app.world().resource::<SharedRegistries>();
    assert_eq!(shared.shared_in(app.world(), app.world()).len(), 1);
}
