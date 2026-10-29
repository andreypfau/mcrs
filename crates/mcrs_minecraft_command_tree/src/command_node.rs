use crate::argument_parser::ArgumentParser;
use crate::command_node_kind::CommandNodeKind;
use crate::parser_properties::decode_properties;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_wire::{Decode, Encode, Error, Result, VarInt};
use std::borrow::Cow;
use std::io::Write;

const KIND_MASK: u8 = 0x03;
const EXECUTABLE: u8 = 0x04;
const REDIRECT: u8 = 0x08;
const SUGGESTIONS: u8 = 0x10;
const RESTRICTED: u8 = 0x20;

/// One node of the command tree of a `commands` packet.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandNode<'a> {
    pub kind: CommandNodeKind<'a>,
    /// The indices of the child nodes in the node list.
    pub children: Vec<VarInt>,
    /// The index of the node that the rest of the input continues at.
    pub redirect: Option<VarInt>,
    /// The input that ends at this node is a complete command.
    pub executable: bool,
    /// Only a source with the restricted permission may use the node.
    pub restricted: bool,
}

impl Encode for CommandNode<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let (kind, suggestions) = match &self.kind {
            CommandNodeKind::Root => (0, false),
            CommandNodeKind::Literal(_) => (1, false),
            CommandNodeKind::Argument { suggestions, .. } => (2, suggestions.is_some()),
        };
        let flags = kind
            | if self.executable { EXECUTABLE } else { 0 }
            | if self.redirect.is_some() { REDIRECT } else { 0 }
            | if suggestions { SUGGESTIONS } else { 0 }
            | if self.restricted { RESTRICTED } else { 0 };
        flags.encode(&mut w)?;
        self.children.encode(&mut w)?;
        if let Some(redirect) = self.redirect {
            redirect.encode(&mut w)?;
        }
        match &self.kind {
            CommandNodeKind::Root => {}
            CommandNodeKind::Literal(name) => name.as_ref().encode(&mut w)?,
            CommandNodeKind::Argument {
                name,
                parser,
                suggestions,
            } => {
                name.as_ref().encode(&mut w)?;
                parser.id.encode(&mut w)?;
                parser.properties.encode(&mut w)?;
                if let Some(suggestions) = suggestions {
                    suggestions.encode(&mut w)?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> Decode<'a> for CommandNode<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let flags = u8::decode(r)?;
        let children = Vec::decode(r)?;
        let redirect = (flags & REDIRECT != 0)
            .then(|| VarInt::decode(r))
            .transpose()?;
        let kind = match flags & KIND_MASK {
            0 => CommandNodeKind::Root,
            1 => CommandNodeKind::Literal(Cow::decode(r)?),
            2 => {
                let name = Cow::decode(r)?;
                let id = VarInt::decode(r)?;
                let properties = decode_properties(id, r)?;
                let suggestions = (flags & SUGGESTIONS != 0)
                    .then(|| ResourceLocation::decode(r))
                    .transpose()?;
                CommandNodeKind::Argument {
                    name,
                    parser: ArgumentParser { id, properties },
                    suggestions,
                }
            }
            other => return Err(Error::invalid(format!("node kind {other} does not exist"))),
        };
        Ok(Self {
            kind,
            children,
            redirect,
            executable: flags & EXECUTABLE != 0,
            restricted: flags & RESTRICTED != 0,
        })
    }
}
