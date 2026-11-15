use alloc::vec::Vec;
use core::mem;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::nbt_path_operation_error::NbtPathOperationError;
use crate::node::Node;
use crate::selected_tag::SelectedTag;

const DEPTH_LIMIT: usize = 512;

/// A path of one or more nodes into an NBT tag, read with [`str::parse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NbtPath {
    nodes: Vec<Node>,
}

impl NbtPath {
    pub(crate) const fn new(nodes: Vec<Node>) -> NbtPath {
        NbtPath { nodes }
    }

    /// Appends each tag that the path selects from `root` to `out`, in order.
    pub fn select<'t>(&self, root: &'t NbtTag, out: &mut Vec<SelectedTag<'t>>) {
        let Some((last, middle)) = self.nodes.split_last() else {
            return;
        };
        let mut current: Vec<&NbtTag> = Vec::new();
        current.push(root);
        let mut next: Vec<&NbtTag> = Vec::new();
        for node in middle {
            for tag in current.drain(..) {
                node.select(tag, |selected| {
                    if let SelectedTag::Tag(selected) = selected {
                        next.push(selected);
                    }
                });
            }
            mem::swap(&mut current, &mut next);
        }
        for tag in current {
            last.select(tag, |selected| out.push(selected));
        }
    }

    /// Gets the tags of the path in `root`, and creates each missing tag on the way. The new
    /// tag of the last node is a copy of `new_tag`; the new tag of another node is an empty list
    /// before an index node and an empty compound otherwise. Pushes to `out` the tags of the
    /// last node, except array elements. Fails when a node before the last one gives no tag;
    /// the tags that it created before the fault stay.
    pub fn get_or_create<'t>(
        &self,
        root: &'t mut NbtTag,
        new_tag: &NbtTag,
        out: &mut Vec<&'t mut NbtTag>,
    ) -> Result<(), NbtPathOperationError> {
        get_or_create(&self.nodes, SliceEnd::AtLastNode, root, new_tag, out)
    }

    /// Sets the tags of the path in `root` to copies of `value`. It first does
    /// [`NbtPath::get_or_create`] for each node except the last one, and fails when one of these
    /// gives no tag. A last node that writes nothing is no failure. The created tags stay after
    /// a failure and after a last node that writes nothing. Fails also, before
    /// any change, when the number of nodes plus the highest level in `value` is 512 or more.
    /// `value` has the level 0, and each list element or compound value one more than its
    /// container. An array takes the value converted to its element type.
    pub fn set(&self, root: &mut NbtTag, value: &NbtTag) -> Result<(), NbtPathOperationError> {
        let Some((last, middle)) = self.nodes.split_last() else {
            return Ok(());
        };
        if level_at_least(value, DEPTH_LIMIT.saturating_sub(self.nodes.len())) {
            return Err(NbtPathOperationError::TooDeep);
        }
        let mut parents: Vec<&mut NbtTag> = Vec::new();
        if middle.is_empty() {
            parents.push(root);
        } else {
            let new_tag = new_tag_before(last);
            get_or_create(
                middle,
                SliceEnd::BeforeLastNode,
                root,
                &new_tag,
                &mut parents,
            )?;
        }
        for parent in parents {
            last.write(parent, value);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SliceEnd {
    AtLastNode,
    BeforeLastNode,
}

fn get_or_create<'t>(
    nodes: &[Node],
    end: SliceEnd,
    root: &'t mut NbtTag,
    new_tag: &NbtTag,
    out: &mut Vec<&'t mut NbtTag>,
) -> Result<(), NbtPathOperationError> {
    let mut current: Vec<&'t mut NbtTag> = Vec::new();
    current.push(root);
    let mut next: Vec<&'t mut NbtTag> = Vec::new();
    let mut nodes = nodes.iter().peekable();
    while let Some(node) = nodes.next() {
        let following = nodes.peek().copied();
        let before_last_node = following.is_some() || end == SliceEnd::BeforeLastNode;
        let target = if following.is_some() {
            &mut next
        } else {
            &mut *out
        };
        let before = target.len();
        let mut gave_element = false;
        for tag in current.drain(..) {
            let create = || following.map_or_else(|| new_tag.clone(), new_tag_before);
            gave_element |= node.get_or_create(tag, create, target);
        }
        if before_last_node && !gave_element && target.len() == before {
            return Err(NbtPathOperationError::NoTag);
        }
        mem::swap(&mut current, &mut next);
    }
    Ok(())
}

fn new_tag_before(node: &Node) -> NbtTag {
    if node.is_index() {
        NbtTag::List(Vec::new())
    } else {
        NbtTag::Compound(NbtCompound::new())
    }
}

fn level_at_least(tag: &NbtTag, level: usize) -> bool {
    let Some(inner) = level.checked_sub(1) else {
        return true;
    };
    match tag {
        NbtTag::List(list) => list.iter().any(|element| level_at_least(element, inner)),
        NbtTag::Compound(compound) => compound
            .child_tags
            .iter()
            .any(|(_, value)| level_at_least(value, inner)),
        _ => false,
    }
}
