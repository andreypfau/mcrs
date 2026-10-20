use bevy_app::App;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_registries::require_id;
use mcrs_minecraft_registries::{Declaration, Diagnostics, Registries, RegistryType};
use mcrs_minecraft_world::registry_load::registries_declaring;
use mcrs_minecraft_world::resolvers::{AddRegistryResolver, Resolved, run_resolvers};

struct TestBiome;

impl RegistryType for TestBiome {
    type Value = ();
}

fn biomes_named(names: &[&str]) -> Registries {
    registries_declaring(&[Declaration::fixed::<TestBiome>(
        mcrs_minecraft_biome::keys::BIOME.location().into(),
        names
            .iter()
            .map(|name| mcrs_minecraft_core::ResourceLocation::read(name).unwrap())
            .collect(),
    )])
}

struct Needs<const N: usize>(Vec<Id<Biome>>);

fn needs<const N: usize>(
    wanted: [ResourceKey<Biome, &'static str>; N],
) -> impl Fn(&Registries, &mut Diagnostics) -> Option<Needs<N>> {
    move |registries, diagnostics| {
        let biomes = registries.registry::<TestBiome>()?;
        let found: Vec<_> = wanted
            .iter()
            .map(|key| require_id(biomes, *key, diagnostics))
            .collect();
        found.into_iter().collect::<Option<Vec<_>>>().map(Needs)
    }
}

fn first_consumer(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Needs<2>> {
    needs([
        mcrs_minecraft_biome::keys::biome::ERODED_BADLANDS,
        mcrs_minecraft_biome::keys::biome::FROZEN_OCEAN,
    ])(registries, diagnostics)
}

fn second_consumer(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Needs<3>> {
    needs([
        mcrs_minecraft_biome::keys::biome::ERODED_BADLANDS,
        mcrs_minecraft_biome::keys::biome::DEEP_FROZEN_OCEAN,
        mcrs_minecraft_biome::keys::biome::PLAINS,
    ])(registries, diagnostics)
}

#[test]
fn a_biome_named_in_code_and_missing_gives_one_report_line() {
    let registries = biomes_named(&["minecraft:plains", "minecraft:frozen_ocean"]);
    let mut app = App::new();
    app.add_registry_resolver(first_consumer)
        .add_registry_resolver(|registries: &Registries, diagnostics: &mut Diagnostics| {
            needs([mcrs_minecraft_biome::keys::biome::PLAINS])(registries, diagnostics)
        });

    let report = run_resolvers(app.world_mut(), &registries).expect_err("a missing biome refuses");

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
    let registries = biomes_named(&["minecraft:plains"]);
    let text = |forward: bool| {
        let mut app = App::new();
        if forward {
            app.add_registry_resolver(first_consumer)
                .add_registry_resolver(second_consumer);
        } else {
            app.add_registry_resolver(second_consumer)
                .add_registry_resolver(first_consumer);
        }
        run_resolvers(app.world_mut(), &registries)
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
    let registries = biomes_named(&["minecraft:plains"]);
    let mut empty = App::new();
    run_resolvers(empty.world_mut(), &registries).expect("no resolver, no miss");

    let mut app = App::new();
    app.add_registry_resolver(|registries: &Registries, diagnostics: &mut Diagnostics| {
        needs([mcrs_minecraft_biome::keys::biome::PLAINS])(registries, diagnostics)
    });
    run_resolvers(app.world_mut(), &registries).expect("every name resolves");

    let plains = require_id(
        registries.registry::<TestBiome>().unwrap(),
        mcrs_minecraft_biome::keys::biome::PLAINS,
        &mut Diagnostics::default(),
    );
    assert_eq!(
        app.world().resource::<Resolved<Needs<1>>>().0,
        [plains.unwrap()]
    );
}

#[test]
fn a_resolver_that_gives_up_without_a_reason_still_refuses() {
    let registries = biomes_named(&["minecraft:plains"]);
    let mut app = App::new();
    app.add_registry_resolver(|_: &Registries, _: &mut Diagnostics| None::<Needs<1>>);

    let refused =
        run_resolvers(app.world_mut(), &registries).expect_err("an unresolved table refuses");

    let text = refused.to_string();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("Needs"), "{text}");
}
