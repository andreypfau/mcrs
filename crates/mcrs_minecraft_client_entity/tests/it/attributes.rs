use bevy_ecs::entity::Entity;
use bevy_ecs::query::{Changed, With};
use bevy_ecs::system::Query;
use glam::DVec3;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::local_player::LocalPlayerPlugin;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeId, AttributeInstances, AttributeModifier, ModifierOperation,
};
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
        attribute: attribute.into(),
        base,
        modifiers,
    }
}

fn pig() -> Client {
    let mut client = Client::new();
    client.receive(add_entity(PIG, EntityType::Pig, DVec3::ZERO, 0.0, 0.0));
    client
}

fn instances(client: &Client, entity: Entity) -> &AttributeInstances {
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
fn the_scale_of_an_entity_follows_its_entries() {
    let mut client = pig();
    client.receive(update(
        PIG,
        vec![entry(
            Attribute::Scale,
            2.0,
            vec![modifier("test:grow", 1.0, ModifierOperation::AddValue)],
        )],
    ));
    assert_eq!(client.inputs(PIG, |inputs| inputs.data.scale), 3.0);

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

fn logged_in() -> Client {
    let mut client = Client::new();
    client.app.add_plugins(LocalPlayerPlugin);
    client.app.world_mut().spawn(Player);
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
    client
}

fn player(client: &mut Client) -> Entity {
    client
        .app
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(client.app.world())
        .expect("the client has one local player")
}

#[test]
fn a_respawn_keeps_the_bases_and_keeps_the_modifiers_only_by_its_bit() {
    for (data_to_keep, luck) in [(0x00, 0.0), (0x01, 1.0), (0x02, 0.0), (0x03, 1.0)] {
        let mut client = logged_in();
        let player = player(&mut client);
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

#[test]
fn an_unknown_attribute_id_skips_only_its_entry() {
    let unknown = AttributeSnapshot {
        attribute: AttributeId(i32::try_from(Attribute::ALL.len()).expect("the table is small")),
        base: 9.0,
        modifiers: Vec::new(),
    };
    for target in ["pig", "own player"] {
        let (mut client, entity, id) = if target == "pig" {
            let client = pig();
            let entity = client.entity(PIG).expect("the pig was added");
            (client, entity, PIG)
        } else {
            let mut client = logged_in();
            let entity = player(&mut client);
            (client, entity, OWN_PLAYER)
        };
        client.receive(update(
            id,
            vec![entry(Attribute::MaxHealth, 30.0, Vec::new())],
        ));
        client.receive(update(
            id,
            vec![
                entry(Attribute::Armor, 2.0, Vec::new()),
                unknown.clone(),
                entry(
                    Attribute::MovementSpeed,
                    0.25,
                    vec![modifier("test:a", 0.5, ModifierOperation::AddValue)],
                ),
            ],
        ));
        let received = instances(&client, entity);
        assert_eq!(received.value(Attribute::Armor), Some(2.0), "{target}");
        assert_eq!(
            received.value(Attribute::MovementSpeed),
            Some(0.75),
            "{target}: the entry after the unknown id applies"
        );
        assert_eq!(
            modifier_ids(received, Attribute::MovementSpeed),
            ["test:a"],
            "{target}"
        );
        assert_eq!(
            received.value(Attribute::MaxHealth),
            Some(30.0),
            "{target}: an attribute named by no entry keeps its value"
        );
    }
}

fn changed(entities: Query<Entity, Changed<AttributeInstances>>) -> Vec<Entity> {
    entities.iter().collect()
}

#[test]
fn an_entry_that_changes_nothing_leaves_the_instances_unchanged() {
    for target in ["pig", "own player"] {
        let (mut client, entity, id) = if target == "pig" {
            let client = pig();
            let entity = client.entity(PIG).expect("the pig was added");
            (client, entity, PIG)
        } else {
            let mut client = logged_in();
            let entity = player(&mut client);
            (client, entity, OWN_PLAYER)
        };
        let armor = || update(id, vec![entry(Attribute::Armor, 2.0, Vec::new())]);
        let system = client.app.world_mut().register_system(changed);
        let run = |client: &mut Client| {
            client
                .app
                .world_mut()
                .run_system(system)
                .expect("the system is registered")
        };

        let _since_spawn = run(&mut client);
        client.receive(armor());
        assert_eq!(run(&mut client), [entity], "{target}: a new base");
        client.receive(armor());
        assert_eq!(
            run(&mut client),
            [],
            "{target}: the same entry again changes nothing"
        );
    }
}
