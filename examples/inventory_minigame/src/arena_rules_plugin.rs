use bevy_app::App;
use bevy_app::Plugin;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::Refusal;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::RequestSource;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;

use crate::EnterGame;
use crate::Frozen;
use crate::LeaveGame;

const LOCKED_SLOT: u16 = 8;

const LOCKED: SlotAccess = SlotAccess {
    limit: 99,
    take: Gate::Refuses,
    place: Gate::Refuses,
};

const FROZEN_CODE: u16 = 1;

/// Locks the slot 8 of the player inventory from [`EnterGame`] to [`LeaveGame`], and denies each
/// request of a [`Frozen`] player with the source in [`ArenaRules`] and the code 1. Each denial of
/// that source adds one to [`Frozen::denied`] of the player.
///
/// The plugin allocates its request source in `build`. When the world has no free source, the
/// plugin adds nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaRulesPlugin;

impl Plugin for ArenaRulesPlugin {
    fn build(&self, app: &mut App) {
        let Ok(source) = app.allocate_request_source() else {
            return;
        };
        app.insert_resource(ArenaRules { source })
            .add_observer(lock_slot)
            .add_observer(unlock_slot)
            .add_observer(veto_frozen)
            .add_observer(count_denials);
    }
}

/// The request source that [`ArenaRulesPlugin`] got from the world. A reader of a
/// [`RequestResult`] compares the source of a denial with it to find the denials of the plugin.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaRules {
    /// The source of the denials of the plugin.
    pub source: RequestSource,
}

#[derive(Component)]
struct UnlockedAccess(SlotAccess);

fn lock_slot(enter: On<EnterGame>, mut players: Query<&mut SlotPolicy>, mut commands: Commands) {
    let player = enter.entity;
    let Ok(mut policy) = players.get_mut(player) else {
        return;
    };
    let unlocked = policy.access(LOCKED_SLOT);
    if unlocked == LOCKED {
        return;
    }
    policy.set(LOCKED_SLOT, LOCKED);
    commands.entity(player).insert(UnlockedAccess(unlocked));
}

fn unlock_slot(
    leave: On<LeaveGame>,
    mut players: Query<(&mut SlotPolicy, &UnlockedAccess)>,
    mut commands: Commands,
) {
    let player = leave.entity;
    let Ok((mut policy, unlocked)) = players.get_mut(player) else {
        return;
    };
    policy.set(LOCKED_SLOT, unlocked.0);
    commands.entity(player).remove::<UnlockedAccess>();
}

fn veto_frozen(
    mut check: On<RequestCheck>,
    frozen: Query<(), With<Frozen>>,
    rules: Res<ArenaRules>,
) {
    if frozen.contains(check.actor) {
        check.deny(Denial {
            source: rules.source,
            code: FROZEN_CODE,
        });
    }
}

fn count_denials(
    result: On<RequestResult>,
    rules: Res<ArenaRules>,
    mut frozen: Query<&mut Frozen>,
) {
    let Err(Refusal::Denied(denial)) = result.outcome else {
        return;
    };
    if denial.source != rules.source {
        return;
    }
    if let Ok(frozen) = frozen.get_mut(result.holder) {
        let denied = frozen.denied.saturating_add(1);
        frozen
            .map_unchanged(|frozen| &mut frozen.denied)
            .set_if_neq(denied);
    }
}
