use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::prelude::*;
use mcrs_minecraft_client_jar::{self as client_jar, Files, Progress};

use crate::model::is_resource;

/// The asset source holding the resource pack of the vanilla client jar.
pub const SOURCE: &str = "vanilla";

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VanillaAssets {
    #[default]
    Fetching,
    Ready,
}

#[derive(Resource)]
struct Fetch {
    unpacked: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

/// Registers the `vanilla` source and starts filling it. Must run before `AssetPlugin` builds.
pub fn register(app: &mut App) {
    let root = Dir::default();
    register_source(app, root.clone());
    let progress = Arc::<Progress>::default();
    let unpacked = Arc::<AtomicBool>::default();
    #[cfg(not(target_family = "wasm"))]
    let worker = Some(spawn(root, progress.clone(), unpacked.clone()));
    #[cfg(target_family = "wasm")]
    let worker = None;
    app.insert_resource(Fetch { unpacked, worker });
}

// A dedicated thread rather than the IO pool: a download can block for minutes, and the pool
// is shared with `Pack::load` and every other asset load.
#[cfg(not(target_family = "wasm"))]
fn spawn(root: Dir, progress: Arc<Progress>, unpacked: Arc<AtomicBool>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("client jar".to_owned())
        .spawn(move || {
            fill(&root, client_jar::resolve(&progress, is_resource));
            unpacked.store(true, Ordering::Release);
        })
        .expect("a thread for the client jar")
}

pub fn register_source(app: &mut App, root: Dir) {
    app.register_asset_source(
        AssetSourceId::from(SOURCE),
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: root.clone() })),
    );
}

// ponytail: the resource half is held twice, here and again in `Pack`; have `Pack` hold the
// `Dir`'s `Arc<Vec<u8>>` values instead if the memory matters.
pub fn fill(root: &Dir, files: Files) {
    for (path, bytes) in files {
        root.insert_asset(Path::new(&path), bytes);
    }
}

#[cfg(test)]
pub fn resource_files() -> &'static Files {
    static FILES: std::sync::LazyLock<Files> =
        std::sync::LazyLock::new(|| client_jar::resolve(&Progress::default(), is_resource));
    &FILES
}

pub struct VanillaAssetsPlugin;

impl Plugin for VanillaAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<VanillaAssets>().add_systems(
            Update,
            finish_fetch.run_if(in_state(VanillaAssets::Fetching)),
        );
    }
}

fn finish_fetch(mut fetch: ResMut<Fetch>, mut next: ResMut<NextState<VanillaAssets>>) {
    if fetch.unpacked.load(Ordering::Acquire) {
        next.set(VanillaAssets::Ready);
        return;
    }
    if fetch.worker.as_ref().is_some_and(JoinHandle::is_finished)
        && let Err(panic) = fetch.worker.take().unwrap().join()
    {
        std::panic::resume_unwind(panic);
    }
}
