use mcrs_minecraft_registries::Wire;
use std::io::Write;

use crate::item::component::nested::*;
use crate::item::stack::Template;
use crate::item::wire::{newtype_wire, record_wire};
use crate::{Decode, Encode};

newtype_wire!(
    UseRemainder<Wire>,
    SulfurCubeContent<Wire>,
    BundleContents<Wire>
);
record_wire! {
    ChargedProjectiles<Wire> { items },
    Container<Wire> { slots },
}

impl Encode for PotDecorations<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        for side in self.sides() {
            match side {
                Some(template) => {
                    true.encode(&mut w)?;
                    template.encode(&mut w)?;
                }
                None => false.encode(&mut w)?,
            }
        }
        Ok(())
    }
}

impl<'a> Decode<'a> for PotDecorations<Wire> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let mut side = || Option::<Template<Wire>>::decode(r).map(|t| t.map(Box::new));
        Ok(PotDecorations {
            back: side()?,
            left: side()?,
            right: side()?,
            front: side()?,
        })
    }
}
