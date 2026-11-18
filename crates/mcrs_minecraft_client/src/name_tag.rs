use bevy::math::{DVec3, Rect, Vec2};

pub const SCALE: f32 = 0.025;
pub const MAX_DISTANCE_SQUARED: f64 = 64.0 * 64.0;
pub const CROUCHING_DISTANCE_SQUARED: f64 = 1024.0;
pub const ROW_HEIGHT: f32 = 9.0;
const LIFT: f64 = 0.5;

/// What CEREN-NAME-001 and CEREN-NAME-003 read for one entity in one frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct TagInputs {
    pub distance_squared: f64,
    pub crouching: bool,
    pub player: bool,
    pub named: bool,
    pub name_visible: bool,
    pub targeted: bool,
    pub camera_entity: bool,
    pub hidden: bool,
    pub has_passenger: bool,
    pub hud_hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagDraw {
    /// One draw through blocks with the background, then one normal draw of the line.
    Through,
    /// One draw with depth, with the background.
    Crouching,
}

#[must_use]
pub fn tag_draw(inputs: &TagInputs) -> Option<TagDraw> {
    let shown = inputs.named
        && (inputs.player || inputs.name_visible || inputs.targeted)
        && inputs.distance_squared < MAX_DISTANCE_SQUARED
        && (!inputs.crouching || inputs.distance_squared < CROUCHING_DISTANCE_SQUARED)
        && !inputs.hud_hidden
        && !inputs.camera_entity
        && !inputs.hidden
        && !inputs.has_passenger;
    shown.then_some(if inputs.crouching {
        TagDraw::Crouching
    } else {
        TagDraw::Through
    })
}

#[must_use]
pub fn anchor(position: DVec3, box_height: f64) -> DVec3 {
    position + DVec3::new(0.0, box_height + LIFT, 0.0)
}

/// The rects of the tag in its own space: one unit is `SCALE` blocks, +X right, +Y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TagLayout {
    pub line_origin: Vec2,
    pub background: Rect,
}

#[must_use]
pub fn layout(width: f32) -> TagLayout {
    let x = -width / 2.0;
    TagLayout {
        line_origin: Vec2::new(x, 0.0),
        background: Rect::from_corners(
            Vec2::new(x - 1.0, -1.0),
            Vec2::new(x + width + 1.0, ROW_HEIGHT),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_shows_by_name_flag_distance_and_the_hiding_rules() {
        let base = TagInputs {
            distance_squared: 100.0,
            named: true,
            name_visible: true,
            ..TagInputs::default()
        };
        let through = Some(TagDraw::Through);
        let crouching = Some(TagDraw::Crouching);
        type Change = fn(&mut TagInputs);
        let cases: [(&str, Change, Option<TagDraw>); 15] = [
            ("a named mob with the flag", |_| {}, through),
            ("no name", |i| i.named = false, None),
            ("no flag and no target", |i| i.name_visible = false, None),
            (
                "the crosshair target without the flag",
                |i| {
                    i.name_visible = false;
                    i.targeted = true;
                },
                through,
            ),
            (
                "a player ignores the flag",
                |i| {
                    i.name_visible = false;
                    i.player = true;
                },
                through,
            ),
            (
                "a player with no name",
                |i| {
                    i.named = false;
                    i.player = true;
                },
                None,
            ),
            (
                "just inside 64 blocks",
                |i| i.distance_squared = 4095.9,
                through,
            ),
            ("at 64 blocks", |i| i.distance_squared = 4096.0, None),
            (
                "a non-crouching mob keeps the far range",
                |i| i.distance_squared = 2000.0,
                through,
            ),
            (
                "crouching just inside 32 blocks",
                |i| {
                    i.crouching = true;
                    i.distance_squared = 1023.9;
                },
                crouching,
            ),
            (
                "crouching at 32 blocks",
                |i| {
                    i.crouching = true;
                    i.distance_squared = 1024.0;
                },
                None,
            ),
            (
                "the camera entity",
                |i| {
                    i.camera_entity = true;
                    i.player = true;
                },
                None,
            ),
            ("hidden from the viewer", |i| i.hidden = true, None),
            ("a passenger on it", |i| i.has_passenger = true, None),
            ("the HUD hidden", |i| i.hud_hidden = true, None),
        ];
        for (label, change, expected) in cases {
            let mut inputs = base;
            change(&mut inputs);
            assert_eq!(tag_draw(&inputs), expected, "{label}");
        }
    }

    #[test]
    fn the_background_wraps_the_centred_line_and_the_anchor_lifts_half_a_block() {
        let even = layout(59.0);
        assert_eq!(
            even.line_origin,
            Vec2::new(-29.5, 0.0),
            "w / 2 is not rounded"
        );
        assert_eq!(even.background.min, Vec2::new(-30.5, -1.0));
        assert_eq!(even.background.max, Vec2::new(30.5, 9.0));
        assert!(
            (even.background.width() * SCALE - 1.525).abs() < 1e-6,
            "the CEREN-NAME-006 example"
        );
        let empty = layout(0.0);
        assert_eq!(empty.background.size(), Vec2::new(2.0, 10.0));
        let lifted = anchor(DVec3::new(1.0, 2.0, 3.0), 1.8);
        assert!(lifted.distance(DVec3::new(1.0, 4.3, 3.0)) < 1e-9);
    }
}
