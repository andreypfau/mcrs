use std::path::PathBuf;

use bevy_ecs::entity::Entity;
use mcrs_minecraft_command_engine::{CommandLimits, CommandPlugin};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::banlist::{BanEntry, BanList, BanListHostPlugin};
use mcrs_minecraft_engine::bus::{PacketPayload, PacketTarget};
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_item::ItemHoverMap;
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol_game::text::{Color, IntoText, Text};
use mcrs_minecraft_server::operator_broadcast::OperatorBroadcastPlugin;

use crate::host_app::{self, PlayServer};

const ADMIN: Uuid = Uuid::from_u128(1);
const GAMEMASTER: Uuid = Uuid::from_u128(2);

fn start(bans: BanList) -> PlayServer {
    let mut app = host_app::make_host_app();
    app.insert_resource(CommandLimits(Default::default()));
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_dimension_plugin(OperatorBroadcastPlugin);
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(BanListHostPlugin);
    app.insert_resource(bans);
    PlayServer::start_in(
        app,
        &[("minecraft:overworld", "minecraft:overworld")],
        &[(ADMIN, 3), (GAMEMASTER, 2)],
        0,
    )
}

fn replies(server: &mut PlayServer, anchor: Entity, line: &str) -> Vec<Text> {
    let packets = server.send(
        anchor,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
    chats(&packets)
}

fn chats(packets: &[PacketPayload]) -> Vec<Text> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::SystemChat(chat) => {
                Some(chat.content.to_text().unwrap().map(&ItemHoverMap).unwrap())
            }
            _ => None,
        })
        .collect()
}

fn chats_to(server: &mut PlayServer, anchor: Entity) -> Vec<Text> {
    let session = server.session_of(anchor).unwrap();
    let packets: Vec<PacketPayload> = server
        .ticks_all(2)
        .into_iter()
        .filter(|packet| matches!(&packet.target, PacketTarget::PlayerSet(set) if set.contains(&session)))
        .map(|packet| packet.data)
        .collect();
    chats(&packets)
}

fn temp_file(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mcrs-ban-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(test);
    let _ = std::fs::remove_file(&path);
    path
}

fn reply(key: &'static str) -> Text {
    Text::translate(key, Vec::<Text>::new())
}

fn failed(key: &'static str) -> Text {
    reply(key).color(Color::RED)
}

fn default_reason() -> Text {
    reply("multiplayer.disconnect.banned.reason.default")
}

fn banned(name: &str, reason: Text) -> Text {
    Text::translate(
        "commands.ban.success",
        vec![name.to_owned().into_text(), reason],
    )
}

fn unbanned(name: &str) -> Text {
    Text::translate("commands.pardon.success", vec![name.to_owned().into_text()])
}

fn line(target: &str, source: &str, reason: Text) -> Text {
    Text::translate(
        "commands.banlist.entry",
        vec![
            target.to_owned().into_text(),
            source.to_owned().into_text(),
            reason,
        ],
    )
}

fn entry(name: &str) -> BanEntry {
    BanEntry::new(
        offline_player_uuid(name),
        name.to_owned(),
        "tester".to_owned(),
        None,
    )
}

#[test]
fn ban_cmd_001_ban_replies_with_the_default_reason_and_then_that_nothing_changed() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "ban blonline"),
        [banned("blonline", default_reason())]
    );

    server.ticks(admin, 1);
    assert_eq!(
        replies(&mut server, admin, "ban blonline"),
        [failed("commands.ban.failed")]
    );
}

#[test]
fn ban_cmd_001_ban_with_a_reason_takes_every_word_of_it() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "ban blreason griefing the spawn"),
        [banned(
            "blreason",
            "griefing the spawn".to_owned().into_text()
        )]
    );

    server.ticks(admin, 1);
    let bans = server.app.world().resource::<BanList>();
    let entry = bans.get(&offline_player_uuid("blreason")).unwrap();
    assert_eq!(entry.reason.as_deref(), Some("griefing the spawn"));
}

