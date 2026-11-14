use glam::DVec3;
use mcrs_minecraft_client_entity::AttributeInstances;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::{AttributeModifier, ModifierOperation};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{
    AttributeSnapshot, ClientboundLogin, ClientboundRespawn, ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
use mcrs_minecraft_resource::{Id, ResourceLocation};

use crate::{Client, add_entity};

const PIG: i32 = 5;
const OWN_PLAYER: i32 = 7;

fn modifier(
    id: &'static str,
    amount: f64,
    operation: ModifierOperation,
) -> AttributeModifier<'static> {
    AttributeModifier {
        id: ResourceLocation::new_static(id).expect("the test identifier is valid"),
        amount,
        operation,
    }
}

fn update(
    entity_id: i32,
    attributes: Vec<AttributeSnapshot<'static>>,
) -> ClientboundUpdateAttributes<'static> {
    ClientboundUpdateAttributes {
        entity_id: VarInt(entity_id),
        attributes,
    }
}

fn entry(
    attribute: Attribute,
    base: f64,
    modifiers: Vec<AttributeModifier<'static>>,
) -> AttributeSnapshot<'static> {
    AttributeSnapshot {
        attribute,
        base,
        modifiers,
    }
}

fn pig() -> Client {
    let mut client = Client::new();
    client.receive(add_entity(PIG, EntityType::Pig, DVec3::ZERO, 0.0, 0.0));
    client
}

fn instances(client: &Client, entity: bevy_ecs::entity::Entity) -> &AttributeInstances {
    client
        .app
        .world()
        .get::<AttributeInstances>(entity)
        .expect("a living entity has its attribute instances")
}

fn modifier_ids(instances: &AttributeInstances, attribute: Attribute) -> Vec<String> {
    instances
        .set()
        .instance(attribute)
        .map(|instance| {
            instance
                .modifiers()
                .iter()
                .map(|modifier| modifier.id().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn an_entry_replaces_the_base_and_the_modifiers_in_its_order() {
    let mut client = pig();
    let entity = client.entity(PIG).expect("the pig was added");
    client.receive(update(
        PIG,
        vec![entry(
            Attribute::MovementSpeed,
            0.5,
            vec![
                modifier("test:b", 1.0, ModifierOperation::AddMultipliedTotal),
                modifier("test:a", 0.25, ModifierOperation::AddValue),
                modifier("test:b", 100.0, ModifierOperation::AddValue),
            ],
        )],
    ));
    let speed = instances(&client, entity);
    assert_eq!(speed.value(Attribute::MovementSpeed), Some(1.5));
    assert_eq!(
        modifier_ids(speed, Attribute::MovementSpeed),
        ["test:b", "test:a"],
        "the modifiers keep the order of the entry, and a second test:b is ignored"
    );

    client.receive(update(
        PIG,
        vec![entry(
            Attribute::MovementSpeed,
            0.25,
            vec![modifier("test:a", 0.5, ModifierOperation::AddValue)],
        )],
    ));
    let speed = instances(&client, entity);
    assert_eq!(speed.value(Attribute::MovementSpeed), Some(0.75));
    assert_eq!(modifier_ids(speed, Attribute::MovementSpeed), ["test:a"]);

    client.receive(update(PIG, vec![entry(Attribute::Scale, 1.0, Vec::new())]));
    assert_eq!(
        instances(&client, entity).value(Attribute::MovementSpeed),
        Some(0.75),
        "an instance with no entry does not change"
    );
}

#[test]
fn the_scale_of_an_entity_follows_its_entries() {
    let mut client = pig();
    client.receive(update(
        PIG,
        vec![entry(
            Attribute::Scale,
            2.0,
            vec![
                modifier("test:grow", 1.0, ModifierOperation::AddValue),
                modifier("test:grow", 4.0, ModifierOperation::AddValue),
            ],
        )],
    ));
    assert_eq!(
        client.inputs(PIG, |inputs| inputs.data.scale),
        3.0,
        "the scale counts only the first modifier of an identifier"
    );

    client.receive(update(PIG, vec![entry(Attribute::Scale, 0.5, Vec::new())]));
    assert_eq!(client.inputs(PIG, |inputs| inputs.data.scale), 0.5);
}

fn spawn_info() -> PlayerSpawnInfo {
    PlayerSpawnInfo::new(
        Id::checked(0, 1).expect("the dimension type id is in range"),
        OVERWORLD.into(),
        GameMode::Survival,
    )
}

fn logged_in() -> (Client, bevy_ecs::entity::Entity) {
    let mut client = Client::new();
    let player = client.app.world_mut().spawn(Player).id();
    client.join(JoinedGame {
        player_id: OWN_PLAYER,
        dimensions: Vec::new(),
    });
    client.receive(ClientboundLogin {
        player_id: OWN_PLAYER,
        hardcore: false,
        dimensions: Vec::new(),
        max_players: VarInt(20),
        chunk_radius: VarInt(8),
        simulation_distance: VarInt(8),
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        player_spawn_info: spawn_info(),
        online_mode: false,
        enforces_secure_chat: false,
    });
    (client, player)
}

#[test]
fn a_respawn_keeps_the_bases_and_keeps_the_modifiers_only_by_its_bit() {
    for (data_to_keep, luck) in [(0x00, 0.0), (0x01, 1.0), (0x02, 0.0), (0x03, 1.0)] {
        let (mut client, player) = logged_in();
        assert_eq!(
            instances(&client, player).value(Attribute::MovementSpeed),
            Some(f64::from(0.1_f32)),
            "the login gives the bases of the type"
        );
        client.receive(update(
            OWN_PLAYER,
            vec![
                entry(Attribute::MaxHealth, 30.0, Vec::new()),
                entry(
                    Attribute::Luck,
                    0.0,
                    vec![modifier("test:a", 1.0, ModifierOperation::AddValue)],
                ),
            ],
        ));
        client.receive(ClientboundRespawn {
            player_spawn_info: spawn_info(),
            data_to_keep,
        });
        let respawned = instances(&client, player);
        assert_eq!(
            respawned.value(Attribute::MaxHealth),
            Some(30.0),
            "bits {data_to_keep:#x}"
        );
        assert_eq!(
            respawned.value(Attribute::Luck),
            Some(luck),
            "bits {data_to_keep:#x}"
        );
    }
}
