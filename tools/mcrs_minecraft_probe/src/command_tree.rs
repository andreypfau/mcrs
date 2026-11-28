use anyhow::Context;
use mcrs_minecraft_command_tree::{CommandNode, CommandNodeKind};
use mcrs_minecraft_keys::command_argument_type;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol_game::clientbound::ClientboundCommands;
use serde::{Deserialize, Serialize};

const DEPTH_LIMIT: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandTree {
    pub client: String,
    pub node_count: usize,
    pub roots: Vec<TreeNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    pub index: i32,
    #[serde(flatten)]
    pub kind: TreeNodeKind,
    pub executable: bool,
    pub restricted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect: Option<i32>,
    pub children: Vec<TreeNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum TreeNodeKind {
    Root,
    Literal {
        name: String,
    },
    Argument {
        name: String,
        parser: String,
        properties: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suggestions: Option<String>,
    },
}

impl CommandTree {
    pub(crate) fn read(client: &str, mut body: &[u8], names: &[String]) -> anyhow::Result<Self> {
        let packet = ClientboundCommands::decode(&mut body)?;
        anyhow::ensure!(body.is_empty(), "the commands packet has trailing bytes");
        let root = node(&packet.nodes, packet.root.0)?;
        let mut roots = Vec::new();
        for child in &root.children {
            let candidate = node(&packet.nodes, child.0)?;
            if let CommandNodeKind::Literal(name) = &candidate.kind
                && names.iter().any(|wanted| wanted == name.as_ref())
            {
                roots.push(subtree(&packet.nodes, child.0, 0)?);
            }
        }
        Ok(Self {
            client: client.to_owned(),
            node_count: packet.nodes.len(),
            roots,
        })
    }
}

fn node<'a, 'b>(nodes: &'b [CommandNode<'a>], index: i32) -> anyhow::Result<&'b CommandNode<'a>> {
    usize::try_from(index)
        .ok()
        .and_then(|index| nodes.get(index))
        .with_context(|| format!("the node {index} is not in the packet"))
}

fn subtree(nodes: &[CommandNode<'_>], index: i32, depth: usize) -> anyhow::Result<TreeNode> {
    anyhow::ensure!(
        depth < DEPTH_LIMIT,
        "the command tree is deeper than {DEPTH_LIMIT}"
    );
    let source = node(nodes, index)?;
    let kind = match &source.kind {
        CommandNodeKind::Root => TreeNodeKind::Root,
        CommandNodeKind::Literal(name) => TreeNodeKind::Literal {
            name: name.to_string(),
        },
        CommandNodeKind::Argument {
            name,
            parser,
            suggestions,
        } => {
            let id = usize::try_from(parser.id.0).context("the parser id is negative")?;
            let key = command_argument_type::ENTRIES
                .get(id)
                .with_context(|| format!("the parser {id} is not in the registry"))?;
            TreeNodeKind::Argument {
                name: name.to_string(),
                parser: key.to_string(),
                properties: format!("{:?}", parser.properties),
                suggestions: suggestions.as_ref().map(ToString::to_string),
            }
        }
    };
    let children = source
        .children
        .iter()
        .map(|child| subtree(nodes, child.0, depth + 1))
        .collect::<anyhow::Result<Vec<TreeNode>>>()?;
    Ok(TreeNode {
        index,
        kind,
        executable: source.executable,
        restricted: source.restricted,
        redirect: source.redirect.map(|redirect| redirect.0),
        children,
    })
}
