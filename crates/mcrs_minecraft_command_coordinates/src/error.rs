use mcrs_minecraft_command::ErrorText;
use mcrs_minecraft_protocol::text::Text;

/// The errors of the coordinate, rotation and swizzle arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinatesError {
    PosMixed,
    PosMissingDouble,
    Pos3dIncomplete,
    RotationIncomplete,
    PosMissingInt,
    SwizzleInvalid,
}

impl ErrorText for CoordinatesError {
    fn text(&self, _: &str) -> Text {
        use CoordinatesError as E;
        match *self {
            E::PosMixed => Text::translate("argument.pos.mixed", vec![]),
            E::PosMissingDouble => Text::translate("argument.pos.missing.double", vec![]),
            E::Pos3dIncomplete => Text::translate("argument.pos3d.incomplete", vec![]),
            E::RotationIncomplete => Text::translate("argument.rotation.incomplete", vec![]),
            E::PosMissingInt => Text::translate("argument.pos.missing.int", vec![]),
            E::SwizzleInvalid => Text::translate("arguments.swizzle.invalid", vec![]),
        }
    }
}
