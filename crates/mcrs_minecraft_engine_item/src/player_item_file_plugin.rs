use bevy_app::App;
use bevy_app::Plugin;

/// The codecs of the player file for the stacks of a player: `Inventory` into its
/// [`Slots`](crate::Slots), `SelectedItemSlot` into [`Selected`](crate::Selected) and
/// `equipment` into [`WornEquipment`](crate::WornEquipment). A plugin that owns the player
/// inventory registers its own codecs over these keys and replaces this plugin; an app adds one
/// of the two, never both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerItemFilePlugin;

impl Plugin for PlayerItemFilePlugin {
    fn build(&self, app: &mut App) {
        crate::persist::register_player(app);
    }
}
