use mcrs_minecraft_core::{Direction, Mirror, Rotation};
use mcrs_minecraft_worldgen_feature::template::PaletteState;

use super::Turn;

pub(super) fn direction(name: &str) -> Option<Direction> {
    Direction::all().into_iter().find(|d| d.name() == name)
}

impl Turn {
    /// A mirror across z is a mirror across x followed by a half turn.
    fn flip_and_quarters(self) -> (bool, i32) {
        let quarters = match self.rotation {
            Rotation::None => 0,
            Rotation::Clockwise90 => 1,
            Rotation::Clockwise180 => 2,
            Rotation::Counterclockwise90 => 3,
        };
        match self.mirror {
            Mirror::None => (false, quarters),
            Mirror::FrontBack => (true, quarters),
            Mirror::LeftRight => (true, (quarters + 2) % 4),
        }
    }

    pub fn direction(self, direction: Direction) -> Direction {
        self.rotation.rotate(self.mirror.mirror(direction))
    }

    fn side(self, name: &str) -> String {
        direction(name)
            .map_or(name, |d| self.direction(d).name())
            .to_owned()
    }
}

fn swap_left_right(value: &str) -> String {
    value
        .replace("left", "\0")
        .replace("right", "left")
        .replace('\0', "right")
}

// chisle: `rotate_state` and `mirror_state` in the feature placement turn
// numeric ids through `WorldStates` and keep the game's own handling of
// mirrored stairs and `type`; this is the plain geometric turn the blueprints
// rely on. Turning by property tables shared with the block definitions lifts it.
pub fn turned(state: &PaletteState, turn: Turn) -> PaletteState {
    let Some(properties) = &state.properties else {
        return state.clone();
    };
    if turn == Turn::NONE {
        return state.clone();
    }
    let (flip, quarters) = turn.flip_and_quarters();
    let name = state.id.path();
    let turned = properties
        .iter()
        .map(|(key, value)| match key.as_str() {
            "north" | "south" | "east" | "west" => (turn.side(key), value.clone()),
            "facing" => (key.clone(), turn.side(value)),
            "axis" if quarters % 2 == 1 => {
                let swapped = match value.as_str() {
                    "x" => "z",
                    "z" => "x",
                    other => other,
                };
                (key.clone(), swapped.to_owned())
            }
            "hinge" | "type" if flip && matches!(value.as_str(), "left" | "right") => {
                (key.clone(), swap_left_right(value))
            }
            "shape" if name.ends_with("_stairs") && flip => (key.clone(), swap_left_right(value)),
            "rotation" => {
                let rotation: i32 = value.parse().expect("a sixteenth turn");
                let rotation = if flip { (16 - rotation) % 16 } else { rotation };
                (key.clone(), ((rotation + 4 * quarters) % 16).to_string())
            }
            "orientation" => {
                let parts: Vec<String> = value.split('_').map(|part| turn.side(part)).collect();
                (key.clone(), parts.join("_"))
            }
            "shape" if name.contains("rail") => {
                let mut parts: Vec<String> = value.split('_').map(|part| turn.side(part)).collect();
                if parts[0] != "ascending" {
                    parts.sort_by_key(|side| side == "east" || side == "west");
                }
                if parts == ["south", "north"] {
                    parts.reverse();
                }
                if parts == ["west", "east"] {
                    parts.reverse();
                }
                (key.clone(), parts.join("_"))
            }
            _ => (key.clone(), value.clone()),
        })
        .collect();
    PaletteState {
        id: state.id.clone(),
        properties: Some(turned),
    }
}
