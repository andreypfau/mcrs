use std::io::Read;
use std::path::Path;

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_assets::packs::PackLayers;
use mcrs_minecraft_dimension_environment::environment::Weather;
use mcrs_minecraft_environment::world_clock::{AdvanceTime, WorldClocks};
use mcrs_minecraft_network::browser::target_from_query;
use mcrs_minecraft_network::client::ClientNetworkPlugin;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::config::{Knobs, TerrainLimits};
use crate::registries::SessionSet;
use crate::{ClientPlugins, ClientTerrainPlugin, local_player, player, vanilla};

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
        AssetSourceBuilder::new(move || {
            Box::new(PackLayers::new(Box::new(MemoryAssetReader {
                root: root.clone(),
            })))
        }),
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
fn server(knobs: &Knobs) -> Option<mcrs_minecraft_network::client::ServerAddress> {
    let (url, hash) = (knobs.server()?, knobs.server_certificate()?);
    match target_from_query(url, hash) {
        Ok(target) => Some(target),
        Err(error) => {
            error!("?cert={hash}: {error}");
            None
        }
    }
}

/// The browser has no environment, so the knobs the native binary reads from
/// `MCRS_*` variables are taken from the query string instead: `?time=6000`.
fn query_knobs() -> Knobs {
    let Some(entries) = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok())
        .and_then(|params| web_sys::js_sys::try_iter(&params).ok()?)
    else {
        return Knobs::default();
    };
    let knobs = entries.filter_map(|entry| {
        let entry = web_sys::js_sys::Array::from(&entry.ok()?);
        let (name, value) = (entry.get(0).as_string()?, entry.get(1).as_string()?);
        let lowercase = !name.bytes().any(|byte| byte.is_ascii_uppercase());
        (lowercase && !value.is_empty()).then(|| (name.to_ascii_uppercase(), value))
    });
    Knobs::new(knobs, false)
}

#[derive(Resource)]
struct FrozenTicks(i64);

/// The browser entry point. There is no world folder and no save to read, so
/// the clocks come from the registry, and the world reaches the browser over
/// WebTransport rather than off disk.
pub fn run() {
    console_error_panic_hook::set_once();

    let knobs = query_knobs();
    let frozen_at = knobs.frozen_time();

    let mut app = App::new();
    app.insert_resource(knobs.clone());
    register_asset_source(&mut app);
    vanilla::register(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(window()),
                ..default()
            })
            .disable::<bevy::pbr::PbrPlugin>()
            .disable::<bevy::light::LightPlugin>(),
    )
    .add_plugins(ClientPlugins)
    .insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND))
    .insert_resource(WorldClocks::default())
    .insert_resource(AdvanceTime(frozen_at.is_none()))
    .insert_resource(Weather::default());

    // With no save to seed them, the clocks appear only when the session's
    // registries arrive, which is later than this, so a frozen time has to be
    // applied there instead.
    if let Some(ticks) = frozen_at {
        app.insert_resource(FrozenTicks(ticks)).add_systems(
            Update,
            freeze_clocks
                .after(SessionSet::Derive)
                .run_if(resource_exists_and_changed::<CurrentRegistries>),
        );
    }

    app.add_plugins(ClientTerrainPlugin(TERRAIN_LIMITS));
    #[cfg(feature = "dev")]
    app.add_plugins(crate::dev::DevPlugin);

    match server(&knobs) {
        Some(server) => {
            app.add_plugins(ClientNetworkPlugin {
                server,
                username: knobs.username(),
                profile_id: None,
                view_distance: knobs.view_distance(),
                holds_vanilla_pack: false,
            });
            app.add_plugins(crate::columns::ColumnCachePlugin);
        }
        None => warn!(
            "no ?server=<https url>&cert=<sha-256 hex>: the browser draws sky only. \
             The server logs both at startup."
        ),
    }
    let (yaw, pitch) = knobs.look_override().unwrap_or((0.0, 0.0));
    player::spawn_player(app.world_mut(), SPAWN, yaw, pitch);
    app.run();
}

/// The browser draws the same columns the native client does, from a smaller
/// arena: a WebGPU context has far less room than a desktop one.
const TERRAIN_LIMITS: TerrainLimits = TerrainLimits {
    arena_scale: 1,
    groups: 1 << 20,
    sections: 1 << 14,
    tint_span: 512,
};

fn freeze_clocks(mut clocks: ResMut<WorldClocks>, frozen: Res<FrozenTicks>) {
    for (_, state) in clocks.iter_mut() {
        state.total_ticks = frozen.0;
        state.partial_tick = 0.0;
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
