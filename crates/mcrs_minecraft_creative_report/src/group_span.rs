use core::ops::Range;

use crate::Group;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSpan {
    pub group: Group,
    pub stacks: Range<usize>,
}
