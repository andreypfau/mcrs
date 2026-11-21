use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use glam::DVec3;
use mcrs_minecraft_engine_mob_attributes::AttributeChanged;
use mcrs_minecraft_engine_mob_attributes::AttributeEdit;
use mcrs_minecraft_engine_mob_attributes::MobAttributesPlugin;
use mcrs_minecraft_engine_physics::MobExtras;
use mcrs_minecraft_engine_physics::MobIndex;
use mcrs_minecraft_engine_physics::MobSpawnRequest;
use mcrs_minecraft_engine_physics::SpawnSetup;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;

pub fn scene() -> TestScene {
    let mut scene = TestScene::empty(Tables::new(TestBlocks::new().table));
    scene.app.add_plugins(MobAttributesPlugin);
    let ids = EntityIdRange::starting_at(100).expect("the range fits");
    scene
        .app
        .world_mut()
        .resource_mut::<CellEntityIds>()
        .add(ids);
    scene
}

pub fn spawn_pig(scene: &mut TestScene, uuid: u128) -> Entity {
    scene.app.world_mut().write_message(MobSpawnRequest {
        kind: &EntityTypeData::PIG,
        position: DVec3::new(0.5, 64.0, 0.5),
        uuid,
        setup: SpawnSetup::None,
        extras: MobExtras::default(),
    });
    tick(scene);
    scene
        .app
        .world()
        .resource::<MobIndex>()
        .get(EntityUuid::from_u128(uuid))
        .expect("the pig is spawned")
}

pub fn edit(
    scene: &mut TestScene,
    target: Entity,
    attribute: Attribute,
    edit: InstanceEdit,
) -> Vec<AttributeChanged> {
    scene.app.world_mut().write_message(AttributeEdit {
        target,
        attribute,
        edit,
    });
    tick(scene)
}

pub fn tick(scene: &mut TestScene) -> Vec<AttributeChanged> {
    scene.step();
    scene
        .app
        .world_mut()
        .resource_mut::<Messages<AttributeChanged>>()
        .drain()
        .collect()
}
