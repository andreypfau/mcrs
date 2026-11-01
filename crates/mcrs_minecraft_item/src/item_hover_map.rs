use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_nbt::{from_tag, to_nbt_tag};
use mcrs_minecraft_registries::{Named, ResolveError};
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_text::{ItemHover, TextMap};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::Template;
use crate::dialog::DialogRef;

/// Maps the wire form of a text, [`ItemHover`], to the typed form, `Template<Named>`, and back.
/// The typed direction checks a hover as a data-pack load checks it.
#[derive(Clone, Copy, Debug, Default)]
pub struct ItemHoverMap;

fn convert<A: Serialize, B: DeserializeOwned>(value: &A) -> Result<B, ResolveError> {
    to_nbt_tag(value)
        .and_then(from_tag)
        .map_err(|error| ResolveError::new(error.to_string()))
}

impl TextMap<ItemHover, Template<Named>> for ItemHoverMap {
    type Error = ResolveError;

    fn item(&self, item: &ItemHover) -> Result<Template<Named>, ResolveError> {
        convert(item)
    }

    fn entity(
        &self,
        entity: &ResourceLocationBuf,
    ) -> Result<ResourceKeyBuf<EntityType>, ResolveError> {
        Ok(ResourceKeyBuf::from_location(entity.clone()))
    }

    fn dialog(
        &self,
        dialog: &mcrs_minecraft_text::DialogRef,
    ) -> Result<DialogRef<Named>, ResolveError> {
        convert(dialog)
    }

    fn at(error: ResolveError, field: &'static str) -> ResolveError {
        error.at(field)
    }

    fn at_index(error: ResolveError, index: usize) -> ResolveError {
        error.at_index(index)
    }
}

impl TextMap<Template<Named>, ItemHover> for ItemHoverMap {
    type Error = ResolveError;

    fn item(&self, item: &Template<Named>) -> Result<ItemHover, ResolveError> {
        convert(item)
    }

    fn entity(
        &self,
        entity: &ResourceKeyBuf<EntityType>,
    ) -> Result<ResourceLocationBuf, ResolveError> {
        Ok(entity.location().clone())
    }

    fn dialog(
        &self,
        dialog: &DialogRef<Named>,
    ) -> Result<mcrs_minecraft_text::DialogRef, ResolveError> {
        convert(dialog)
    }

    fn at(error: ResolveError, field: &'static str) -> ResolveError {
        error.at(field)
    }

    fn at_index(error: ResolveError, index: usize) -> ResolveError {
        error.at_index(index)
    }
}
