use std::sync::Arc;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use mcrs_minecraft_client::camera::PlayerCamera;
use mcrs_minecraft_client::cave::CaveCull;
use mcrs_minecraft_client::sky::{SkyEnvironment, SkyPlugin};
use mcrs_minecraft_client::sky_state::SkyFrame;
use mcrs_minecraft_client::stream::{MeshPacing, StreamPlugin};
use mcrs_minecraft_client_world::columns::{
    Column, ColumnCachePlugin, ColumnStore, Dimension, Extent,
};
use mcrs_minecraft_client_world::joined_game::CurrentDimension;
use mcrs_minecraft_dimension_environment::environment::WeatherLevel;
use mcrs_minecraft_environment::{ClockTime, WorldClock, WorldClocks};
use mcrs_minecraft_fog::FogRamp;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_render::sky::SkyUniform;
use mcrs_minecraft_render::{Budget, RenderDistance, Uploads, WorldFog};
use mcrs_minecraft_resource::{Id, ResourceKeyBuf, ResourceLocationBuf};

use crate::boot::{boot, receive_registries, received_id};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;

fn sky(app: &App) -> Option<(SkyUniform, String)> {
    let environment = app.world().get_resource::<SkyEnvironment>()?;
    let uniform = environment.uniform(app.world().resource::<SkyFrame>(), 0.0);
    Some((uniform, format!("{:?}", environment.key())))
}

const OVERWORLD_EXTENT: Extent = Extent {
    min_section_y: -4,
    sections: 24,
};

/// Streams the columns the store holds into meshes, and steps the time of a frame by `frame`.
fn streaming(app: &mut App, frame: Duration) {
    let budget = Budget {
        quads: 1024,
        models: 1024,
        faces: 1024,
        groups: 1024,
        sections: 64,
        upload: 1 << 20,
        tint_size: [512; 2],
    };
    app.add_plugins((
        TimePlugin,
        ColumnCachePlugin::default(),
        StreamPlugin::new(
            Arc::new(budget),
            Uploads::default(),
            MeshPacing {
                per_frame: 1,
                in_flight: 1,
            },
        ),
    ))
    .insert_resource(CaveCull::new(64, false))
    .insert_resource(TimeUpdateStrategy::ManualDuration(frame));
}

fn spawn_camera(app: &mut App) {
    app.world_mut().spawn((
        PlayerCamera,
        Camera3d::default(),
        GlobalTransform::from_xyz(0.0, 80.0, 0.0),
    ));
}

/// Columns with no blocks, so each is drawn as soon as it arrives, out to `reach` columns from
/// the column of the camera.
fn hold_empty_columns(app: &mut App, reach: i32) {
    let mut store = app.world_mut().resource_mut::<ColumnStore>();
    store.enter(Dimension {
        name: "minecraft:overworld".to_owned(),
        extent: OVERWORLD_EXTENT,
        sky_light: true,
    });
    for x in -reach..=reach {
        for z in -reach..=reach {
            store.insert(
                ColumnPos::new(x, z),
                Column::unlit(
                    OVERWORLD_EXTENT.min_section_y,
                    vec![None; OVERWORLD_EXTENT.sections],
                ),
            );
        }
    }
}

fn render_fog_end(app: &App) -> f32 {
    app.world().resource::<WorldFog>().render_distance.end
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
        streaming(app, Duration::from_secs(1));
    });
    let connection = app.world_mut().spawn(ConnectionState::Configuration).id();
    receive_registries(&mut app, connection, |_| {});
    let overworld = received_id::<DimensionTypeFile>(&app, "minecraft:overworld");
    let clock = received_id::<WorldClock>(&app, "minecraft:overworld");
    spawn_camera(&mut app);
    // Every column the widest case draws, and a frame long enough for the end to come to rest.
    hold_empty_columns(&mut app, 12);
    app.update();

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
        app.update();

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

#[test]
fn the_fog_end_follows_the_drawn_rings_and_starts_over_in_a_new_dimension() {
    let mut app = boot(|app| {
        app.add_plugins(SkyPlugin)
            .init_resource::<ClearColor>()
            .init_resource::<WorldClocks>()
            .insert_resource(WeatherLevel::default())
            .insert_resource(Time::<Fixed>::from_hz(20.0))
            .insert_resource(RenderDistance(4));
        streaming(app, Duration::from_millis(50));
    });
    let connection = app.world_mut().spawn(ConnectionState::Configuration).id();
    receive_registries(&mut app, connection, |_| {});
    let overworld = received_id::<DimensionTypeFile>(&app, "minecraft:overworld");
    let nether = received_id::<DimensionTypeFile>(&app, "minecraft:the_nether");
    spawn_camera(&mut app);
    join(&mut app, connection, "minecraft:overworld", overworld);
    app.update();
    assert_eq!(render_fog_end(&app), 0.0, "no column is drawn");

    hold_empty_columns(&mut app, 1);
    app.update();
    assert!(
        (render_fog_end(&app) - 1.6).abs() < 1e-4,
        "one tick toward the one complete ring: {}",
        render_fog_end(&app)
    );
    for _ in 0..200 {
        app.update();
    }
    assert!(
        (render_fog_end(&app) - 16.0).abs() < 1e-3,
        "at rest on the complete ring: {}",
        render_fog_end(&app)
    );

    join(&mut app, connection, "minecraft:the_nether", nether);
    assert!(
        (render_fog_end(&app) - 1.6).abs() < 1e-4,
        "a new dimension follows from 0: {}",
        render_fog_end(&app)
    );
}
