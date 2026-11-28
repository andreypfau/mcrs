use bytes::BytesMut;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{AttributeBases, AttributeSnapshot};
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::clientbound::ClientboundUpdateAttributes;

use crate::attribute_state::AttributeState;

#[derive(Clone, Debug, Default)]
pub(crate) struct SubjectAttributes {
    bases: Option<AttributeBases>,
    held: Vec<AttributeState>,
    unsent: Vec<Attribute>,
}

impl SubjectAttributes {
    pub(crate) fn with_stored(bases: Option<AttributeBases>, stored: Vec<AttributeState>) -> Self {
        Self {
            bases,
            held: stored,
            unsent: Vec::new(),
        }
    }

    pub(crate) fn record(&mut self, state: &AttributeState) {
        match self
            .held
            .iter_mut()
            .find(|held| held.attribute == state.attribute)
        {
            Some(held) => held.clone_from(state),
            None => self.held.push(state.clone()),
        }
        if !self.unsent.contains(&state.attribute) {
            self.unsent.push(state.attribute);
        }
    }

    pub(crate) fn write_unsent(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        if self.unsent.is_empty() {
            return Ok(());
        }
        self.write(id, |held| self.unsent.contains(&held.attribute), bytes)
    }

    pub(crate) fn mark_sent(&mut self) {
        self.unsent.clear();
    }

    pub(crate) fn write_spawn(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        self.write(id, |held| held.instance.is_some(), bytes)
    }

    pub(crate) fn write_all(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        self.write(id, |_| true, bytes)
    }

    fn write(
        &self,
        id: VarInt,
        keep: impl Fn(&AttributeState) -> bool,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let Some(bases) = self.bases else {
            return Ok(());
        };
        let attributes: Vec<AttributeSnapshot<'_>> = self
            .held
            .iter()
            .filter(|held| keep(held))
            .filter_map(|held| {
                AttributeSnapshot::from_instance(bases, held.attribute, held.instance.as_ref())
            })
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