#[test]
fn ban_cmd_001_pardon_replies_for_each_state() {
    let mut server = start(BanList::new([entry("blreason")]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "pardon blreason"),
        [unbanned("blreason")]
    );

    server.ticks(admin, 1);
    assert_eq!(
        replies(&mut server, admin, "pardon blreason"),
        [failed("commands.pardon.failed")]
    );
}

#[test]
fn pardon_finds_a_ban_by_its_name_in_any_case_and_replies_with_that_name() {
    let id = Uuid::from_u128(9);
    let mut server = start(BanList::new([BanEntry::new(
        id,
        "Other".to_owned(),
        "tester".to_owned(),
        None,
    )]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "pardon oTHER"),
        [unbanned("Other")]
    );

    server.ticks(admin, 1);
    assert!(server.app.world().resource::<BanList>().get(&id).is_none());
}

#[test]
fn ban_cmd_001_banlist_of_no_bans() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "banlist"),
        [reply("commands.banlist.none")]
    );
}

#[test]
fn ban_cmd_001_banlist_counts_the_bans_and_names_the_source_and_the_reason() {
    let mut with_reason = entry("blreason");
    with_reason.reason = Some("griefing".to_owned());
    let mut server = start(BanList::new([with_reason]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "banlist"),
        [
            Text::translate_args("commands.banlist.list", vec![1.into()]),
            line("blreason", "tester", "griefing".to_owned().into_text()),
        ]
    );
}

#[test]
fn ban_cmd_001_a_ban_without_a_reason_lists_the_default_text() {
    let mut server = start(BanList::new([entry("blminimal")]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "banlist")[1],
        line("blminimal", "tester", default_reason())
    );
}

#[test]
fn ban_cmd_002_banlist_players_shows_the_player_bans() {
    let mut server = start(BanList::new([entry("blminimal")]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "banlist players"),
        replies(&mut server, admin, "banlist")
    );
}

#[test]
fn ban_cmd_003_ban_writes_the_file_with_the_name_of_the_player_as_the_source() {
    let path = temp_file("ban");
    let mut server = start(BanList::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");

    replies(&mut server, admin, "ban blonline griefing");
    server.ticks(admin, 1);

    let read = BanList::read(&path);
    let entry = read.get(&offline_player_uuid("blonline")).unwrap();
    assert_eq!(entry.name, "blonline");
    assert_eq!(entry.source, "Admin");
    assert_eq!(entry.reason.as_deref(), Some("griefing"));
}

#[test]
fn ban_cmd_003_pardon_writes_the_file() {
    let path = temp_file("pardon");
    let id = offline_player_uuid("blonline");
    std::fs::write(
        &path,
        format!(r#"[{{"uuid": "{id}", "name": "blonline"}}]"#),
    )
    .unwrap();
    let mut server = start(BanList::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");

    replies(&mut server, admin, "pardon blonline");
    server.ticks(admin, 1);

    assert_eq!(BanList::read(&path).entries().count(), 0);
}

#[test]
fn ban_cmd_003_a_command_that_changes_nothing_does_not_write_the_file() {
    let path = temp_file("nothing");
    std::fs::write(&path, "[]").unwrap();
    let mut server = start(BanList::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");
    std::fs::write(&path, "kept").unwrap();

    replies(&mut server, admin, "pardon blonline");
    server.ticks(admin, 1);

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept");
}

#[test]
fn ban_cmd_006_a_name_that_is_not_online_keeps_its_case_and_takes_the_id_of_that_name() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "ban BlCaseOne"),
        [banned("BlCaseOne", default_reason())]
    );

    server.ticks(admin, 1);
    let bans = server.app.world().resource::<BanList>();
    assert!(bans.get(&offline_player_uuid("BlCaseOne")).is_some());
    assert!(bans.get(&offline_player_uuid("blcaseone")).is_none());
}

#[test]
fn ban_cmd_006_each_case_is_its_own_ban_and_pardon_takes_the_exact_case() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");
    replies(&mut server, admin, "ban BlCaseTwo");
    server.ticks(admin, 1);

    assert_eq!(
        replies(&mut server, admin, "ban blcasetwo"),
        [banned("blcasetwo", default_reason())]
    );
    server.ticks(admin, 1);

    assert_eq!(
        replies(&mut server, admin, "pardon blcasetwo"),
        [unbanned("blcasetwo")]
    );
    server.ticks(admin, 1);
    let bans = server.app.world().resource::<BanList>();
    assert!(bans.get(&offline_player_uuid("BlCaseTwo")).is_some());
    assert!(bans.get(&offline_player_uuid("blcasetwo")).is_none());
}

