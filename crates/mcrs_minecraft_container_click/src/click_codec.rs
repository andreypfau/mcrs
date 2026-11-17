use std::io::Write;

use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::Error;
use mcrs_minecraft_wire::Result;

use crate::Click;
use crate::ClickKind;
use crate::Clicked;

impl Encode for Click {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        i16::try_from(self.clicked)
            .map_err(Error::other)?
            .encode(&mut w)?;
        self.button.encode(&mut w)?;
        self.kind.encode(w)
    }
}

impl Decode<'_> for Click {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let clicked = Clicked::from(i16::decode(r)?);
        let button = u8::decode(r)?;
        let kind = ClickKind::decode(r)?;
        Ok(Self {
            kind,
            button,
            clicked,
        })
    }
}
