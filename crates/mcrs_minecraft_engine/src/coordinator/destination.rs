use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeId;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::coordinator::dim::DimSubAppHandle;
use crate::departure::Departure;
use crate::to_muxer::{Destination, RoutedDeparture};

/// The running dimension of `key`.
pub fn find(host: &mut World, key: &ResourceKeyBuf<DimensionKey>) -> Option<Destination> {
    host.query_filtered::<(Entity, &ResourceKeyBuf<DimensionKey>, &DimensionTypeId), With<DimSubAppHandle>>()
        .iter(host)
        .find(|(_, found, _)| *found == key)
        .map(|(label, key, dimension_type)| Destination {
            label,
            key: key.clone(),
            dimension_type: dimension_type.0,
        })
}

/// Picks the dimension that `departure` from the dimension `source` goes to: the running
/// dimension it names, unless that is `source`.
pub fn route(host: &mut World, source: Entity, departure: Departure) -> RoutedDeparture {
    let destination = find(host, &departure.to).filter(|found| found.label != source);
    RoutedDeparture {
        departure,
        destination,
    }
}
