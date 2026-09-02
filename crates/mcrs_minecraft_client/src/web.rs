use std::io::Read;
use std::path::Path;

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_world::environment::Weather;
use mcrs_minecraft_world::world_clock::{AdvanceTime, WorldClocks, seed_world_clocks};
use mcrs_minecraft_network::browser::target_from_query;
use mcrs_minecraft_network::client::ClientNetworkPlugin;

use bevy::camera::visibility::VisibilitySystems;
use std::sync::Arc;

use crate::render::{
    Budget, FACE_BYTES, MODEL_BYTES, QUAD_BYTES, TerrainPlugin, Uploads,
};
use crate::sky_state::SkyEffects;
use crate::{camera, cave, config, gui, input, local_player, player, render, sky, sky_render, stream};

pub const CANVAS: &str = "#mcrs";

const BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web_assets.bin"));

const SPAWN: DVec3 = DVec3::new(0.5, 80.0, 0.5);

/// The browser has no asset folder to point `AssetPlugin` at, so the corpus
/// baked in by `build.rs` is unpacked into an in-memory tree and registered as
/// the default source before `AssetPlugin` builds.
pub fn register_asset_source(app: &mut App) {
    let mut raw = Vec::new();
    flate2::read::DeflateDecoder::new(BLOB)
        .read_to_end(&mut raw)
        .expect("the baked asset blob decompresses");

    let root = Dir::default();
    let mut rest = raw.as_slice();
    let mut files = 0usize;
    while !rest.is_empty() {
        let (name, tail) = take(rest);
        let (bytes, tail) = take(tail);
        root.insert_asset(
            Path::new(std::str::from_utf8(name).expect("asset paths are utf-8")),
            bytes.to_vec(),
        );
        rest = tail;
        files += 1;
    }
    info!(files, bytes = raw.len(), "unpacked baked assets");

    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: root.clone() })),
    );
}

fn take(bytes: &[u8]) -> (&[u8], &[u8]) {
    let (len, rest) = bytes.split_at(4);
    let len = u32::from_le_bytes(len.try_into().unwrap()) as usize;
    rest.split_at(len)
}

/// `?server=` is the WebTransport URL and `?cert=` the hex SHA-256 of the
/// certificate the server minted at startup, which it prints on its own
/// listening line. There is no discovery endpoint to fetch them from.
fn server() -> Option<mcrs_minecraft_network::client::ServerAddress> {
    let (url, hash) = (query("server")?, query("cert")?);
    match target_from_query(&url, &hash) {
        Ok(target) => Some(target),
        Err(error) => {
            error!("?cert={hash}: {error}");
            None
        }
    }
}

/// The browser has no environment, so the knobs the native binary reads from
/// `MCRS_*` variables are taken from the query string instead: `?time=6000`.
pub fn query(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let value = web_sys::UrlSearchParams::new_with_str(&search)
        .ok()?
        .get(name)?;
    (!value.is_empty()).then_some(value)
}

#[derive(Resource)]
struct FrozenTicks(i64);

