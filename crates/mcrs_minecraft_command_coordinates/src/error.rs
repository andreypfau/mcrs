use core::fmt;
use mcrs_minecraft_command::ErrorText;
use mcrs_minecraft_protocol::text::Text;

/// The errors of the coordinate, rotation and swizzle arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoordinatesError {
    /// Local `^` and world coordinates are mixed.
    PosMixed,
    /// A decimal coordinate is missing.
    PosMissingDouble,
    /// A position has fewer than three coordinates.
    Pos3dIncomplete,
    /// A rotation has fewer than two angles.
    RotationIncomplete,
    /// An integer coordinate is missing.
    PosMissingInt,
    /// A swizzle holds a letter other than `x`, `y` or `z`, or one twice.
    SwizzleInvalid,
}

impl fmt::Display for CoordinatesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CoordinatesError::PosMixed => "local and world coordinates are mixed",
            CoordinatesError::PosMissingDouble => "missing coordinate",
            CoordinatesError::Pos3dIncomplete => "incomplete position",
            CoordinatesError::RotationIncomplete => "incomplete rotation",
            CoordinatesError::PosMissingInt => "missing block coordinate",
            CoordinatesError::SwizzleInvalid => "invalid swizzle",
        })
    }
}

impl core::error::Error for CoordinatesError {}

impl ErrorText for CoordinatesError {
    fn text(&self, _: &str) -> Text {
        use CoordinatesError::*;
        match *self {
            PosMixed => Text::translate("argument.pos.mixed", vec![]),
            PosMissingDouble => Text::translate("argument.pos.missing.double", vec![]),
            Pos3dIncomplete => Text::translate("argument.pos3d.incomplete", vec![]),
            RotationIncomplete => Text::translate("argument.rotation.incomplete", vec![]),
            PosMissingInt => Text::translate("argument.pos.missing.int", vec![]),
            SwizzleInvalid => Text::translate("arguments.swizzle.invalid", vec![]),
        }
    }
}
