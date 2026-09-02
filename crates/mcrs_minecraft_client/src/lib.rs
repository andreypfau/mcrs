// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

use std::path::{Path, PathBuf};

pub mod anim;
pub mod arena;
pub mod atlas;
pub mod bake;
pub mod blocks;
pub mod camera;
#[cfg(target_os = "macos")]
pub mod capture;
pub mod cave;
pub mod config;
pub mod gui;
pub mod input;
pub mod local_player;
pub mod mesh;
pub mod model;
pub mod options;
pub mod pack;
pub mod player;
pub mod probe;
pub mod readback;
pub mod render;
#[cfg(not(target_family = "wasm"))]
pub mod screenshot;
pub mod sky;
pub mod sky_render;
pub mod sky_state;
pub mod stream;
#[cfg(target_family = "wasm")]
pub mod web;

pub fn asset_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .join("assets")
}
