use std::path::PathBuf;

use bevy_ecs::entity::Entity;
use mcrs_minecraft_command_engine::{CommandLimits, CommandPlugin};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::bus::{PacketPayload, PacketTarget};
use mcrs_minecraft_engine::whitelist::{Whitelist, WhitelistEntry, WhitelistHostPlugin};
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_item::ItemHoverMap;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::text::{Color, IntoText, Text};
use mcrs_minecraft_server::operator_broadcast::OperatorBroadcastPlugin;

use crate::host_app::{self, PlayServer};

const ADMIN: Uuid = Uuid::from_u128(1);
const GAMEMASTER: Uuid = Uuid::from_u128(2);

fn start(whitelist: Whitelist) -> PlayServer {
    let mut app = host_app::make_host_app();
    app.insert_resource(CommandLimits(Default::default()));
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_dimension_plugin(OperatorBroadcastPlugin);
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(WhitelistHostPlugin);
    app.insert_resource(whitelist);
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

fn none() -> Text {
    Text::translate("commands.whitelist.none", Vec::<Text>::new())
}

#[test]
fn wl_cmd_001_list_of_an_empty_whitelist() {
    let mut server = start(Whitelist::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(replies(&mut server, admin, "whitelist list"), [none()]);
}

#[test]
fn wl_cmd_001_list_names_each_entry_of_the_host_with_their_count() {
    let mut server = start(Whitelist::new([WhitelistEntry {
        uuid: Uuid::from_u128(3),
        name: "wa".to_owned(),
    }]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "whitelist list"),
        [Text::translate_args(
            "commands.whitelist.list",
            vec![1.into(), "wa".to_owned().into()],
        )]
    );
}

#[test]
fn the_command_requires_the_level_of_admins() {
    let mut server = start(Whitelist::default());
    let (gamemaster, _) = server.join(GAMEMASTER, "Gamemaster");

    assert_ne!(replies(&mut server, gamemaster, "whitelist list"), [none()]);
}

fn reply(key: &'static str) -> Text {
    Text::translate(key, Vec::<Text>::new())
}

fn enabled(server: &PlayServer) -> bool {
    server.app.world().resource::<Whitelist>().is_enabled()
}

#[test]
fn wl_cfg_002_on_and_off_change_the_check_and_reply_for_each_state() {
    let mut server = start(Whitelist::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "whitelist off"),
        [failed("commands.whitelist.alreadyOff")]
    );
    assert_eq!(
        replies(&mut server, admin, "whitelist on"),
        [reply("commands.whitelist.enabled")]
    );
    server.ticks(admin, 1);
    assert!(enabled(&server));
    assert_eq!(
        replies(&mut server, admin, "whitelist on"),
        [failed("commands.whitelist.alreadyOn")]
    );
    assert_eq!(
        replies(&mut server, admin, "whitelist off"),
        [reply("commands.whitelist.disabled")]
    );
    server.ticks(admin, 1);
    assert!(!enabled(&server));
}

#[test]
fn wl_dev_003_on_does_not_write_the_file() {
    let path = temp_file("on");
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");
    std::fs::write(&path, "kept").unwrap();

    replies(&mut server, admin, "whitelist on");
    server.ticks(admin, 1);

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept");
}

#[test]
fn wl_file_002_reload_reads_the_file_that_changed() {
    let id = WhitelistEntry::offline("wn").uuid;
    let path = temp_file("reload");
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");
    std::fs::write(&path, format!(r#"[{{"uuid": "{id}", "name": "wn"}}]"#)).unwrap();
    server.ticks(admin, 1);
    assert!(!server.app.world().resource::<Whitelist>().contains(&id));

    assert_eq!(
        replies(&mut server, admin, "whitelist reload"),
        [reply("commands.whitelist.reloaded")]
    );

    server.ticks(admin, 1);
    assert!(server.app.world().resource::<Whitelist>().contains(&id));
    assert_eq!(
        replies(&mut server, admin, "whitelist list"),
        [Text::translate_args(
            "commands.whitelist.list",
            vec![1.into(), "wn".to_owned().into()],
        )]
    );
}

#[test]
fn wl_file_005_reload_without_a_file_keeps_the_whitelist_and_creates_no_file() {
    let id = WhitelistEntry::offline("wn").uuid;
    let path = temp_file("reload_missing");
    std::fs::write(&path, format!(r#"[{{"uuid": "{id}", "name": "wn"}}]"#)).unwrap();
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");
    std::fs::remove_file(&path).unwrap();

    assert_eq!(
        replies(&mut server, admin, "whitelist reload"),
        [reply("commands.whitelist.reloaded")]
    );

    server.ticks(admin, 1);
    assert!(server.app.world().resource::<Whitelist>().contains(&id));
    assert!(!path.exists());
}

#[test]
fn wl_cmd_005_a_success_reaches_the_other_operators_and_a_failure_does_not() {
    let mut server = start(Whitelist::default());
    let (admin, _) = server.join(ADMIN, "Admin");
    let (watcher, _) = server.join(GAMEMASTER, "Watcher");
    let line = ServerboundChatCommand {
        command: Bounded("whitelist on"),
    };

    server.queue(admin, &line);
    assert_eq!(
        chats_to(&mut server, watcher),
        [Text::translate(
            "chat.type.admin",
            vec!["Admin".into_text(), reply("commands.whitelist.enabled")],
        )
        .color(Color::GRAY)
        .italic()]
    );

    server.queue(admin, &line);
    assert_eq!(chats_to(&mut server, watcher), []);
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
    let dir = std::env::temp_dir().join(format!("mcrs-whitelist-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(test);
    let _ = std::fs::remove_file(&path);
    path
}

fn success(key: &'static str, name: &str) -> Text {
    Text::translate(key, vec![name.to_owned().into_text()])
}

fn failed(key: &'static str) -> Text {
    Text::translate(key, Vec::<Text>::new()).color(Color::RED)
}

#[test]
fn wl_cmd_002_add_writes_the_file_and_reaches_the_copy_of_the_dimension() {
    let path = temp_file("add");
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "whitelist add Wa"),
        [success("commands.whitelist.add.success", "Wa")]
    );

    server.ticks(admin, 1);
    let id = WhitelistEntry::offline("Wa").uuid;
    assert!(server.app.world().resource::<Whitelist>().contains(&id));
    assert!(Whitelist::read(&path).contains(&id));
    assert_eq!(
        replies(&mut server, admin, "whitelist add Wa"),
        [failed("commands.whitelist.add.failed")]
    );
}

#[test]
fn wl_cmd_002_a_failed_remove_does_not_write_the_file() {
    let path = temp_file("remove_failed");
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");
    std::fs::write(&path, "kept").unwrap();

    assert_eq!(
        replies(&mut server, admin, "whitelist remove Wa"),
        [failed("commands.whitelist.remove.failed")]
    );

    server.ticks(admin, 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept");
}

#[test]
fn wl_cmd_004_add_of_an_online_player_takes_its_profile_in_any_case() {
    let mut server = start(Whitelist::default());
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "whitelist add aDMIN"),
        [success("commands.whitelist.add.success", "Admin")]
    );

    server.ticks(admin, 1);
    assert!(server.app.world().resource::<Whitelist>().contains(&ADMIN));
}

#[test]
fn wl_cmd_004_remove_compares_the_id_and_replies_with_the_name_of_the_profile() {
    let wf = WhitelistEntry::offline("Wf").uuid;
    let path = temp_file("remove");
    std::fs::write(&path, format!(r#"[{{"uuid": "{wf}", "name": "Other"}}]"#)).unwrap();
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "whitelist remove Other"),
        [failed("commands.whitelist.remove.failed")]
    );
    assert_eq!(
        replies(&mut server, admin, "whitelist remove Wf"),
        [success("commands.whitelist.remove.success", "Wf")]
    );

    server.ticks(admin, 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]");
}

#[test]
fn wl_dev_001_remove_in_another_case_does_not_find_a_new_name() {
    let mut server = start(Whitelist::new([WhitelistEntry::offline("wa")]));
    let (admin, _) = server.join(ADMIN, "Admin");

    assert_eq!(
        replies(&mut server, admin, "whitelist remove WA"),
        [failed("commands.whitelist.remove.failed")]
    );
}

fn in_one_pass(server: &mut PlayServer, anchor: Entity, lines: [&str; 2]) {
    for line in lines {
        server.queue(
            anchor,
            &ServerboundChatCommand {
                command: Bounded(line),
            },
        );
    }
    server.ticks(anchor, 2);
}

#[test]
fn a_command_reads_the_edit_of_the_command_before_it_in_the_pass() {
    let path = temp_file("one_pass");
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");
    server.ticks(admin, 1);

    in_one_pass(
        &mut server,
        admin,
        ["whitelist add Wa", "whitelist remove Wa"],
    );
    let id = WhitelistEntry::offline("Wa").uuid;
    assert!(!server.app.world().resource::<Whitelist>().contains(&id));

    in_one_pass(&mut server, admin, ["whitelist on", "whitelist off"]);
    assert!(!enabled(&server));
}

#[test]
fn wl_cmd_004_an_online_name_takes_the_player_with_the_exact_case_first() {
    let (upper, lower) = (Uuid::from_u128(11), Uuid::from_u128(12));
    let path = temp_file("exact_case");
    let mut server = start(Whitelist::read(&path));
    let (admin, _) = server.join(ADMIN, "Admin");
    server.join(upper, "Wa");
    server.join(lower, "wa");

    replies(&mut server, admin, "whitelist add wa");
    server.ticks(admin, 1);

    let whitelist = server.app.world().resource::<Whitelist>();
    assert!(whitelist.contains(&lower));
    assert!(!whitelist.contains(&upper));
}
