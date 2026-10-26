use crate::{GUEST, OP};
use mcrs_command::{GraphBuilder, NodeId, ParseScratch, build_list};
use mcrs_minecraft_command::{
    CommandLevel, EntityArgument, GameModeArgument, PermissionCheck, VanillaArgument, VanillaGraph,
    player_tree, suggestion_reply,
};
use mcrs_minecraft_protocol::command::{CommandNodeKind, ParserProperties};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundCommands;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};

fn graph() -> VanillaGraph<u32> {
    let op = PermissionCheck::Require(CommandLevel::Gamemasters);
    let open = PermissionCheck::AlwaysPass;
    let mut b = GraphBuilder::<VanillaArgument, PermissionCheck, u32>::new();
    let gamemode = b.literal(NodeId::ROOT, "gamemode", op).unwrap();
    let mode = b
        .argument(gamemode, "gamemode", GameModeArgument, open)
        .unwrap();
    b.executes(mode.node(), 0).unwrap();
    let target = b
        .argument(mode.node(), "target", EntityArgument::PLAYERS, open)
        .unwrap();
    b.executes(target.node(), 0).unwrap();
    let tp = b.literal(NodeId::ROOT, "tp", op).unwrap();
    b.redirect(tp, gamemode).unwrap();
    let help = b.literal(NodeId::ROOT, "help", open).unwrap();
    b.executes(help, 1).unwrap();
    b.freeze().unwrap()
}

fn assert_round_trip(packet: &ClientboundCommands<'_>) {
    let mut bytes = Vec::new();
    packet.encode(&mut bytes).unwrap();
    let mut slice = bytes.as_slice();
    let decoded = ClientboundCommands::decode(&mut slice).unwrap();
    assert!(slice.is_empty(), "the decode ends at the last byte");
    assert_eq!(&decoded, packet);
}

#[test]
fn a_player_sees_only_the_nodes_it_can_use() {
    let graph = graph();
    let guest = player_tree(&graph, &GUEST);
    assert_eq!(guest.nodes.len(), 2);
    assert_eq!(guest.nodes[1].kind, CommandNodeKind::Literal("help".into()));
    assert!(!guest.nodes[1].restricted);

    let op = player_tree(&graph, &OP);
    assert_round_trip(&op);
    let [root, gamemode, mode, target, tp, help] = op.nodes.as_slice() else {
        panic!("{op:?}");
    };
    assert_eq!(root.children, [VarInt(1), VarInt(4), VarInt(5)]);
    assert!(gamemode.restricted && !gamemode.executable);
    assert!(mode.executable && !mode.restricted);
    let CommandNodeKind::Argument { parser, .. } = &target.kind else {
        panic!("{target:?}");
    };
    assert_eq!(parser.id, VarInt(6));
    assert_eq!(
        parser.properties,
        ParserProperties::Entity {
            single: false,
            players_only: true
        }
    );
    let CommandNodeKind::Argument { parser, .. } = &mode.kind else {
        panic!("{mode:?}");
    };
    assert_eq!(parser.id, VarInt(42));
    assert_eq!(tp.redirect, Some(VarInt(1)));
    assert!(help.executable);
}

#[test]
fn a_reply_counts_utf16_code_units_from_the_slash() {
    let graph = graph();
    let text = "/gamem";
    let mut scratch = ParseScratch::default();
    let parse = graph.parse(text, 1, &OP, &mut scratch);
    let mut out = Vec::new();
    graph.suggest(&parse, &OP, &mut out);
    let list = build_list(text, &mut out);
    let reply = suggestion_reply(VarInt(7), text, &list);
    assert_eq!((reply.start, reply.length), (VarInt(1), VarInt(5)));
    assert_eq!(reply.suggestions.len(), 1);
    assert_eq!(reply.suggestions[0].text, "gamemode");

    let text = "/gamemode \u{1f600}";
    let parse = graph.parse(text, 1, &OP, &mut scratch);
    let mut out = Vec::new();
    graph.suggest(&parse, &OP, &mut out);
    let list = build_list(text, &mut out);
    let reply = suggestion_reply(VarInt(8), text, &list);
    assert!(reply.suggestions.is_empty());
}
