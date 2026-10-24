use mcrs_minecraft_registries::Wire;
use std::io::Write;

use crate::item::component::common::MobEffectInstance;
use crate::item::component::potion::*;
use crate::{Decode, Encode};

impl Encode for PotionContents<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.potion.encode(&mut w)?;
        self.custom_color.encode(&mut w)?;
        self.visible_effects().encode(&mut w)?;
        self.custom_name.encode(w)
    }
}

impl Decode<'_> for PotionContents<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let potion = Option::decode(r)?;
        let custom_color = Option::decode(r)?;
        let mut custom_effects: Vec<MobEffectInstance<Wire>> = Vec::decode(r)?;
        strip_hidden(&mut custom_effects);
        Ok(PotionContents {
            potion,
            custom_color,
            custom_effects,
            custom_name: Option::decode(r)?,
        })
    }
}
