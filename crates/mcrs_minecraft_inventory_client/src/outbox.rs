use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;

use crate::outgoing::Outgoing;

#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct Outbox(pub(crate) Vec<Outgoing>);

pub(crate) fn outbox_has_packets(outbox: Res<Outbox>) -> bool {
    !outbox.0.is_empty()
}
