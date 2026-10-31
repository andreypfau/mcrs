use mcrs_minecraft_datapack::Level;
use mcrs_minecraft_registries::Diagnostics;

/// Logs each diagnostic of a registry load at its level.
pub(crate) fn log_diagnostics(diagnostics: &Diagnostics) {
    for diagnostic in diagnostics.iter() {
        match diagnostic.level() {
            Level::Info => tracing::info!("{diagnostic}"),
            Level::Warning => tracing::warn!("{diagnostic}"),
            Level::Error => tracing::error!("{diagnostic}"),
        }
    }
}
