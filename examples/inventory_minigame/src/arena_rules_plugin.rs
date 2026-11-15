use bevy_app::App;
use bevy_app::Plugin;
use bevy_ecs::component::Component;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_item::RequestId;
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
/// request of a [`Frozen`] player with `denial_source` and the code 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaRulesPlugin {
    /// The source of the denials of the plugin.
    pub denial_source: RequestId,
}

impl Default for ArenaRulesPlugin {
    fn default() -> Self {
        Self {
            denial_source: RequestId(16),
        }
    }
}

impl Plugin for ArenaRulesPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DenialSource(self.denial_source))
            .add_observer(lock_slot)
            .add_observer(unlock_slot)
            .add_observer(veto_frozen);
    }
}

#[derive(Resource)]
struct DenialSource(RequestId);

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
    source: Res<DenialSource>,
) {
    if frozen.contains(check.actor) {
        check.deny(Denial {
            source: source.0,
            code: FROZEN_CODE,
        });
    }
}
