use std::collections::HashSet;

use crate::{PackDiagnostic, PackIndex, PackStack};

/// Which files the filter of a pack hides from the packs below it.
pub struct Visibility<'t> {
    hidden: HashSet<(PackIndex, &'t str, &'t str)>,
}

impl<'t> Visibility<'t> {
    /// `tested` names each file that may be hidden, as its pack, namespace and path below the
    /// namespace directory. A filter whose pattern fails on a file is ignored as a whole.
    pub fn new(
        stack: &PackStack,
        tested: &[(PackIndex, &'t str, &'t str)],
    ) -> (Self, Vec<PackDiagnostic>) {
        let mut hidden = HashSet::new();
        let mut diagnostics = Vec::new();
        for (above, pack) in stack.packs().iter().enumerate() {
            let Some(filter) = &pack.filter else {
                continue;
            };
            let below = tested.iter().filter(|(at, _, _)| at.index() < above);
            let hides: Result<Vec<_>, String> = below
                .filter_map(
                    |&(at, namespace, relative)| match filter.hides(namespace, relative) {
                        Ok(true) => Some(Ok((at, namespace, relative))),
                        Ok(false) => None,
                        Err(reason) => Some(Err(reason)),
                    },
                )
                .collect();
            match hides {
                Ok(keys) => hidden.extend(keys),
                Err(reason) => diagnostics.push(PackDiagnostic::FilterIgnored {
                    pack: pack.id().to_owned(),
                    reason,
                }),
            }
        }
        (Self { hidden }, diagnostics)
    }

    pub fn hidden(&self, pack: PackIndex, namespace: &str, relative: &str) -> bool {
        self.hidden.contains(&(pack, namespace, relative))
    }
}
