use bevy_state::prelude::States;

/// Top-level application lifecycle states.
///
/// The state machine progresses linearly during startup:
///
/// ```text
/// Bootstrap → LoadingDataPack → WorldgenFreeze → Playing
///                                                    ↕
///                                             Reconfiguring
/// ```
///
/// - **Bootstrap**: Static registries (blocks, items) are populated.
/// - **LoadingDataPack**: Bevy asset loaders are running; worldgen JSON assets
///   are being loaded (biomes, density functions, noise settings, etc.).
/// - **WorldgenFreeze**: All worldgen assets are ready.  Tags are resolved,
///   `RegistrySnapshot` is assigned stable network IDs, `NoiseRouter` is
///   compiled.  No further data-pack changes until next reconfiguration.
/// - **Playing**: Normal server operation.
/// - **Reconfiguring**: Mid-session reconfiguration (client reconnect flow).
///   After resolution returns to `Playing`.
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Bootstrap,
    LoadingDataPack,
    WorldgenFreeze,
    Playing,
    Reconfiguring,
}
