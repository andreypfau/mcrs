use derive_more::Deref;
use mcrs_minecraft_protocol::{Decode, Encode, Result};
use std::fmt;
use std::io::Write;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deref)]
pub struct ByteAngle(pub u8);

impl ByteAngle {
    pub fn from_degrees(degrees: f32) -> ByteAngle {
        ByteAngle((degrees * 256.0 / 360.0).floor() as i32 as u8)
    }

    pub fn to_degrees(self) -> f32 {
        f32::from(self.0 as i8) / 256.0 * 360.0
    }
}

impl fmt::Debug for ByteAngle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for ByteAngle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}°", self.to_degrees())
    }
}

impl Encode for ByteAngle {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.0.encode(w)
    }
}

impl Decode<'_> for ByteAngle {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        u8::decode(r).map(ByteAngle)
    }
}
