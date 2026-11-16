use bevy_ecs::resource::Resource;

use crate::commit::Settlement;

/// The settlement buffers of the commit of a request, used again by each request.
#[derive(Resource, Default)]
pub(crate) struct RequestScratch {
    pub(crate) settlement: Settlement,
}
