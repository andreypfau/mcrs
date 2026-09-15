use serde::{Deserialize, Serialize};

use crate::rotation::{legacy_deserialize, legacy_serialize};
use crate::{Axis, Direction, Rotation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mirror {
    None,
    LeftRight,
    FrontBack,
}

impl Mirror {
    pub const ALL: [Mirror; 3] = [Mirror::None, Mirror::LeftRight, Mirror::FrontBack];

    pub const fn legacy_name(self) -> &'static str {
        match self {
            Mirror::None => "NONE",
            Mirror::LeftRight => "LEFT_RIGHT",
            Mirror::FrontBack => "FRONT_BACK",
        }
    }

    /// The axis whose sign this mirror flips.
    pub const fn axis(self) -> Option<Axis> {
        match self {
            Mirror::None => None,
            Mirror::LeftRight => Some(Axis::Z),
            Mirror::FrontBack => Some(Axis::X),
        }
    }

    pub fn mirror(self, direction: Direction) -> Direction {
        if self.axis() == Some(direction.axis()) {
            direction.opposite()
        } else {
            direction
        }
    }

    /// The rotation a block facing `direction` takes under this mirror: a half
    /// turn when the facing lies on the mirrored axis, nothing otherwise.
    pub fn rotation(self, direction: Direction) -> Rotation {
        if self.axis() == Some(direction.axis()) {
            Rotation::Clockwise180
        } else {
            Rotation::None
        }
    }

    /// A block rotation of `steps` per turn reflected across the mirror.
    pub fn mirror_index(self, index: u16, steps: u16) -> u16 {
        let (index, steps) = (i32::from(index), i32::from(steps));
        let half = steps / 2;
        let corrected = if index > half { index - steps } else { index };
        let mirrored = match self {
            Mirror::None => index,
            Mirror::LeftRight => (half - corrected + steps) % steps,
            Mirror::FrontBack => (steps - corrected) % steps,
        };
        mirrored as u16
    }
}

/// The `FRONT_BACK` spelling the piece NBT uses.
pub mod legacy {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        mirror: &Mirror,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        legacy_serialize(mirror.legacy_name(), serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Mirror, D::Error> {
        legacy_deserialize(deserializer, &Mirror::ALL, Mirror::legacy_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mirror_flips_the_faces_on_its_axis_only() {
        assert_eq!(Mirror::LeftRight.mirror(Direction::North), Direction::South);
        assert_eq!(Mirror::LeftRight.mirror(Direction::East), Direction::East);
        assert_eq!(Mirror::FrontBack.mirror(Direction::West), Direction::East);
        assert_eq!(Mirror::FrontBack.mirror(Direction::Up), Direction::Up);
        assert_eq!(Mirror::None.mirror(Direction::North), Direction::North);
        assert_eq!(
            Mirror::LeftRight.rotation(Direction::South),
            Rotation::Clockwise180
        );
        assert_eq!(Mirror::LeftRight.rotation(Direction::East), Rotation::None);
        assert_eq!(
            Mirror::FrontBack.rotation(Direction::East),
            Rotation::Clockwise180
        );
    }

    #[test]
    fn a_sixteen_step_rotation_reflects_across_the_axis() {
        let steps = 16;
        assert_eq!(Mirror::None.mirror_index(3, steps), 3);
        assert_eq!(Mirror::LeftRight.mirror_index(0, steps), 8);
        assert_eq!(Mirror::LeftRight.mirror_index(8, steps), 0);
        assert_eq!(Mirror::LeftRight.mirror_index(3, steps), 5);
        assert_eq!(Mirror::LeftRight.mirror_index(13, steps), 11);
        assert_eq!(Mirror::FrontBack.mirror_index(0, steps), 0);
        assert_eq!(Mirror::FrontBack.mirror_index(4, steps), 12);
        assert_eq!(Mirror::FrontBack.mirror_index(8, steps), 8);
        assert_eq!(Mirror::FrontBack.mirror_index(15, steps), 1);
    }

    #[test]
    fn json_and_legacy_names_round_trip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Legacy(#[serde(with = "legacy")] Mirror);

        for mirror in Mirror::ALL {
            let json = serde_json::to_string(&mirror).unwrap();
            assert_eq!(serde_json::from_str::<Mirror>(&json).unwrap(), mirror);
            let legacy = serde_json::to_string(&Legacy(mirror)).unwrap();
            assert_eq!(
                serde_json::from_str::<Legacy>(&legacy).unwrap(),
                Legacy(mirror)
            );
        }
        assert_eq!(
            serde_json::to_string(&Mirror::FrontBack).unwrap(),
            "\"front_back\""
        );
        assert_eq!(
            serde_json::to_string(&Legacy(Mirror::FrontBack)).unwrap(),
            "\"FRONT_BACK\""
        );
        assert!(serde_json::from_str::<Legacy>("\"front_back\"").is_err());
        assert!(serde_json::from_str::<Mirror>("\"FRONT_BACK\"").is_err());
    }
}
