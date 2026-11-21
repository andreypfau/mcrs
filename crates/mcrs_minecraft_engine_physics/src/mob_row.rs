use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_pose::Dimensions;

#[must_use]
pub fn mob_row(kind: &EntityTypeData) -> MotionRow {
    MotionRow::living(
        Category::Walker,
        Dimensions::scalable(kind.width, kind.height).with_eye_height(kind.eye_height),
    )
}
