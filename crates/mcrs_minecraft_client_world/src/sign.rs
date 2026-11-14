//! Which block states are signs, and how each one holds (`client-sign-text.md`, section 0.1).

use mcrs_minecraft_block::definition::Blocks;
use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignFamily {
    Standing,
    Hanging,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignMount {
    Ground,
    Wall,
    Ceiling,
}

/// A sign state: its family and mount, and the angle `A` of SIGN-PLACE-001 item 1 in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignBlock {
    pub family: SignFamily,
    pub mount: SignMount,
    pub angle: f32,
}

/// The family and mount of the block named `path`, with no namespace.
pub fn family_and_mount(path: &str) -> Option<(SignFamily, SignMount)> {
    if path.ends_with("_wall_hanging_sign") {
        Some((SignFamily::Hanging, SignMount::Wall))
    } else if path.ends_with("_hanging_sign") {
        Some((SignFamily::Hanging, SignMount::Ceiling))
    } else if path.ends_with("_wall_sign") {
        Some((SignFamily::Standing, SignMount::Wall))
    } else if path.ends_with("_sign") {
        Some((SignFamily::Standing, SignMount::Ground))
    } else {
        None
    }
}

/// The yaw of a horizontal direction: `south` 0, `west` 90, `north` 180, `east` 270.
pub fn yaw_of(facing: &str) -> Option<f32> {
    match facing {
        "south" => Some(0.0),
        "west" => Some(90.0),
        "north" => Some(180.0),
        "east" => Some(270.0),
        _ => None,
    }
}

/// `None` for a state that is no sign, or one with no readable `rotation` or `facing`.
pub fn sign_block(blocks: &Blocks, state: BlockStateId) -> Option<SignBlock> {
    let entry = blocks.owner(state)?;
    let (family, mount) = family_and_mount(entry.identifier.path())?;
    let text = |property: &str| Some(entry.value_of(state, property)?.to_text());
    let angle = match mount {
        SignMount::Wall => yaw_of(&text("facing")?)?,
        SignMount::Ground | SignMount::Ceiling => {
            text("rotation")?.parse::<u8>().ok()? as f32 * 22.5
        }
    };
    Some(SignBlock {
        family,
        mount,
        angle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_families_come_from_the_block_name() {
        use SignFamily::*;
        use SignMount::*;
        assert_eq!(family_and_mount("oak_sign"), Some((Standing, Ground)));
        assert_eq!(
            family_and_mount("pale_oak_wall_sign"),
            Some((Standing, Wall))
        );
        assert_eq!(
            family_and_mount("bamboo_hanging_sign"),
            Some((Hanging, Ceiling))
        );
        assert_eq!(
            family_and_mount("cherry_wall_hanging_sign"),
            Some((Hanging, Wall))
        );
        assert_eq!(family_and_mount("oak_planks"), None);
    }
}
