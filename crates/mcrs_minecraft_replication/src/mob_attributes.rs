use bytes::BytesMut;
use mcrs_minecraft_entity::AttributeBases;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::clientbound::{AttributeSnapshot, ClientboundUpdateAttributes};

use crate::attribute_snapshot::attribute_snapshot;
use crate::mob_attribute::MobAttribute;

/// The syncable attribute instances of a mob that changed after its spawn, with the latest state
/// of each, and the ones that its next update owes its viewers (ATR-NET-003).
#[derive(Clone, Debug, Default)]
pub(crate) struct MobAttributes {
    bases: Option<AttributeBases>,
    changed: Vec<MobAttribute>,
    unsent: Vec<Attribute>,
}

impl MobAttributes {
    pub(crate) fn new(bases: Option<AttributeBases>) -> Self {
        Self {
            bases,
            changed: Vec::new(),
            unsent: Vec::new(),
        }
    }

    pub(crate) fn record(&mut self, state: &MobAttribute) {
        match self
            .changed
            .iter_mut()
            .find(|held| held.attribute == state.attribute)
        {
            Some(held) => held.clone_from(state),
            None => self.changed.push(state.clone()),
        }
        if !self.unsent.contains(&state.attribute) {
            self.unsent.push(state.attribute);
        }
    }

    /// Appends the `update_attributes` of the instances that changed after the last update, when
    /// one did.
    pub(crate) fn write_unsent(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        if self.unsent.is_empty() {
            return Ok(());
        }
        self.write(id, |held| self.unsent.contains(&held.attribute), bytes)
    }

    pub(crate) fn mark_sent(&mut self) {
        self.unsent.clear();
    }

    /// Appends the `update_attributes` of the spawn bundle (TRK-DATA-004): each instance that
    /// differs from that of a new mob of the type, when one does.
    pub(crate) fn write_spawn(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        self.write(id, |held| held.instance.is_some(), bytes)
    }

    /// Appends the `update_attributes` of every instance that changed after the spawn, for a
    /// client that missed updates.
    pub(crate) fn write_all(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        self.write(id, |_| true, bytes)
    }

    fn write(
        &self,
        id: VarInt,
        keep: impl Fn(&MobAttribute) -> bool,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let attributes: Vec<AttributeSnapshot<'_>> = self
            .changed
            .iter()
            .filter(|held| keep(held))
            .map(|held| attribute_snapshot(self.bases, held.attribute, held.instance.as_ref()))
            .collect();
        if attributes.is_empty() {
            return Ok(());
        }
        write_record(
            bytes,
            &ClientboundUpdateAttributes {
                entity_id: id,
                attributes,
            },
        )
    }
}
