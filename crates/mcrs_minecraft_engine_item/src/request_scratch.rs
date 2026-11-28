use bevy_ecs::resource::Resource;

use crate::settlement::Settlement;

#[derive(Resource, Default)]
pub(crate) struct RequestScratch {
    pub(crate) settlement: Settlement,
}
