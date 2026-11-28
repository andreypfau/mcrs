use std::collections::HashMap;

use crate::declaration::Declaration;
use crate::registries::Kind;

pub(crate) struct Owner<'d, 'p> {
    pub(crate) tag: bool,
    pub(crate) declaration: &'d Declaration,
    pub(crate) below: &'p str,
}

#[derive(Clone)]
pub struct ContentPaths<'d> {
    directories: HashMap<&'d str, &'d Declaration>,
}

impl<'d> ContentPaths<'d> {
    #[must_use]
    pub fn new(declarations: &'d [Declaration]) -> Self {
        let directories = declarations
            .iter()
            .map(|declaration| (declaration.key.path(), declaration))
            .collect();
        Self { directories }
    }

    #[must_use]
    pub fn needs(&self, relative: &str) -> bool {
        self.owner(relative)
            .is_some_and(|owner| owner.tag || matches!(owner.declaration.kind, Kind::Data(_)))
    }

    pub(crate) fn owner<'p>(&self, relative: &'p str) -> Option<Owner<'d, 'p>> {
        let (tag, inside) = match relative.strip_prefix("tags/") {
            Some(inside) => (true, inside),
            None => (false, relative),
        };
        inside.match_indices('/').rev().find_map(|(at, _)| {
            let (directory, below) = inside.split_at_checked(at)?;
            let declaration = *self.directories.get(directory)?;
            if !tag && matches!(declaration.kind, Kind::Fixed) {
                return None;
            }
            Some(Owner {
                tag,
                declaration,
                below: below.strip_prefix('/')?,
            })
        })
    }
}
