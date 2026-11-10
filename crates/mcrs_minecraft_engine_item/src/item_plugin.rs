use bevy_app::App;
use bevy_app::Plugin;
use mcrs_minecraft_item_table::ItemTable;

use crate::Given;
use crate::item_components::ItemComponents;

/// Stacks as entities over the prototypes of `table`.
pub struct ItemPlugin {
    pub table: ItemTable,
}

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.table)
            .init_resource::<ItemComponents>()
            .add_message::<Given>();
    }
}
