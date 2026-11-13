use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;

use crate::InventoryClick;

/// The clicks that wait for the request of an earlier click of the same player to apply, in the
/// order of their arrival (ITX-JAV-003, ITX-MOD-005).
#[derive(Resource, Debug, Default)]
pub(crate) struct ClickBacklog(pub(crate) Vec<InventoryClick>);

pub(crate) fn any_backlog(backlog: Res<ClickBacklog>) -> bool {
    !backlog.0.is_empty()
}
