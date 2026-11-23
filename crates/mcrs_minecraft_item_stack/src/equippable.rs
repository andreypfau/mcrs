use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::EntityTypeSet;
use crate::EquippableSlot;
use crate::SoundEventRef;
use crate::entity_form::EntityForm;
use crate::identifier::Identifier;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Equippable {
    pub slot: EquippableSlot,
    pub equip_sound: Option<SoundEventRef>,
    asset_id: Option<Identifier>,
    camera_overlay: Option<Identifier>,
    pub allowed_entities: Option<EntityTypeSet>,
    pub dispensable: bool,
    pub swappable: bool,
    pub damage_on_hurt: bool,
    pub equip_on_interact: bool,
    pub can_be_sheared: bool,
    pub shearing_sound: Option<SoundEventRef>,
}

impl Equippable {
    #[must_use]
    pub const fn new(slot: EquippableSlot) -> Self {
        Self {
            slot,
            equip_sound: None,
            asset_id: None,
            camera_overlay: None,
            allowed_entities: None,
            dispensable: true,
            swappable: true,
            damage_on_hurt: true,
            equip_on_interact: false,
            can_be_sheared: false,
            shearing_sound: None,
        }
    }

    #[must_use]
    pub fn asset_id(&self) -> Option<ResourceLocation<'_>> {
        self.asset_id.as_ref().map(Identifier::as_location)
    }

    pub fn set_asset_id(&mut self, asset_id: Option<ResourceLocationBuf>) {
        self.asset_id = asset_id.map(Identifier::Owned);
    }

    #[must_use]
    pub fn camera_overlay(&self) -> Option<ResourceLocation<'_>> {
        self.camera_overlay.as_ref().map(Identifier::as_location)
    }

    pub fn set_camera_overlay(&mut self, camera_overlay: Option<ResourceLocationBuf>) {
        self.camera_overlay = camera_overlay.map(Identifier::Owned);
    }
}

impl EntityForm for Equippable {
    fn borrow_static(&'static self) -> Self {
        Self {
            slot: self.slot,
            equip_sound: self.equip_sound.as_ref().map(SoundEventRef::borrow_static),
            asset_id: self.asset_id.as_ref().map(Identifier::borrow_static),
            camera_overlay: self.camera_overlay.as_ref().map(Identifier::borrow_static),
            allowed_entities: self
                .allowed_entities
                .as_ref()
                .map(EntityTypeSet::borrow_static),
            shearing_sound: self
                .shearing_sound
                .as_ref()
                .map(SoundEventRef::borrow_static),
            ..*self
        }
    }
}
