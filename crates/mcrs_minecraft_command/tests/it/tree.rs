use crate::{GUEST, OP};
use mcrs_command::{GraphBuilder, NodeId, ParseScratch, build_list};
use mcrs_minecraft_command::{
    CommandLevel, EntityArgument, GameModeArgument, PermissionCheck, VanillaArgument, VanillaGraph,
    player_tree, suggestion_reply,
};
use mcrs_minecraft_protocol::command::{CommandNode, CommandNodeKind, ParserProperties};
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

fn child<'p, 'a>(
    tree: &'p ClientboundCommands<'a>,
    node: &'p CommandNode<'a>,
    name: &str,
) -> &'p CommandNode<'a> {
    node.children
        .iter()
        .filter_map(|child| tree.nodes.get(child.0 as usize))
        .find(|child| node_name(child) == name)
        .unwrap_or_else(|| panic!("no child {name}"))
}

fn node_name<'p>(node: &'p CommandNode<'_>) -> &'p str {
    match &node.kind {
        CommandNodeKind::Literal(name) | CommandNodeKind::Argument { name, .. } => name,
        CommandNodeKind::Root => "",
    }
}

fn child_names<'p>(tree: &'p ClientboundCommands<'_>, node: &CommandNode<'_>) -> Vec<&'p str> {
    node.children
        .iter()
        .filter_map(|child| tree.nodes.get(child.0 as usize))
        .map(node_name)
        .collect()
}

fn parser<'p>(node: &'p CommandNode<'_>) -> (VarInt, &'p ParserProperties<'p>) {
    let CommandNodeKind::Argument { parser, .. } = &node.kind else {
        panic!("{node:?}");
    };
    (parser.id, &parser.properties)
}

#[test]
fn a_player_sees_only_the_nodes_it_can_use() {
    let graph = graph();
    let guest = player_tree(&graph, &GUEST).unwrap();
    let root = guest.nodes.get(guest.root.0 as usize).unwrap();
    assert_eq!(child_names(&guest, root), ["help"]);
    assert_eq!(guest.nodes.len(), 2);
    assert!(!child(&guest, root, "help").restricted);

    let op = player_tree(&graph, &OP).unwrap();
    assert_round_trip(&op);
    let root = op.nodes.get(op.root.0 as usize).unwrap();
    assert_eq!(child_names(&op, root), ["gamemode", "tp", "help"]);
    let gamemode = child(&op, root, "gamemode");
    assert!(gamemode.restricted && !gamemode.executable);
    let mode = child(&op, gamemode, "gamemode");
    assert!(mode.executable && !mode.restricted);
    assert_eq!(parser(mode), (VarInt(42), &ParserProperties::None));
    let target = child(&op, mode, "target");
    assert_eq!(
        parser(target),
        (
            VarInt(6),
            &ParserProperties::Entity {
                single: false,
                players_only: true
            }
        )
    );
    let tp = child(&op, root, "tp");
    let redirect = tp.redirect.and_then(|id| op.nodes.get(id.0 as usize));
    assert_eq!(redirect, Some(gamemode));
    assert!(child(&op, root, "help").executable);
}

fn reply(graph: &VanillaGraph<u32>, text: &str) -> (VarInt, VarInt, Vec<String>) {
    let mut scratch = ParseScratch::default();
    let parse = graph.parse(text, 1, &OP, &mut scratch);
    let mut out = Vec::new();
    graph.suggest(&parse, &OP, &mut out);
    let list = build_list(text, &mut out);
    let reply = suggestion_reply(VarInt(7), text, &list);
    let texts = reply
        .suggestions
        .iter()
        .map(|suggestion| suggestion.text.to_string())
        .collect();
    (reply.start, reply.length, texts)
}

#[test]
fn a_reply_counts_utf16_code_units_from_the_slash() {
    let open = PermissionCheck::AlwaysPass;
    let mut b = GraphBuilder::<VanillaArgument, PermissionCheck, u32>::new();
    let face = b.literal(NodeId::ROOT, "\u{1f600}", open).unwrap();
    b.executes(face, 0).unwrap();
    let go = b.literal(face, "go", open).unwrap();
    b.executes(go, 0).unwrap();
    let faces = b.literal(NodeId::ROOT, "\u{1f600}x", open).unwrap();
    b.executes(faces, 0).unwrap();
    let faces = b.freeze().unwrap();

    let cases = [
        (&graph(), "/gamem", (1, 5, vec!["gamemode"])),
        (&faces, "/\u{1f600} g", (4, 1, vec!["go"])),
        (&faces, "/\u{1f600}", (1, 2, vec!["\u{1f600}x"])),
    ];
    for (graph, text, (start, length, texts)) in cases {
        assert_eq!(
            reply(graph, text),
            (
                VarInt(start),
                VarInt(length),
                texts.iter().map(|t| t.to_string()).collect()
            ),
            "{text}"
        );
    }
}

fn literal(name: &'static str, children: &[i32]) -> CommandNode<'static> {
    CommandNode {
        kind: CommandNodeKind::Literal(name.into()),
        children: children.iter().copied().map(VarInt).collect(),
        redirect: None,
        executable: true,
        restricted: false,
    }
}

fn packet(nodes: Vec<CommandNode<'static>>) -> ClientboundCommands<'static> {
    let mut root = literal("", &[1, 2]);
    root.kind = CommandNodeKind::Root;
    root.executable = false;
    let mut all = vec![root];
    all.extend(nodes);
    ClientboundCommands {
        nodes: all,
        root: VarInt(0),
    }
}

#[test]
fn the_client_tree_shares_a_node_two_parents_name_and_rejects_a_child_cycle() {
    use mcrs_minecraft_command::{TreeError, client_tree};
    let shared = packet(vec![
        literal("a", &[3]),
        literal("b", &[3]),
        literal("run", &[]),
    ]);
    let graph = client_tree(&shared, &[]).unwrap();
    let mut scratch = ParseScratch::default();
    for line in ["a run", "b run"] {
        let parse = graph.parse(line, 0, &OP, &mut scratch);
        assert!(parse.check(&graph).is_ok(), "{line}");
    }
    let cycle = packet(vec![
        literal("a", &[3]),
        literal("b", &[]),
        literal("c", &[1]),
    ]);
    assert_eq!(
        client_tree(&cycle, &[]).err(),
        Some(TreeError::ChildCycle(1))
    );
}
