use bevy_app::{App, FixedUpdate};
use bevy_ecs::message::Messages;
use glam::DVec3;
use mcrs_minecraft_engine_mob_attributes::{
    AttributeChanged, AttributeEdit, AttributeTarget, MobAttributesPlugin,
};
use mcrs_minecraft_engine_physics::{Mob, Mobs, mob_body, mob_row};
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::{EntityId, EntityUuid};
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_protocol_game::entity::MobLooks;

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MobAttributesPlugin::<()>::default());
    app
}

pub fn spawn_pig(app: &mut App, id: EntityId) {
    let world = app.world_mut();
    let row = mob_row(&EntityTypeData::PIG);
    let state = mob_body(
        &EntityTypeData::PIG,
        &row,
        EntityUuid::from_u128(u128::from(id.0)),
        DVec3::new(0.5, 64.0, 0.5),
    );
    world.resource_mut::<RowStore<()>>().spawn(id, state, row);
    world.resource_mut::<Mobs>().0.insert(
        id,
        Mob {
            kind: &EntityTypeData::PIG,
            uuid: u128::from(id.0),
            looks: MobLooks::default(),
            custom_name: None,
            tags: Vec::new(),
        },
    );
}

pub fn despawn_row(app: &mut App, id: EntityId) {
    let mut rows = app.world_mut().resource_mut::<RowStore<()>>();
    let tile = rows.tile_of(id).expect("the row is living");
    rows.despawn(tile, id).expect("the tile holds the row");
}

pub fn edit(
    app: &mut App,
    target: AttributeTarget,
    attribute: Attribute,
    edit: InstanceEdit,
) -> Vec<AttributeChanged> {
    app.world_mut().write_message(AttributeEdit {
        target,
        attribute,
        edit,
    });
    tick(app)
}

pub fn tick(app: &mut App) -> Vec<AttributeChanged> {
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut()
        .resource_mut::<Messages<AttributeChanged>>()
        .drain()
        .collect()
}
