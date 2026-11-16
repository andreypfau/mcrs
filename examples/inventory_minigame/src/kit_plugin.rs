use bevy_app::App;
use bevy_app::Plugin;
use bevy_ecs::observer::On;
use bevy_ecs::query::Has;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_vanilla_inventory::PlayerInventories;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;

use crate::EnterGame;
use crate::KitSnapshot;
use crate::LeaveGame;

/// Replaces the items of a player by the kit at [`EnterGame`], and gives the earlier items back at
/// [`LeaveGame`]. The earlier items wait in a [`KitSnapshot`] on the player.
#[derive(Clone, Debug)]
pub struct KitPlugin {
    kit: PlayerItems,
}

impl KitPlugin {
    #[must_use]
    pub fn new(kit: PlayerItems) -> Self {
        Self { kit }
    }
}

impl Plugin for KitPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Kit(self.kit.clone()))
            .add_observer(hand_out_kit)
            .add_observer(give_back_items);
    }
}

#[derive(Resource)]
struct Kit(PlayerItems);

fn hand_out_kit(
    enter: On<EnterGame>,
    in_game: Query<Has<KitSnapshot>>,
    mut inventories: PlayerInventories,
    kit: Res<Kit>,
    mut commands: Commands,
) {
    let player = enter.entity;
    if in_game.get(player).unwrap_or(true) {
        return;
    }
    let Some(items) = inventories.capture(player) else {
        return;
    };
    commands
        .entity(player)
        .insert(KitSnapshot(items))
        .restore_items(kit.0.clone());
}

fn give_back_items(leave: On<LeaveGame>, snapshots: Query<&KitSnapshot>, mut commands: Commands) {
    let player = leave.entity;
    let Ok(snapshot) = snapshots.get(player) else {
        return;
    };
    commands
        .entity(player)
        .restore_items(snapshot.0.clone())
        .remove::<KitSnapshot>();
}
