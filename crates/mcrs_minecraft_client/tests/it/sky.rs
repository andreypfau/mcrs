use bevy::prelude::*;
use mcrs_minecraft_client::player::PlayerCamera;
use mcrs_minecraft_client::sky::{SkyEnvironment, SkyPlugin};
use mcrs_minecraft_client::sky_state::SkyFrame;
use mcrs_minecraft_client::wire_id::WireIdPlugin;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_dimension::environment::Weather;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    JoinedGame, ReceivedRegistries, ReceivedRegistry, RegistryEntry,
};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_render::sky::SkyUniform;

use crate::boot::boot;

fn sky(app: &App) -> Option<(SkyUniform, String)> {
    let environment = app.world().get_resource::<SkyEnvironment>()?;
    let uniform = environment.uniform(app.world().resource::<SkyFrame>(), 0.0);
    Some((uniform, format!("{:?}", environment.key())))
}

fn join(app: &mut App, connection: Entity, dimension: &str, dimension_type: u16) {
    *app.world_mut().get_mut::<JoinedGame>(connection).unwrap() = JoinedGame {
        player_id: 1,
        dimensions: Vec::new(),
        dimension: ResourceKey::from_location(ResourceLocation::parse(dimension).unwrap()),
        dimension_type_id: dimension_type,
    };
    app.update();
}

#[test]
fn a_dimension_named_unlike_its_type_gets_its_types_sky() {
    let mut app = boot(|app| {
        app.add_plugins((WireIdPlugin, SkyPlugin))
            .init_resource::<ClearColor>()
            .insert_resource(Weather::default());
    });

    let local: Vec<String> = app
        .world()
        .resource::<RegistrySet>()
        .registry::<keys::DimensionType>()
        .expect("the dimension type registry is loaded")
        .table()
        .names()
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut server = local.clone();
    server.reverse();
    let wire = |name: &str| server.iter().position(|n| n == name).unwrap() as u16;
    let (overworld, nether, beta) = (
        wire("minecraft:overworld"),
        wire("minecraft:the_nether"),
        wire("minecraft:beta"),
    );
    assert_ne!(
        local
            .iter()
            .position(|n| n == "minecraft:the_nether")
            .unwrap() as u16,
        nether,
        "the server numbers the types differently from the local set"
    );

    let mut received = ReceivedRegistries::default();
    received.push(ReceivedRegistry {
        registry: keys::DimensionType::KEY.as_str().to_owned(),
        entries: server
            .iter()
            .map(|id| RegistryEntry {
                id: id.clone(),
                data: None,
            })
            .collect(),
    });
    app.world_mut()
        .spawn((PlayerCamera, GlobalTransform::from_xyz(0.0, 80.0, 0.0)));
    let connection = app
        .world_mut()
        .spawn((
            ConnectionState::Game,
            received,
            JoinedGame {
                player_id: 1,
                dimensions: Vec::new(),
                dimension: keys::dimension::OVERWORLD.into(),
                dimension_type_id: nether,
            },
        ))
        .id();
    app.update();
    let overworld_named_nether_typed = sky(&app).expect("the login's type resolves to a sky");

    join(&mut app, connection, "test:elsewhere", nether);
    assert_eq!(
        sky(&app).as_ref(),
        Some(&overworld_named_nether_typed),
        "the dimension's name does not pick the sky"
    );

    join(&mut app, connection, "minecraft:overworld", overworld);
    let overworld_typed = sky(&app).expect("the overworld type has a sky");
    assert_ne!(
        overworld_typed.1, overworld_named_nether_typed.1,
        "the nether type draws no sun, moon or stars and the overworld type does"
    );
    assert_ne!(overworld_typed.0, overworld_named_nether_typed.0);

    join(&mut app, connection, "minecraft:overworld", beta);
    assert!(sky(&app).is_some(), "the beta type has a sky");

    join(
        &mut app,
        connection,
        "minecraft:overworld",
        u16::try_from(server.len()).unwrap(),
    );
    assert!(
        sky(&app).is_none(),
        "a type number the server never sent builds no sky"
    );
}
