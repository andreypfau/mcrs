use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Local;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_mob_attributes::{AttributeTarget, RowAttributeChanged, RowAttributes};
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_replication::AttributeState;
use mcrs_minecraft_replication::MobReport;

pub(crate) fn publish_mob_attributes<M: Send + Sync + 'static>(
    mut changed: MessageReader<RowAttributeChanged>,
    attributes: Res<RowAttributes>,
    rows: Res<RowStore<M>>,
    mut events: ResMut<TileEvents>,
    mut touched: Local<Vec<(EntityId, Attribute)>>,
) {
    touched.clear();
    for change in changed.read() {
        if let AttributeTarget::Row(id) = change.target
            && change.attribute.definition().syncable
        {
            touched.push((id, change.attribute));
        }
    }
    touched.sort_unstable();
    touched.dedup();
    for &(id, attribute) in touched.iter() {
        let Some(tile) = rows.tile_of(id) else {
            continue;
        };
        let instance = attributes
            .get(id)
            .and_then(|set| set.instance(attribute))
            .cloned();
        events.push(
            tile,
            id,
            EntityEvent::Data(
                MobReport::Attribute(AttributeState {
                    attribute,
                    instance,
                })
                .into(),
            ),
        );
    }
}
