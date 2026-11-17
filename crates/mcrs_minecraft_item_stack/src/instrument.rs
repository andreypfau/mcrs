use crate::InlineInstrument;
use crate::InstrumentId;
use crate::RegistryValue;

#[derive(Clone, Debug, PartialEq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Instrument(pub RegistryValue<InstrumentId, InlineInstrument>);
