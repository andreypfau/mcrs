use glam::DVec3;
use mcrs_minecraft_client_world::columns::{Column, ColumnStore};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::EntityLight;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::chunk::LightChunk;
use mcrs_minecraft_protocol_game::clientbound::ClientboundTeleportEntity;
use mcrs_minecraft_protocol_game::light_codec::{ColumnLight, RowLight};
use mcrs_minecraft_protocol_game::{ColumnPos, PositionFlags};

use crate::{Client, add_entity};

const PIG: i32 = 1;

/// A store with one column at the origin whose section 0 has `lit` in every block.
fn store_lit_at_origin(lit: EntityLight) -> ColumnStore {
    let pos = ColumnPos::new(0, 0);
    let mut store = ColumnStore::default();
    store.insert(pos, Column::unlit(0, vec![None]));
    let row = |level: u8| {
        vec![
            RowLight::Unchanged,
            RowLight::Filled(LightChunk([level | level << 4; 2048])),
            RowLight::Unchanged,
        ]
    };
    store.relight(
        pos,
        &ColumnLight {
            sky: row(lit.sky),
            block: row(lit.block),
        },
    );
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
