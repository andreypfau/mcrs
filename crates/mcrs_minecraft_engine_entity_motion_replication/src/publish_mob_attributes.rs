use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_mob_attributes::AttributeChanged;
use mcrs_minecraft_engine_mob_attributes::MobAttributes;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_replication::AttributeState;
use mcrs_minecraft_replication::MobReport;

use crate::MotionId;
use crate::PublishedTile;
use crate::subject::Subject;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Touched {
    subject: Subject,
    attribute: Attribute,
}

pub(crate) fn publish_mob_attributes(
    mut changed: MessageReader<AttributeChanged>,
    ids: Query<&MotionId>,
    mobs: Query<(&PublishedTile, &MobAttributes)>,
    mut events: ResMut<TileEvents>,
    mut touched: Local<Vec<Touched>>,
) {
    touched.clear();
    for change in changed.read() {
        if !change.attribute.definition().syncable {
            continue;
        }
        let entity = change.target;
        if let Ok(id) = ids.get(entity) {
            touched.push(Touched {
                subject: Subject { id: id.0, entity },
                attribute: change.attribute,
            });
        }
    }
    touched.sort_unstable();
    touched.dedup();
    for touched in touched.iter() {
        let subject = touched.subject;
        let Ok((published, attributes)) = mobs.get(subject.entity) else {
            continue;
        };
        let Some(tile) = published.tile() else {
            continue;
        };
        let instance = attributes.set().instance(touched.attribute).cloned();
        events.push(
            tile,
            subject.id,
            EntityEvent::Data(
                MobReport::Attribute(AttributeState {
                    attribute: touched.attribute,
                    instance,
                })
                .into(),
            ),
        );
    }
}
