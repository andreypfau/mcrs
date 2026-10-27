use crate::tree::{Graph, NodeId, NodeKind, Requirement};
use alloc::string::String;

impl<A, R, H> Graph<A, R, H> {
    /// The usage text of a node (CMD-USAGE-001).
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

    /// Writes the short usage of a node (CMD-USAGE-002); `false` when the node has none.
    pub fn short_usage<S>(
        &self,
        id: NodeId,
        source: &S,
        optional: bool,
        deep: bool,
        out: &mut String,
    ) -> bool
    where
        R: Requirement<S>,
    {
        let Some(node) = self.node(id).filter(|node| node.requirement().test(source)) else {
            return false;
        };
        if optional {
            out.push('[');
        }
        self.write_usage_text(id, out);
        if optional {
            out.push(']');
        }
        if deep {
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
            if !self.short_usage(first, source, child_optional, child_optional, out) {
                out.truncate(mark);
            }
            return true;
        };
        let first_start = out.len();
        self.short_usage(first, source, child_optional, true, out);
        let first_end = out.len();
        let mut same = true;
        for child in core::iter::once(second).chain(children) {
            let start = out.len();
            self.short_usage(child, source, child_optional, true, out);
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
