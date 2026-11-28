use bevy::prelude::*;
use mcrs_minecraft_menu_geometry::CreativeTabKind;
use mcrs_minecraft_pose::Dimensions;

const FEET_DROP_BLOCKS: f32 = 0.0625;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigurePlacement {
    pub(crate) rect: IRect,
    pub(crate) units_per_block: f32,
}

impl FigurePlacement {
    pub(crate) const SURVIVAL: Self = Self {
        rect: IRect {
            min: IVec2::new(26, 8),
            max: IVec2::new(75, 78),
        },
        units_per_block: 30.0,
    };

    pub(crate) const CREATIVE: Self = Self {
        rect: IRect {
            min: IVec2::new(73, 6),
            max: IVec2::new(105, 49),
        },
        units_per_block: 20.0,
    };

    #[must_use]
    pub(crate) fn for_creative_tab(kind: CreativeTabKind) -> Option<Self> {
        (kind == CreativeTabKind::SurvivalInventory).then_some(Self::CREATIVE)
    }

    #[must_use]
    pub(crate) fn rect_at(self, origin: IVec2) -> IRect {
        IRect::from_corners(origin + self.rect.min, origin + self.rect.max)
    }

    #[must_use]
    pub(crate) fn points(self, origin: IVec2, dimensions: Dimensions) -> FigurePoints {
        let centre = self.rect_at(origin).as_rect().center();
        let feet =
            centre + Vec2::Y * (dimensions.height / 2.0 + FEET_DROP_BLOCKS) * self.units_per_block;
        let eye = feet - Vec2::Y * dimensions.eye_height * self.units_per_block;
        FigurePoints { feet, eye }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FigurePoints {
    pub(crate) feet: Vec2,
    pub(crate) eye: Vec2,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_pose::Pose;

    const ORIGIN: IVec2 = IVec2::new(125, 37);

    #[test]
    fn placement_matches_the_inventory_of_an_854_by_480_window() {
        assert_eq!(
            FigurePlacement::SURVIVAL.rect_at(ORIGIN),
            IRect::new(151, 45, 200, 115),
            "the figure rectangle of the experiment"
        );

        let survival = FigurePlacement::SURVIVAL;
        let creative = FigurePlacement::CREATIVE;
        let poses = [
            (survival, Pose::Standing, 50.5, 71.875, 23.275),
            (survival, Pose::Crouching, 50.5, 67.375, 29.275),
            (creative, Pose::Standing, 89.0, 46.75, 14.35),
            (creative, Pose::Crouching, 89.0, 43.75, 18.35),
        ];
        for (placement, pose, centre_x, feet_y, eye_y) in poses {
            let points = placement.points(ORIGIN, pose.player_dimensions());
            let expected = FigurePoints {
                feet: ORIGIN.as_vec2() + Vec2::new(centre_x, feet_y),
                eye: ORIGIN.as_vec2() + Vec2::new(centre_x, eye_y),
            };
            assert!(
                points.feet.abs_diff_eq(expected.feet, 1e-4)
                    && points.eye.abs_diff_eq(expected.eye, 1e-4),
                "{placement:?} {pose:?}: {points:?} is not {expected:?}"
            );
        }
    }
}
