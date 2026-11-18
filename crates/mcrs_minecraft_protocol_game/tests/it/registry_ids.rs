use mcrs_minecraft_item::data_component_predicate_type_keys::DataComponentPredicateType;
use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_particle::particle_type_keys::ParticleType;
use mcrs_minecraft_protocol::{Decode, Encode};
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_protocol_game::entity::OptionalBlockState;
use mcrs_minecraft_registries::{decode_registry_id, encode_registry_id};
use mcrs_minecraft_resource::Id;
use std::borrow::Cow;

type Decoder = fn(&mut &[u8]) -> Result<()>;

const DECODERS: [(&str, Decoder); 7] = [
    ("registry id", |r| decode_registry_id(r).map(drop)),
    ("block state", |r| BlockStateNumber::decode(r).map(drop)),
    ("item", |r| Item::decode(r).map(drop)),
    ("optional block state", |r| {
        OptionalBlockState::decode(r).map(drop)
    }),
    ("particle type", |r| ParticleType::decode(r).map(drop)),
    ("data component type", |r| {
        DataComponentType::decode(r).map(drop)
    }),
    ("component predicate type", |r| {
        DataComponentPredicateType::decode(r).map(drop)
    }),
];

fn var_int(value: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    VarInt(value).encode(&mut bytes).unwrap();
    bytes
}

#[test]
fn only_an_id_within_sixteen_bits_decodes() {
    let mut widest = Vec::new();
    encode_registry_id(u16::MAX, &mut widest).unwrap();
    assert_eq!(widest, var_int(65535));
    assert_eq!(
        decode_registry_id(&mut widest.as_slice()).unwrap(),
        u16::MAX
    );
    for (name, decode) in DECODERS {
        for raw in [-1, i32::MIN, 65536, 65537, i32::MAX] {
            let bytes = var_int(raw);
            assert!(decode(&mut bytes.as_slice()).is_err(), "{name} {raw}");
        }
    }
}

fn with_number(valid: &[u8], prefix: usize, last: u16, number: u16) -> Vec<u8> {
    let tail = prefix + var_int(i32::from(last)).len();
    [
        &valid[..prefix],
        &var_int(i32::from(number)),
        &valid[tail..],
    ]
    .concat()
}

fn refused_past_the_table(
    registry: &str,
    table_len: usize,
    prefix: usize,
    valid: &[u8],
    decode_packet: impl Fn(&mut &[u8]) -> Result<()>,
) {
    let last = table_len as u16 - 1;
    let decode = |number: u16| {
        let bytes = with_number(valid, prefix, last, number);
        decode_packet(&mut bytes.as_slice())
    };
    decode(last).unwrap_or_else(|e| panic!("{registry} {last}: {e:#}"));
    let refused = format!("{:#}", decode(last + 1).unwrap_err());
    assert!(refused.contains(registry), "{registry}: {refused}");
}

fn encoded(packet: &impl Encode) -> Vec<u8> {
    let mut bytes = Vec::new();
    packet.encode(&mut bytes).unwrap();
    bytes
}

