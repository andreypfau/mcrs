use crate::graph::Graph;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use crate::requirement::Requirement;
use alloc::string::String;

/// Whether a usage shows in brackets as optional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Required,
    Optional,
}

/// How much of the tree below a node its short usage shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageDepth {
    /// The node alone.
    Node,
    /// The node, then its redirect or its children.
    Children,
}

impl<A, R, H> Graph<A, R, H> {
    /// The usage text of a node.
    pub fn write_usage_text(&self, node: NodeId, out: &mut String) {
        match self.node(node).map(|node| node.kind()) {
            Some(NodeKind::Literal(name)) => out.push_str(name),
            Some(NodeKind::Argument { name, .. }) => {
                out.push('<');
                out.push_str(name);
                out.push('>');
            }
            Some(NodeKind::Root) | None => {}
        }
    }

    /// Writes the short usage of a node; `false` when the node has none.
    pub fn short_usage<S>(
        &self,
        id: NodeId,
        source: &S,
        presence: Presence,
        depth: UsageDepth,
        out: &mut String,
    ) -> bool
    where
        R: Requirement<S>,
    {
        let Some(node) = self.node(id).filter(|node| node.requirement().test(source)) else {
            return false;
        };
        let optional = presence == Presence::Optional;
        if optional {
            out.push('[');
        }
        self.write_usage_text(id, out);
        if optional {
            out.push(']');
        }
        if depth == UsageDepth::Node {
            return true;
        }
        if let Some(target) = node.redirect() {
            out.push(' ');
            if target == NodeId::ROOT {
                out.push_str("...");
            } else {
                out.push_str("-> ");
                self.write_usage_text(target, out);
            }
            return true;
        }
        let child_optional = node.command().is_some();
        let child_presence = if child_optional {
            Presence::Optional
        } else {
            Presence::Required
        };
        let usable = |&child: &NodeId| {
            self.node(child)
                .is_some_and(|child| child.requirement().test(source))
        };
        let mut children = node.children().iter().copied().filter(usable);
        let (Some(first), second) = (children.next(), children.next()) else {
            return true;
        };
        let mark = out.len();
        out.push(' ');
        let Some(second) = second else {
            let child_depth = if child_optional {
                UsageDepth::Node
            } else {
                UsageDepth::Children
            };
            if !self.short_usage(first, source, child_presence, child_depth, out) {
                out.truncate(mark);
            }
            return true;
        };
        let first_start = out.len();
        self.short_usage(first, source, child_presence, UsageDepth::Node, out);
        let first_end = out.len();
        let mut same = true;
        for child in core::iter::once(second).chain(children) {
            let start = out.len();
            self.short_usage(child, source, child_presence, UsageDepth::Node, out);
            same &= out.get(start..) == out.get(first_start..first_end);
            out.truncate(start);
        }
        if same {
            if child_optional {
                out.insert(first_start, '[');
                out.push(']');
            }
            return true;
        }
        out.truncate(first_start);
        out.push(if child_optional { '[' } else { '(' });
        for (index, child) in node.children().iter().copied().filter(usable).enumerate() {
            if index > 0 {
                out.push('|');
            }
            self.write_usage_text(child, out);
        }
        out.push(if child_optional { ']' } else { ')' });
        true
    }
}
