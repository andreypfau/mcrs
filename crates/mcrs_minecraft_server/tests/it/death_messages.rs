use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_server::{
    VanillaDeathMessagesPlugin, VanillaEntityAttributesPlugin, VanillaEntityPhysicsPlugin,
};
use mcrs_minecraft_text::Text;

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer};

#[derive(Debug, PartialEq)]
enum Sent {
    Kill(Text),
    Chat { content: Text, overlay: bool },
}

fn death_packets(packets: &[PacketPayload]) -> Vec<Sent> {
    let kill_success = Text::translate("commands.kill.success.single", vec![Text::text("dying")]);
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::PlayerCombatKill(kill) => {
                Some(Sent::Kill(kill.message.to_text().unwrap()))
            }
            PacketPayload::SystemChat(chat) => Some(Sent::Chat {
                content: chat.content.to_text().unwrap(),
                overlay: chat.overlay,
            }),
            _ => None,
        })
        .filter(|sent| !matches!(sent, Sent::Chat { content, .. } if content == &kill_success))
        .collect()
}

#[test]
fn a_killed_player_gets_the_kill_packet_then_the_death_message_when_the_rule_is_on() {
    let message = Text::translate("death.attack.genericKill", vec![Text::text("dying")]);
    for show in [true, false] {
        let mut app = host_app::make_host_app();
        app.add_plugins((
            CommandPlugin::<VanillaCommands>::new(),
            VanillaCommandsPlugin,
            VanillaEntityPhysicsPlugin,
            VanillaEntityAttributesPlugin,
            VanillaDeathMessagesPlugin,
        ));
        let mut server = PlayServer::start_in(
            app,
            &[("minecraft:overworld", "minecraft:overworld")],
            &[],
            2,
        );
        load_stone_column(&mut server);
        server
            .world()
            .resource_mut::<GameRules>()
            .set(GameRule::ShowDeathMessages, i32::from(show));
        let (dying, _) = server.join(Uuid::new_v4(), "dying");
        let (watching, _) = server.join(Uuid::new_v4(), "watching");

        server.queue(
            dying,
            &ServerboundChatCommand {
                command: Bounded("kill @s"),
            },
        );
        let packets = server.ticks_all(2);
        let to_dying = host_app::sent_to(server.session_of(dying).unwrap(), packets.clone());
        let to_watching = host_app::sent_to(server.session_of(watching).unwrap(), packets);

        let chat = || Sent::Chat {
            content: message.clone(),
            overlay: false,
        };
        let (dying_reads, watching_reads) = if show {
            (vec![Sent::Kill(message.clone()), chat()], vec![chat()])
        } else {
            (vec![Sent::Kill(Text::default())], Vec::new())
        };
        assert_eq!(
            (death_packets(&to_dying), death_packets(&to_watching)),
            (dying_reads, watching_reads),
            "show_death_messages {show}: PLD-SCR-001, PLD-DTH-001 and PLD-MSG-006"
        );
    }
}
