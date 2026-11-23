use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;

use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_client::test_support;
use mcrs_minecraft_client_control::ControlPlugin;
use mcrs_minecraft_client_control::Reply;
use mcrs_minecraft_client_control::Target;
use mcrs_minecraft_client_entity::EntityDamaged;
use mcrs_minecraft_client_world::block_breaking::BlockBreakingPlugin;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_client_world::local_motion::LocalMotion;
use mcrs_minecraft_client_world::local_player::LocalPlayerPlugin;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_engine_world::world::storage::column::ColumnPos;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::ClientEntity;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;

use crate::local_world::LocalWorld;

struct Controller {
    stream: UnixStream,
    received: Vec<u8>,
}

const MAX_TICKS: usize = 2000;

impl Controller {
    fn connect(address: &str) -> Self {
        let stream = UnixStream::connect(address).unwrap();
        stream.set_nonblocking(true).unwrap();
        Self {
            stream,
            received: Vec::new(),
        }
    }

    fn ask(&mut self, app: &mut App, line: &str) -> Reply {
        writeln!(self.stream, "{line}").unwrap();
        self.reply(app, line)
    }

    fn reply(&mut self, app: &mut App, line: &str) -> Reply {
        for _ in 0..MAX_TICKS {
            if let Some(end) = self.received.iter().position(|byte| *byte == b'\n') {
                let reply: Vec<u8> = self.received.drain(..=end).collect();
                return serde_json::from_slice(&reply).unwrap();
            }
            app.update();
            app.world_mut().run_schedule(FixedUpdate);
            let mut chunk = [0; 4096];
            if let Ok(read) = self.stream.read(&mut chunk) {
                self.received.extend_from_slice(chunk.get(..read).unwrap());
            }
        }
        panic!("no reply to {line:?} within {MAX_TICKS} ticks");
    }
}

fn pig(world: &mut World) -> Option<(Entity, i32)> {
    world
        .query::<(Entity, &ClientEntity)>()
        .iter(world)
        .find(|(_, client)| client.kind == EntityType::Pig)
        .map(|(entity, client)| (entity, client.network_id))
}

#[derive(Resource, Default)]
struct Damaged(Vec<Entity>);

fn record_damage(mut damaged: MessageReader<EntityDamaged>, mut record: ResMut<Damaged>) {
    record
        .0
        .extend(damaged.read().map(|damaged| damaged.entity));
}

fn fly(world: &mut World) {
    for mut motion in world
        .query_filtered::<&mut LocalMotion, With<Player>>()
        .iter_mut(world)
    {
        motion.0.flight0 = true;
        motion.0.body.velocity = DVec3::ZERO;
    }
}

#[test]
fn a_look_and_an_attack_through_the_control_socket_hit_the_pig_on_the_server() {
    let address = std::env::temp_dir().join(format!("mcrs-control-{}.sock", std::process::id()));
    let address = address.display().to_string();
    let plugin_address = address.clone();
    let mut world = LocalWorld::join(
        "control-attack",
        |_| {},
        move |app| {
            app.add_plugins((
                LocalPlayerPlugin,
                BlockBreakingPlugin,
                ControlPlugin::bind(&plugin_address).unwrap(),
            ))
            .init_resource::<Damaged>()
            .add_systems(Update, record_damage);
        },
    );
    let mut closing = Controller::connect(&address);
    closing.stream.write_all(b"ready\n").unwrap();
    closing.stream.shutdown(std::net::Shutdown::Write).unwrap();
    assert!(
        matches!(closing.reply(&mut world.app, "ready"), Reply::Ready(_)),
        "a request and the end of the stream in one read get their reply"
    );
    let mut control = Controller::connect(&address);
    test_support::connection(world.app.world_mut()).write_packet(&ServerboundChatCommand {
        command: Bounded("summon minecraft:pig 0.5 0 2.5 {NoAI:1b}"),
    });
    let mut target = None;
    test_support::update_until(&mut world.app, |app| {
        target = pig(app.world_mut());
        let column_held = app
            .world()
            .resource::<ColumnStore>()
            .holds(ColumnPos::from(DVec3::ZERO));
        target.is_some() && column_held
    });
    let (pig, network_id) = target.unwrap();
    let Reply::Entities(nearby) = control.ask(&mut world.app, "entities 8") else {
        panic!("entities answers with the list");
    };
    assert!(
        nearby
            .iter()
            .any(|entity| entity.id == network_id && entity.kind == "minecraft:pig"),
        "{nearby:?}"
    );
    assert!(
        matches!(
            control.ask(&mut world.app, "look_entity -5"),
            Reply::Error(_)
        ),
        "an unknown network id is an error"
    );
    let is_pig =
        |target: &Target| matches!(target, Target::Entity(entity) if entity.id == network_id);
    for _ in 0..100 {
        fly(world.app.world_mut());
        assert_eq!(
            control.ask(&mut world.app, &format!("look_entity {network_id}")),
            Reply::Done
        );
        let Reply::Target(aimed) = control.ask(&mut world.app, "target") else {
            panic!("target answers with the pick");
        };
        let Reply::Attacked(attacked) = control.ask(&mut world.app, "attack") else {
            panic!("attack answers with what it attacked");
        };
        assert_eq!(is_pig(&aimed), is_pig(&attacked), "{aimed:?} {attacked:?}");
        if world.app.world().resource::<Damaged>().0.contains(&pig) {
            assert!(is_pig(&attacked), "{attacked:?}");
            break;
        }
    }
    assert!(
        world.app.world().resource::<Damaged>().0.contains(&pig),
        "the pig takes damage within 100 attacks"
    );
    drop(control);
    world.leave();
    assert!(
        std::fs::metadata(&address).is_err(),
        "the plugin removes its socket when the app is dropped"
    );
}
