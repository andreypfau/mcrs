use bevy_asset::AssetServer;
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_minecraft_assets::{AppState, LoadedRegistryAssets};

pub(crate) fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

pub(crate) fn check_registry_assets_ready(
    registry_assets: Res<LoadedRegistryAssets>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if registry_assets.all_handles_settled(&asset_server) {
        tracing::info!("all registry assets settled — entering WorldgenFreeze");
        next.set(AppState::WorldgenFreeze);
    }
}
