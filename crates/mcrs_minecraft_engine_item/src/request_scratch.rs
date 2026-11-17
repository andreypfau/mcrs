use bevy_ecs::resource::Resource;

use crate::commit::Settlement;

#[derive(Resource, Default)]
pub(crate) struct RequestScratch {
    pub(crate) settlement: Settlement,
}