#[test]
fn a_static_registry_number_past_its_table_does_not_decode() {
    use glam::DVec3;
    use mcrs_minecraft_block::block_entity_type_keys::BlockEntityType;
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_menu_key::menu_keys::MenuType;
    use mcrs_minecraft_protocol_game::chunk::ChunkDataBlockEntity;
    use mcrs_minecraft_protocol_game::clientbound::{ClientboundAddEntity, ClientboundOpenScreen};
    use mcrs_minecraft_protocol_game::text::{RawText, Text};

    let add_entity = ClientboundAddEntity {
        id: VarInt(1),
        uuid: uuid::Uuid::nil(),
        kind: *EntityType::ALL.last().unwrap(),
        pos: DVec3::ZERO,
        movement: mcrs_minecraft_protocol_game::LpVec3(DVec3::ZERO),
        pitch: mcrs_minecraft_protocol_game::ByteAngle(0),
        yaw: mcrs_minecraft_protocol_game::ByteAngle(0),
        head_yaw: mcrs_minecraft_protocol_game::ByteAngle(0),
        data: VarInt(0),
    };
    refused_past_the_table(
        "minecraft:entity_type",
        EntityType::ALL.len(),
        1 + 16,
        &encoded(&add_entity),
        |r| ClientboundAddEntity::decode(r).map(drop),
    );

    let last_item = Item::ALL.last().unwrap().id();
    refused_past_the_table(
        "minecraft:item",
        Item::ALL.len(),
        0,
        &encoded(&last_item),
        |r| Item::decode(r).map(drop),
    );

    let open_screen = ClientboundOpenScreen {
        container_id: VarInt(1),
        menu_type: *MenuType::ALL.last().unwrap(),
        title: RawText::from_text(&Text::text("Chest")).unwrap(),
    };
    refused_past_the_table(
        "minecraft:menu",
        MenuType::ALL.len(),
        1,
        &encoded(&open_screen),
        |r| ClientboundOpenScreen::decode(r).map(drop),
    );

    let block_entity = ChunkDataBlockEntity {
        packed_xz: 0,
        y: 0,
        kind: *BlockEntityType::ALL.last().unwrap(),
        data: Cow::Owned(Default::default()),
    };
    refused_past_the_table(
        "minecraft:block_entity_type",
        BlockEntityType::ALL.len(),
        1 + 2,
        &encoded(&block_entity),
        |r| ChunkDataBlockEntity::decode(r).map(drop),
    );
}

#[test]
fn a_world_registry_id_writes_its_number_without_a_registry() {
    use mcrs_minecraft_entity::variant::CatVariant;

    let id = Id::<CatVariant>::checked(2, 3).unwrap();
    let mut bytes = Vec::new();
    id.encode(&mut bytes).unwrap();
    assert_eq!(bytes, var_int(2));
}

#[test]
fn an_attribute_id_outside_the_registry_decodes_and_encodes_to_the_same_bytes() {
    use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
    use mcrs_minecraft_entity_attribute::{AttributeId, AttributeModifier, ModifierOperation};
    use mcrs_minecraft_protocol_game::clientbound::{
        AttributeSnapshot, ClientboundUpdateAttributes,
    };
    use mcrs_minecraft_resource::rl;

    let past_the_table = i32::try_from(Attribute::ALL.len()).unwrap();
    for raw in [past_the_table, -1, 65536, i32::MAX] {
        let packet = ClientboundUpdateAttributes {
            entity_id: VarInt(7),
            attributes: vec![
                AttributeSnapshot {
                    attribute: AttributeId(raw),
                    base: 2.0,
                    modifiers: vec![AttributeModifier {
                        id: rl!("test:unknown"),
                        amount: 0.5,
                        operation: ModifierOperation::AddValue,
                    }],
                },
                AttributeSnapshot {
                    attribute: Attribute::Armor.into(),
                    base: 3.0,
                    modifiers: vec![],
                },
            ],
        };
        let modifier_id = b"test:unknown";
        let bytes = [
            var_int(7),
            var_int(2),
            var_int(raw),
            2.0_f64.to_be_bytes().to_vec(),
            var_int(1),
            var_int(modifier_id.len() as i32),
            modifier_id.to_vec(),
            0.5_f64.to_be_bytes().to_vec(),
            var_int(0),
            var_int(i32::from(Attribute::Armor.id().number())),
            3.0_f64.to_be_bytes().to_vec(),
            var_int(0),
        ]
        .concat();
        let decoded = ClientboundUpdateAttributes::decode(&mut bytes.as_slice())
            .unwrap_or_else(|e| panic!("{raw}: {e:#}"));
        assert_eq!(decoded, packet, "{raw}");
        assert_eq!(decoded.attributes[0].attribute.resolve(), None, "{raw}");
        assert_eq!(encoded(&decoded), bytes, "{raw}");
    }
}
