use bevy::prelude::*;
use mcrs_minecraft_client::player::PlayerCamera;
use mcrs_minecraft_client::sky::{SkyEnvironment, SkyPlugin};
use mcrs_minecraft_client::sky_state::SkyFrame;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_dimension_environment::environment::Weather;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::CurrentDimension;
use mcrs_minecraft_registry::Id;
use mcrs_minecraft_render::sky::SkyUniform;

use crate::boot::{boot, session_registries};
use mcrs_minecraft_dimension::DimensionType;

fn sky(app: &App) -> Option<(SkyUniform, String)> {
    let environment = app.world().get_resource::<SkyEnvironment>()?;
    let uniform = environment.uniform(app.world().resource::<SkyFrame>(), 0.0);
    Some((uniform, format!("{:?}", environment.key())))
}

fn join(app: &mut App, connection: Entity, dimension: &str, dimension_type: Id<DimensionType>) {
    app.world_mut()
        .entity_mut(connection)
        .insert(CurrentDimension {
            key: ResourceKey::from_location(ResourceLocation::read(dimension).unwrap()),
            dimension_type,
        });
    app.update();
}

#[test]
fn a_dimension_named_unlike_its_type_gets_its_types_sky() {
    let mut app = boot(|app| {
        app.add_plugins(SkyPlugin)
            .init_resource::<ClearColor>()
            .insert_resource(Weather::default());
    });

    let session = session_registries(&app, |_| {});
    let types = session
        .registry::<DimensionType>()
        .expect("the dimension type registry is received");
    let (overworld, nether, beta) = (
        types.require_by_name("minecraft:overworld").unwrap(),
        types.require_by_name("minecraft:the_nether").unwrap(),
        types.require_by_name("minecraft:beta").unwrap(),
    );

    app.insert_resource(session);
    app.world_mut()
        .spawn((PlayerCamera, GlobalTransform::from_xyz(0.0, 80.0, 0.0)));
    let connection = app
        .world_mut()
        .spawn((
            ConnectionState::Game,
            CurrentDimension {
                key: mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
                dimension_type: nether,
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
}
