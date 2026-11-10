use bevy_app::App;
use bevy_app::Plugin;
use mcrs_minecraft_item_table::ItemTable;

use crate::Given;
use crate::RequestResult;
use crate::item_components::ItemComponents;
use crate::request_scratch::RequestScratch;

/// Stacks as entities over the prototypes of `table`.
pub struct ItemPlugin {
    pub table: ItemTable,
}

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.table)
            .init_resource::<ItemComponents>()
            .init_resource::<RequestScratch>()
            .add_message::<Given>()
            .add_message::<RequestResult>();
    }
}