fn expired_ban_file(test: &str, name: &str) -> PathBuf {
    let path = temp_file(test);
    let id = offline_player_uuid(name);
    std::fs::write(
        &path,
        format!(
            r#"[{{"uuid": "{id}", "name": "{name}", "expires": "2021-01-01 00:00:00 +0000", "reason": "old"}}]"#
        ),
    )
    .unwrap();
    path
}

#[test]
fn ban_exp_003_ban_replaces_an_expired_ban_in_the_list_and_the_file() {
    let path = expired_ban_file("exp_003", "blexpired");
    let mut server = start(BanList::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "ban blexpired fresh"),
        [banned("blexpired", "fresh".to_owned().into_text())]
    );

    server.ticks(admin, 1);
    let id = offline_player_uuid("blexpired");
    let bans = server.app.world().resource::<BanList>();
    assert_eq!(bans.active(&id).unwrap().reason.as_deref(), Some("fresh"));
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(file.contains(r#""reason": "fresh""#), "{file}");
    assert!(file.contains(r#""expires": "forever""#), "{file}");
    assert!(!file.contains("2021"), "{file}");
}

#[test]
fn ban_exp_004_pardon_removes_an_expired_ban_with_the_usual_reply() {
    let path = expired_ban_file("exp_004", "blexpired");
    let mut server = start(BanList::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "pardon blexpired"),
        [unbanned("blexpired")]
    );

    server.ticks(admin, 1);
    assert_eq!(
        server.app.world().resource::<BanList>().entries().count(),
        0
    );
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("blexpired")
    );
}

#[test]
fn ban_of_an_online_player_takes_its_profile_in_any_case() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");
    let (_watcher, _) = server.join(GAMEMASTER, "Watcher");

    assert_eq!(
        replies(&mut server, admin, "ban wATCHER"),
        [banned("Watcher", default_reason())]
    );

    server.ticks(admin, 1);
    let bans = server.app.world().resource::<BanList>();
    assert_eq!(bans.get(&GAMEMASTER).unwrap().name, "Watcher");
}

#[test]
fn a_ban_reaches_the_copy_of_the_dimension_and_shows_in_the_list() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    replies(&mut server, admin, "ban blonline");
    server.ticks(admin, 1);

    assert_eq!(
        replies(&mut server, admin, "banlist")[1],
        line("blonline", "Admin", default_reason())
    );
}

#[test]
fn each_command_requires_the_level_of_admins() {
    let mut server = start(BanList::new([entry("blbanned")]));
    let (gamemaster, _) = server.join(GAMEMASTER, "Gamemaster");

    for command in [
        "ban blother",
        "pardon blbanned",
        "banlist",
        "banlist players",
    ] {
        let unknown = replies(&mut server, gamemaster, command);
        assert_eq!(
            unknown.first(),
            Some(&failed("command.unknown.command")),
            "{command}: {unknown:?}"
        );
    }

    server.ticks(gamemaster, 1);
    let bans = server.app.world().resource::<BanList>();
    assert_eq!(bans.entries().count(), 1);
    assert!(bans.get(&offline_player_uuid("blbanned")).is_some());
}

