#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestPurpose {
    Click = 0,
    HandDrop = 1,
    HandSwap = 2,
    Close = 3,
    Creative = 4,
}

impl RequestPurpose {
    #[must_use]
    pub const fn serial(self) -> u32 {
        self as u32
    }

    #[must_use]
    pub fn from_serial(serial: u32) -> Option<Self> {
        [
            Self::Click,
            Self::HandDrop,
            Self::HandSwap,
            Self::Close,
            Self::Creative,
        ]
        .into_iter()
        .find(|purpose| purpose.serial() == serial)
    }
}
