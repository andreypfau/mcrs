use std::collections::{BTreeMap, BTreeSet};

use mcrs_engine_muxer::{EntityId, ViewerId};

/// The subjects that the client of each viewer holds, players and mobs alike, and the reverse
/// map. Both maps hold no empty entry.
#[derive(Debug, Default)]
pub(crate) struct HoldSet {
    by_viewer: BTreeMap<ViewerId, BTreeSet<EntityId>>,
    /// Sorted, so the updates of a subject go to its viewers in one order.
    by_subject: BTreeMap<EntityId, Vec<ViewerId>>,
}

impl HoldSet {
    pub(crate) fn insert(&mut self, viewer: ViewerId, subject: EntityId) {
        if self.by_viewer.entry(viewer).or_default().insert(subject) {
            let holders = self.by_subject.entry(subject).or_default();
            if let Err(at) = holders.binary_search(&viewer) {
                holders.insert(at, viewer);
            }
        }
    }

    pub(crate) fn remove(&mut self, viewer: ViewerId, subject: EntityId) {
        if let Some(subjects) = self.by_viewer.get_mut(&viewer) {
            subjects.remove(&subject);
            if subjects.is_empty() {
                self.by_viewer.remove(&viewer);
            }
        }
        remove_holder(&mut self.by_subject, subject, viewer);
    }

    /// The viewers whose client holds `subject`, in order.
    pub(crate) fn holders(&self, subject: EntityId) -> &[ViewerId] {
        self.by_subject.get(&subject).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn contains(&self, viewer: ViewerId, subject: EntityId) -> bool {
        self.by_viewer
            .get(&viewer)
            .is_some_and(|subjects| subjects.contains(&subject))
    }

    /// Each subject that some client holds, in the order of the ids, with its holders.
    pub(crate) fn held(&self) -> impl Iterator<Item = (EntityId, &[ViewerId])> {
        self.by_subject
            .iter()
            .map(|(&subject, holders)| (subject, holders.as_slice()))
    }

    /// Takes `subject` out of every client, and gives the viewers that held it.
    pub(crate) fn remove_subject(&mut self, subject: EntityId) -> Vec<ViewerId> {
        let holders = self.by_subject.remove(&subject).unwrap_or_default();
        for viewer in &holders {
            if let Some(subjects) = self.by_viewer.get_mut(viewer) {
                subjects.remove(&subject);
                if subjects.is_empty() {
                    self.by_viewer.remove(viewer);
                }
            }
        }
        holders
    }

    /// Takes out of the client of `viewer` each subject for which `release` is true.
    pub(crate) fn release_where(
        &mut self,
        viewer: ViewerId,
        mut release: impl FnMut(EntityId) -> bool,
    ) {
        let Some(subjects) = self.by_viewer.get_mut(&viewer) else {
            return;
        };
        let by_subject = &mut self.by_subject;
        subjects.retain(|&subject| {
            if !release(subject) {
                return true;
            }
            remove_holder(by_subject, subject, viewer);
            false
        });
        if subjects.is_empty() {
            self.by_viewer.remove(&viewer);
        }
    }
}

fn remove_holder(
    by_subject: &mut BTreeMap<EntityId, Vec<ViewerId>>,
    subject: EntityId,
    viewer: ViewerId,
) {
    let Some(holders) = by_subject.get_mut(&subject) else {
        return;
    };
    if let Ok(at) = holders.binary_search(&viewer) {
        holders.remove(at);
    }
    if holders.is_empty() {
        by_subject.remove(&subject);
    }
}