#[test]
fn a_ban_reaches_the_other_operators_and_a_failure_does_not() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");
    let (watcher, _) = server.join(GAMEMASTER, "Watcher");
    let command = ServerboundChatCommand {
        command: Bounded("ban blonline"),
    };

    server.queue(admin, &command);
    assert_eq!(
        chats_to(&mut server, watcher),
        [Text::translate(
            "chat.type.admin",
            vec!["Admin".into_text(), banned("blonline", default_reason())],
        )
        .color(Color::GRAY)
        .italic()]
    );

    server.queue(admin, &command);
    assert_eq!(chats_to(&mut server, watcher), []);
}

fn suggest(server: &mut PlayServer, anchor: Entity, text: &str) -> Vec<String> {
    let packets = server.send(
        anchor,
        &ServerboundCommandSuggestion {
            id: VarInt(3),
            text: Bounded(text),
        },
    );
    let mut names: Vec<String> = packets
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::CommandSuggestions(reply) => Some(
                reply
                    .suggestions
                    .iter()
                    .map(|suggestion| suggestion.text.to_string())
                    .collect(),
            ),
            _ => None,
        })
        .expect("every request gets a reply");
    names.sort();
    names
}

#[test]
fn ban_suggests_the_players_that_are_online() {
    let mut server = start(BanList::new([entry("blbanned")]));
    let (admin, _) = server.join(ADMIN, "Admin");
    server.join(GAMEMASTER, "Watcher");

    assert_eq!(suggest(&mut server, admin, "/ban "), ["Admin", "Watcher"]);
    assert_eq!(suggest(&mut server, admin, "/ban wa"), ["Watcher"]);
    assert!(suggest(&mut server, admin, "/ban bl").is_empty());
}

#[test]
fn pardon_suggests_the_banned_players_and_not_the_players_that_are_online() {
    let mut server = start(BanList::new([entry("blbanned"), entry("blother")]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        suggest(&mut server, admin, "/pardon "),
        ["blbanned", "blother"]
    );
    assert_eq!(suggest(&mut server, admin, "/pardon BLB"), ["blbanned"]);
    assert!(suggest(&mut server, admin, "/pardon Ad").is_empty());
}

#[test]
fn pardon_stops_suggesting_a_player_after_the_pardon() {
    let mut server = start(BanList::new([entry("blbanned")]));
    let (admin, _) = server.join(ADMIN, "Admin");

    replies(&mut server, admin, "pardon blbanned");
    server.ticks(admin, 1);

    assert!(suggest(&mut server, admin, "/pardon ").is_empty());
}

#[test]
fn a_ban_without_a_name_is_not_suggested() {
    let mut server = start(BanList::new([BanEntry::new(
        Uuid::from_u128(9),
        String::new(),
        "tester".to_owned(),
        None,
    )]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert!(suggest(&mut server, admin, "/pardon ").is_empty());
}

#[test]
fn a_pardon_in_the_pass_of_the_ban_reads_that_ban() {
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");
    server.ticks(admin, 1);

    for line in ["ban Wa", "pardon Wa"] {
        server.queue(
            admin,
            &ServerboundChatCommand {
                command: Bounded(line),
            },
        );
    }
    server.ticks(admin, 2);

    assert_eq!(
        server.app.world().resource::<BanList>().entries().count(),
        0
    );
}

#[test]
fn ban_of_an_online_name_takes_the_player_with_the_exact_case_first() {
    let (upper, lower) = (Uuid::from_u128(11), Uuid::from_u128(12));
    let mut server = start(BanList::default());
    let (admin, _) = server.join(ADMIN, "Admin");
    server.join(upper, "Wa");
    server.join(lower, "wa");

    replies(&mut server, admin, "ban wa");
    server.ticks(admin, 1);

    let bans = server.app.world().resource::<BanList>();
    assert!(bans.get(&lower).is_some());
    assert!(bans.get(&upper).is_none());
}
