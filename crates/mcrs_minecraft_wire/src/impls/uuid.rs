use std::io::Write;

use uuid::Uuid;

use crate::{Decode, Encode, Result};

impl Encode for Uuid {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.as_u128().encode(w)
    }
}

impl Decode<'_> for Uuid {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        u128::decode(r).map(Uuid::from_u128)
    }
}
