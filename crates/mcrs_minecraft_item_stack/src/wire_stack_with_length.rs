use std::io::Write;

use alloc::vec::Vec;

use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;

use crate::ItemStack;
use crate::Kind;
use crate::NameLookup;
use crate::WireError;
use crate::wire::Entries;
use crate::wire::read_frame;
use crate::wire::write_frame;
use crate::wire_frame::WireFrame;
use crate::wire_frame::codec_error;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WireStackWithLength(WireFrame);

impl WireStackWithLength {
    pub const EMPTY: Self = Self(WireFrame::EMPTY);

    pub fn write<L: NameLookup>(stack: Option<&ItemStack>, names: &L) -> Result<Self, WireError> {
        let mut bytes = Vec::new();
        write_frame(stack, names, Entries::LengthPrefixed, &mut bytes)?;
        WireFrame::written(bytes).map(Self)
    }

    pub fn read<L: NameLookup>(&self, names: &L) -> Result<Option<ItemStack>, WireError> {
        self.0
            .read(|input| read_frame(input, names, Entries::LengthPrefixed))
    }

    #[must_use]
    pub fn count(&self) -> i32 {
        self.0.count()
    }

    #[must_use]
    pub fn kind(&self) -> Option<Kind> {
        self.0.kind()
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl Encode for WireStackWithLength {
    fn encode(&self, mut w: impl Write) -> mcrs_minecraft_wire::Result<()> {
        Ok(w.write_all(self.bytes())?)
    }
}

impl Decode<'_> for WireStackWithLength {
    fn decode(r: &mut &[u8]) -> mcrs_minecraft_wire::Result<Self> {
        WireFrame::scan(r, Entries::LengthPrefixed)
            .map(Self)
            .map_err(codec_error)
    }
}
