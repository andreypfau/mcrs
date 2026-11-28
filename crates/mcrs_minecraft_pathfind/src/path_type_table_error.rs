use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathTypeTableError {
    pub collision: usize,
    pub flags: usize,
}

impl fmt::Display for PathTypeTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "collision table has {} states but flags have {} entries",
            self.collision, self.flags
        )
    }
}

impl core::error::Error for PathTypeTableError {}
