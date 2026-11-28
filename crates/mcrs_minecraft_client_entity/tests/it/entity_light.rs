use glam::DVec3;
use mcrs_minecraft_client_world::columns::{Column, ColumnStore};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::{EntityLight, HostLight};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::chunk::LightChunk;
use mcrs_minecraft_protocol_game::clientbound::ClientboundTeleportEntity;
use mcrs_minecraft_protocol_game::light_codec::{ColumnLight, RowLight};
use mcrs_minecraft_protocol_game::{ColumnPos, PositionFlags};

use crate::{Client, add_entity};

const PIG: i32 = 1;

fn unlit_store_at_origin() -> ColumnStore {
    let mut store = ColumnStore::default();
    store.insert(ColumnPos::new(0, 0), Column::unlit(0, vec![None]));
    store
}

fn light_origin(store: &mut ColumnStore, lit: EntityLight) {
    let row = |level: u8| {
        vec![
            RowLight::Unchanged,
            RowLight::Filled(LightChunk([level | level << 4; 2048])),
            RowLight::Unchanged,
        ]
    };
    store.relight(
        ColumnPos::new(0, 0),
        &ColumnLight {
            sky: row(lit.sky),
            block: row(lit.block),
        },
    );
}

fn store_lit_at_origin(lit: EntityLight) -> ColumnStore {
    let mut store = unlit_store_at_origin();
    light_origin(&mut store, lit);
    store
}

#[test]
fn an_entity_that_snaps_into_a_lit_block_takes_its_light() {
    let lit = EntityLight { block: 5, sky: 3 };
    let mut client = Client::new();
    client.app.insert_resource(store_lit_at_origin(lit));
    client.receive(add_entity(
        PIG,
        EntityType::Pig,
        DVec3::new(200.5, 4.0, 8.5),
        0.0,
        0.0,
    ));
    client.app.update();
    let pig = client.entity(PIG).expect("the pig was added");
    assert_eq!(
        client.app.world().get::<EntityLight>(pig),
        Some(&EntityLight::OPEN_SKY),
        "no column holds the eye of the pig"
    );

    client.receive(ClientboundTeleportEntity {
        entity_id: VarInt(PIG),
        position: DVec3::new(8.5, 4.0, 8.5),
        velocity: DVec3::ZERO,
        look: Look::default(),
        relatives: PositionFlags::default(),
        on_ground: true,
    });
    client.app.update();

    assert_eq!(client.app.world().get::<EntityLight>(pig), Some(&lit));
}

#[test]
fn a_standing_entity_takes_the_light_that_arrives_for_its_column() {
    let lit = EntityLight { block: 5, sky: 3 };
    let mut client = Client::new();
    let mut changes = Vec::new();
    let mut store = unlit_store_at_origin();
    store.drain_changes(&mut changes);
    client.app.insert_resource(store);
    client.receive(add_entity(
        PIG,
        EntityType::Pig,
        DVec3::new(8.5, 4.0, 8.5),
        0.0,
        0.0,
    ));
    client.app.update();
    let pig = client.entity(PIG).expect("the pig was added");
    assert_eq!(
        client.app.world().get::<EntityLight>(pig),
        Some(&EntityLight { block: 0, sky: 0 }),
        "the column of the pig holds no light yet"
    );

    changes.clear();
    let mut store = client.app.world_mut().resource_mut::<ColumnStore>();
    light_origin(&mut store, lit);
    store.drain_changes(&mut changes);
    client.app.world_mut().write_message_batch(changes);
    client.app.update();

    assert_eq!(client.app.world().get::<EntityLight>(pig), Some(&lit));
}

#[test]
fn an_entity_with_host_light_keeps_the_light_of_its_host() {
    let host = EntityLight { block: 15, sky: 15 };
    let mut client = Client::new();
    client
        .app
        .insert_resource(store_lit_at_origin(EntityLight { block: 5, sky: 3 }));
    client.receive(add_entity(
        PIG,
        EntityType::Pig,
        DVec3::new(200.5, 4.0, 8.5),
        0.0,
        0.0,
    ));
    client.app.update();
    let pig = client.entity(PIG).expect("the pig was added");
    client
        .app
        .world_mut()
        .entity_mut(pig)
        .insert((HostLight, host));

    client.receive(ClientboundTeleportEntity {
        entity_id: VarInt(PIG),
        position: DVec3::new(8.5, 4.0, 8.5),
        velocity: DVec3::ZERO,
        look: Look::default(),
        relatives: PositionFlags::default(),
        on_ground: true,
    });
    client.app.update();

    assert_eq!(client.app.world().get::<EntityLight>(pig), Some(&host));
}
