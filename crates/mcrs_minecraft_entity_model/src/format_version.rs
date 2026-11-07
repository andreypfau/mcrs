use core::cmp::Ordering;
use core::fmt;
use core::str::FromStr;

const PARTS: usize = 4;

/// A `format_version` of a geometry file: numbers with dots, compared number by number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FormatVersion {
    parts: [u32; PARTS],
}

/// The shape of a geometry file that its format version selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// `1.8.0` and `1.10.0`: geometries are keys of the root object.
    Legacy,
    /// `1.12.0` and higher: the array `minecraft:geometry`.
    Current,
}

/// The text is not numbers with dots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidFormatVersion;

impl fmt::Display for InvalidFormatVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a format version is numbers with dots")
    }
}

impl core::error::Error for InvalidFormatVersion {}

impl FormatVersion {
    /// The highest version that the geometry rules know.
    pub const NEWEST_KNOWN: FormatVersion = FormatVersion::new(1, 26, 10);
    pub const V1_8_0: FormatVersion = FormatVersion::new(1, 8, 0);
    pub const V1_10_0: FormatVersion = FormatVersion::new(1, 10, 0);
    pub const V1_12_0: FormatVersion = FormatVersion::new(1, 12, 0);

    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            parts: [major, minor, patch, 0],
        }
    }

    /// The shape of a file with this version, or `None` when no shape has this version.
    #[must_use]
    pub fn shape(self) -> Option<Shape> {
        if self == Self::V1_8_0 || self == Self::V1_10_0 {
            Some(Shape::Legacy)
        } else if self >= Self::V1_12_0 {
            Some(Shape::Current)
        } else {
            None
        }
    }
}

impl FromStr for FormatVersion {
    type Err = InvalidFormatVersion;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut parts = [0; PARTS];
        for (index, part) in text.split('.').enumerate() {
            let slot = parts.get_mut(index).ok_or(InvalidFormatVersion)?;
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(InvalidFormatVersion);
            }
            *slot = part.parse().map_err(|_| InvalidFormatVersion)?;
        }
        Ok(Self { parts })
    }
}

impl PartialOrd for FormatVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FormatVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parts.cmp(&other.parts)
    }
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, patch, extra] = self.parts;
        write!(formatter, "{major}.{minor}.{patch}")?;
        if extra != 0 {
            write!(formatter, ".{extra}")?;
        }
        Ok(())
    }
}
