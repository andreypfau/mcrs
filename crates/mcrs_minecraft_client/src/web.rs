use std::io::Read;
use std::path::Path;

use bevy::asset::AssetPlugin;
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::prelude::*;

pub const CANVAS: &str = "#mcrs";

const BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web_assets.bin"));

/// The browser has no asset folder to point `AssetPlugin` at, so the corpus
/// subset baked in by `build.rs` is unpacked into an in-memory tree and
/// registered as the default source before `AssetPlugin` builds.
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

pub fn asset_plugin() -> AssetPlugin {
    AssetPlugin::default()
}

pub fn window() -> Window {
    Window {
        canvas: Some(CANVAS.to_owned()),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    }
}
