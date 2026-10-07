use bevy::app::{App, Plugin};
use mcrs_minecraft_network::client::SessionRegistryInputs;
use mcrs_minecraft_world::registries::{refuse, static_registries, world_registries};

const DATAPACK_REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

pub fn session_inputs() -> SessionRegistryInputs {
    SessionRegistryInputs {
        declarations: world_registries(DATAPACK_REPORT).unwrap_or_else(|report| refuse(&report)),
        statics: static_registries().unwrap_or_else(|report| refuse(&report)),
        known: None,
    }
}

pub struct ClientRegistriesPlugin;

impl Plugin for ClientRegistriesPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(session_inputs());
    }
}
