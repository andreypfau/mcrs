use bevy::prelude::*;
use mcrs_minecraft_client::camera::PlayerCamera;
use mcrs_minecraft_client::sky::{SkyEnvironment, SkyPlugin};
use mcrs_minecraft_client::sky_state::SkyFrame;
use mcrs_minecraft_client_world::joined_game::CurrentDimension;
use mcrs_minecraft_dimension_environment::environment::WeatherLevel;
use mcrs_minecraft_environment::{ClockTime, WorldClock, WorldClocks};
use mcrs_minecraft_fog::FogRamp;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_render::sky::SkyUniform;
use mcrs_minecraft_render::{RenderDistance, WorldFog};
use mcrs_minecraft_resource::{Id, ResourceKeyBuf, ResourceLocationBuf};

use crate::boot::{boot, receive_registries, received_id};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;

fn sky(app: &App) -> Option<(SkyUniform, String)> {
    let environment = app.world().get_resource::<SkyEnvironment>()?;
    let uniform = environment.uniform(app.world().resource::<SkyFrame>(), 0.0);
    Some((uniform, format!("{:?}", environment.key())))
}

fn join(app: &mut App, connection: Entity, dimension: &str, dimension_type: Id<DimensionType>) {
    app.world_mut()
        .entity_mut(connection)
        .insert(CurrentDimension {
            key: ResourceKeyBuf::from_location(ResourceLocationBuf::read(dimension).unwrap()),
            dimension_type,
        });
    app.update();
}

#[test]
fn a_dimension_named_unlike_its_type_gets_its_types_sky() {
    let mut app = boot(|app| {
        app.add_plugins(SkyPlugin)
            .init_resource::<ClearColor>()
            .insert_resource(WeatherLevel::default());
    });

    let connection = app.world_mut().spawn(ConnectionState::Configuration).id();
    receive_registries(&mut app, connection, |_| {});
    let id = |name| received_id::<DimensionTypeFile>(&app, name);
    let (overworld, nether, beta) = (
        id("minecraft:overworld"),
        id("minecraft:the_nether"),
        id("minecraft:beta"),
    );

    app.world_mut()
        .spawn((PlayerCamera, GlobalTransform::from_xyz(0.0, 80.0, 0.0)));
    app.world_mut()
        .entity_mut(connection)
        .insert(CurrentDimension {
            key: mcrs_minecraft_dimension::dimension_keys::OVERWORLD.into(),
            dimension_type: nether,
        });
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

struct FogCase {
    ticks: i64,
    render_distance: u8,
    clear: [u8; 3],
    render_ramp: [f32; 2],
    sky_end: f32,
}

#[test]
fn the_fog_of_a_frame_reaches_the_clear_colour_the_world_and_the_sky() {
    let cases = [
        FogCase {
            ticks: 6000,
            render_distance: 12,
            clear: [181, 209, 255],
            render_ramp: [172.8, 192.0],
            sky_end: 192.0,
        },
        FogCase {
            ticks: 6000,
            render_distance: 5,
            clear: [176, 205, 255],
            render_ramp: [72.0, 80.0],
            sky_end: 80.0,
        },
        FogCase {
            ticks: 18000,
            render_distance: 12,
            clear: [10, 11, 19],
            render_ramp: [172.8, 192.0],
            sky_end: 192.0,
        },
    ];
    let mut app = boot(|app| {
        app.add_plugins(SkyPlugin)
            .init_resource::<ClearColor>()
            .init_resource::<WorldClocks>()
            .insert_resource(WeatherLevel::default());
    });
    let connection = app.world_mut().spawn(ConnectionState::Configuration).id();
    receive_registries(&mut app, connection, |_| {});
    let overworld = received_id::<DimensionTypeFile>(&app, "minecraft:overworld");
    let clock = received_id::<WorldClock>(&app, "minecraft:overworld");
    app.world_mut()
        .spawn((PlayerCamera, GlobalTransform::from_xyz(0.0, 80.0, 0.0)));

    for case in cases {
        let total_ticks = case.ticks;
        app.world_mut().resource_mut::<WorldClocks>().insert(
            clock,
            ClockTime {
                total_ticks,
                ..default()
            },
        );
        app.insert_resource(RenderDistance(case.render_distance));
        join(&mut app, connection, "minecraft:overworld", overworld);

        let label = format!("time {total_ticks}, r = {}", case.render_distance);
        let [red, green, blue, _] = app
            .world()
            .resource::<ClearColor>()
            .0
            .to_srgba()
            .to_u8_array();
        assert_eq!([red, green, blue], case.clear, "{label}");
        let fog = *app.world().resource::<WorldFog>();
        assert_eq!(
            [fog.color.map(|channel| (channel * 255.0).round() as u8)],
            [case.clear],
            "{label}"
        );
        assert_eq!(fog.environmental, FogRamp::NONE, "{label}");
        let render = [fog.render_distance.start, fog.render_distance.end];
        assert!(
            render
                .iter()
                .zip(case.render_ramp)
                .all(|(got, want)| (got - want).abs() < 1e-3),
            "{label}: {render:?}"
        );
        let (uniform, _) = sky(&app).expect("the overworld has a sky");
        assert_eq!(uniform.fog[..3], fog.color, "{label}");
        assert_eq!(
            [uniform.fog[3], uniform.cloud[2]],
            [case.sky_end, 2048.0],
            "{label}"
        );
    }
}