/// The browser entry point. There is no world folder and no save to read, so
/// the clocks come from the registry, and the world reaches the browser over
/// WebTransport rather than off disk.
pub fn run() {
    console_error_panic_hook::set_once();

    let frozen_at = query("time").and_then(|ticks| ticks.trim().parse::<i64>().ok());
    let sky_only = query("sky").and_then(|list| match SkyEffects::parse(&list) {
        Ok(effects) => Some(effects),
        Err(error) => {
            error!("?sky={list}: {error}");
            None
        }
    });

    let mut app = App::new();
    register_asset_source(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(window()),
                ..default()
            })
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(mcrs_minecraft_core::MinecraftCorePlugin)
    .add_plugins(mcrs_minecraft_world::MinecraftWorldPlugin)
    .add_plugins(player::PlayerPlugin)
    .add_plugins(input::ClientInputPlugin)
    .add_plugins(local_player::LocalPlayerPlugin)
    .add_plugins(camera::CameraPlugin)
    .add_plugins(gui::debug::DebugScreenPlugin)
    .add_plugins(gui::chunk_map::ChunkMapPlugin)
    .add_plugins(sky::SkyPlugin)
    .insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND))
    .insert_resource(WorldClocks::default())
    .insert_resource(AdvanceTime(frozen_at.is_none()))
    .insert_resource(Weather::default())
    .insert_resource(sky::PlayerDimension("minecraft:overworld".to_owned()));

    // With no save to seed them, the clocks appear only when
    // `seed_world_clocks` fills them from the registry, which is later than
    // this, so a frozen time has to be applied there instead.
    if let Some(ticks) = frozen_at {
        app.insert_resource(FrozenTicks(ticks)).add_systems(
            OnEnter(AppState::WorldgenFreeze),
            freeze_clocks.after(seed_world_clocks),
        );
    }

    if let Some(only) = sky_only {
        app.insert_resource(sky_render::SkyDrawsOnly(only));
    }

    let (budget, uploads, cave, loader) = terrain();
    app.add_plugins(TerrainPlugin(budget, uploads))
        .insert_resource(config::drawn_streams())
        .insert_resource(config::raster_fraction())
        .insert_resource(cave)
        .insert_resource(loader)
        .add_systems(Update, (stream::advance, cave::toggle, render::toggle_wireframe))
        .add_systems(
            PostUpdate,
            cave::cave_cull.after(VisibilitySystems::UpdateFrusta),
        );

    match server() {
        Some(server) => {
            app.add_plugins(ClientNetworkPlugin {
                server,
                username: query("username").unwrap_or_else(|| "Player".to_owned()),
            });
        }
        None => warn!(
            "no ?server=<https url>&cert=<sha-256 hex>: the browser draws sky only. \
             The server logs both at startup."
        ),
    }
    let (yaw, pitch) = look_override().unwrap_or((0.0, 0.0));
    player::spawn_player(app.world_mut(), SPAWN, yaw, pitch);
    app.run();
}

/// The browser draws the same columns the native client does, from a smaller
/// arena: a WebGPU context has far less room than a desktop one.
const ARENA_SCALE: usize = 1;

const GROUPS_BUDGET: usize = 1 << 20;

const SECTIONS_BUDGET: usize = 1 << 14;

const TINT_SPAN: u32 = 512;

fn terrain() -> (Arc<Budget>, Uploads, cave::CaveCull, stream::Loader) {
    let (quad_mb, model_mb, face_mb) = config::arena_budget();
    let centre = |axis: f64| (axis as i32).div_euclid(16) * 16 - TINT_SPAN as i32 / 2;
    let budget = Arc::new(Budget {
        quads: quad_mb * ARENA_SCALE * 1_000_000 / QUAD_BYTES,
        models: model_mb * ARENA_SCALE * 1_000_000 / MODEL_BYTES,
        faces: face_mb * ARENA_SCALE * 1_000_000 / FACE_BYTES,
        groups: GROUPS_BUDGET,
        sections: SECTIONS_BUDGET,
        tint_origin: [centre(SPAWN.x), centre(SPAWN.z)],
        tint_size: [TINT_SPAN; 2],
    });
    let uploads = Uploads::default();
    let loader = stream::Loader::new(&budget, uploads.clone());
    (
        budget.clone(),
        uploads,
        cave::CaveCull::new(budget.sections),
        loader,
    )
}

/// `?look=<yaw>,<pitch>` aims the camera in Minecraft degrees.
fn look_override() -> Option<(f32, f32)> {
    let look = query("look")?;
    match look
        .split_once(',')
        .and_then(|(yaw, pitch)| Some((yaw.trim().parse().ok()?, pitch.trim().parse().ok()?)))
    {
        Some(angles) => Some(angles),
        None => {
            error!("?look={look}: expected <yaw>,<pitch> in degrees");
            None
        }
    }
}

fn freeze_clocks(mut clocks: ResMut<WorldClocks>, frozen: Res<FrozenTicks>) {
    let ids: Vec<String> = clocks.iter().map(|(id, _)| id.to_string()).collect();
    for id in &ids {
        if let Some(state) = clocks.get_mut(id) {
            state.total_ticks = frozen.0;
            state.partial_tick = 0.0;
        }
    }
}

pub fn window() -> Window {
    Window {
        canvas: Some(CANVAS.to_owned()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    }
}
