use alloc::vec::Vec;
use core::mem;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::nbt_path_operation_error::NbtPathOperationError;
use crate::node::Node;
use crate::nothing_found::NothingFound;
use crate::selected_tag::SelectedTag;

const DEPTH_LIMIT: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NbtPath {
    nodes: Vec<Node>,
    ends: Vec<usize>,
}

impl NbtPath {
    pub(crate) const fn new(nodes: Vec<Node>, ends: Vec<usize>) -> NbtPath {
        NbtPath { nodes, ends }
    }

    pub fn select<'t>(&self, root: &'t NbtTag, out: &mut Vec<SelectedTag<'t>>) {
        self.try_select(root, out).unwrap_or_default();
    }

    pub fn try_select<'t>(
        &self,
        root: &'t NbtTag,
        out: &mut Vec<SelectedTag<'t>>,
    ) -> Result<(), NothingFound> {
        let Some((last, middle)) = self.nodes.split_last() else {
            return Ok(());
        };
        let mut current: Vec<&NbtTag> = Vec::new();
        current.push(root);
        let mut next: Vec<&NbtTag> = Vec::new();
        for (node, &end) in middle.iter().zip(&self.ends) {
            for tag in current.drain(..) {
                node.select(tag, |selected| {
                    if let SelectedTag::Tag(selected) = selected {
                        next.push(selected);
                    }
                });
            }
            if next.is_empty() {
                return Err(NothingFound { end });
            }
            mem::swap(&mut current, &mut next);
        }
        let before = out.len();
        for tag in current {
            last.select(tag, |selected| out.push(selected));
        }
        if out.len() == before {
            let end = self.ends.last().copied().unwrap_or_default();
            return Err(NothingFound { end });
        }
        Ok(())
    }

    pub fn get_or_create<'t>(
        &self,
        root: &'t mut NbtTag,
        new_tag: &NbtTag,
        out: &mut Vec<&'t mut NbtTag>,
    ) -> Result<(), NbtPathOperationError> {
        get_or_create(&self.nodes, SliceEnd::AtLastNode, root, new_tag, out)
    }

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
