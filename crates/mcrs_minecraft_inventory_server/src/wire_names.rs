use bevy_ecs::system::Res;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::CurrentRegistries;

/// The item table and the registries that the stack codecs of the wire read.
#[derive(SystemParam)]
pub(crate) struct WireNames<'w> {
    table: Res<'w, ItemTable>,
    registries: Res<'w, CurrentRegistries>,
}

impl WireNames<'_> {
    pub(crate) fn names(&self) -> RegistryNames<'_> {
        RegistryNames::new(&self.registries, &self.table)
    }
}
