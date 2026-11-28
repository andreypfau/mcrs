use alloc::borrow::Cow;
use alloc::vec::Vec;

use mcrs_minecraft_wire::ErrorKind;

use crate::Kind;
use crate::WireError;
use crate::wire::Entries;
use crate::wire_scan::Head;
use crate::wire_scan::read_head;
use crate::wire_scan::scan_stack;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WireFrame {
    bytes: Cow<'static, [u8]>,
    count: i32,
    item: i32,
}

impl WireFrame {
    pub(crate) const EMPTY: Self = Self {
        bytes: Cow::Borrowed(&[0]),
        count: 0,
        item: 0,
    };

    pub(crate) fn scan(input: &mut &[u8], entries: Entries) -> Result<Self, WireError> {
        let start = *input;
        let head = scan_stack(input, entries)?;
        let length = start.len().saturating_sub(input.len());
        let bytes = start.get(..length).ok_or(WireError::Malformed)?;
        Ok(Self::new(head, bytes.to_vec()))
    }

    pub(crate) fn written(bytes: Vec<u8>) -> Result<Self, WireError> {
        let head = read_head(&mut bytes.as_slice())?;
        Ok(Self::new(head, bytes))
    }

    fn new(head: Head, bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cow::Owned(bytes),
            count: head.count,
            item: head.item,
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn count(&self) -> i32 {
        self.count
    }

    pub(crate) fn kind(&self) -> Option<Kind> {
        if self.count <= 0 {
            return None;
        }
        u16::try_from(self.item).ok().map(Kind)
    }

    pub(crate) fn read<T>(
        &self,
        read: impl FnOnce(&mut &[u8]) -> Result<T, WireError>,
    ) -> Result<T, WireError> {
        let mut input = self.bytes();
        let value = read(&mut input)?;
        if !input.is_empty() {
            return Err(WireError::Malformed);
        }
        Ok(value)
    }
}

pub(crate) fn codec_error(error: WireError) -> mcrs_minecraft_wire::Error {
    match error {
        WireError::UnexpectedEnd => ErrorKind::UnexpectedEnd.into(),
        other => mcrs_minecraft_wire::Error::other(other),
    }
}
