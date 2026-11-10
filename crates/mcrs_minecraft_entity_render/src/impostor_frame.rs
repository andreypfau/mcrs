use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Vec3};

/// The yaw views of an impostor image set, one each `360 / IMPOSTOR_YAWS` degrees.
pub const IMPOSTOR_YAWS: u32 = 16;
/// The elevation views of an impostor image set, in degrees.
///
/// A far mob is seen from a low elevation, so the steps are finest there: an image of a step
/// away shows more or less of the top faces, which are brighter than the sides.
pub const IMPOSTOR_ELEVATIONS: [f32; 9] = [0.0, 5.0, 10.0, 15.0, 20.0, 30.0, 40.0, 50.0, 60.0];
/// The views of an image set: the yaws of the first elevation, then of the next.
pub const IMPOSTOR_VIEWS: usize = IMPOSTOR_YAWS as usize * IMPOSTOR_ELEVATIONS.len();
/// The projected height in pixels under which an impostor draws its whole quad, with no discard
/// of the pixels that its image does not cover. A tile-based GPU removes the hidden fragments of
/// such a draw, and the far impostors of a field overlap many times.
pub const IMPOSTOR_SMALL_HEIGHT: f32 = 3.0;
/// The side of one impostor image in texels.
pub const IMPOSTOR_TILE: u32 = 64;

const MARGIN: f32 = 0.02;
/// The height of the entity origin above the frame position, as the pose pass places it.
const ORIGIN_LIFT: f32 = 0.001;

/// The rectangle of the quad of one view, in blocks at the scale 1. The quad stands on the
/// vertical plane through the entity position that faces the camera. Its image is the
/// projection of the model onto that plane along the direction of the view, so a pixel of the
/// quad shows the point that the camera sees through it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ImpostorView {
    /// The edges across the view, positive to the right of the camera.
    pub left: f32,
    pub right: f32,
    /// The edges above the entity position.
    pub low: f32,
    pub high: f32,
}

/// The views of the impostor images of one definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpostorFrames {
    /// One rectangle for each view, in the order of [`IMPOSTOR_VIEWS`].
    pub views: [ImpostorView; IMPOSTOR_VIEWS],
    /// The largest distance of a point of the model from the entity position.
    pub depth_radius: f32,
}

impl Default for ImpostorFrames {
    fn default() -> Self {
        Self {
            views: [ImpostorView::default(); IMPOSTOR_VIEWS],
            depth_radius: 0.0,
        }
    }
}

impl ImpostorFrames {
    /// The views that hold `points`, which are in entity space at the body yaw 0: blocks,
    /// relative to the entity position.
    #[must_use]
    pub fn enclosing(points: &[Vec3]) -> Self {
        let mut frames = Self {
            depth_radius: points
                .iter()
                .map(|point| point.length())
                .fold(0.0f32, f32::max)
                + MARGIN,
            ..Self::default()
        };
        for (index, view) in frames.views.iter_mut().enumerate() {
            let axes = ViewAxes::of(index);
            let mut bounds = [
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ];
            for point in points {
                let across = point.dot(axes.right);
                let up = point.y - point.dot(axes.toward) * axes.slope;
                bounds = [
                    bounds[0].min(across),
                    bounds[1].max(across),
                    bounds[2].min(up),
                    bounds[3].max(up),
                ];
            }
            if bounds[0] > bounds[1] {
                bounds = [-0.5, 0.5, 0.0, 1.0];
            }
            *view = ImpostorView {
                left: bounds[0] - MARGIN,
                right: bounds[1] + MARGIN,
                low: bounds[2] - MARGIN,
                high: bounds[3] + MARGIN,
            };
        }
        frames
    }

    /// The rows of the affine map from a model point after its bone transform, in model units,
    /// to the clip space of the image of the view `index`. The depth is larger for a point
    /// nearer to the camera, in `[0, 1]`.
    #[must_use]
    pub fn bake_rows(&self, index: usize) -> [[f32; 4]; 3] {
        let axes = ViewAxes::of(index);
        let view = self.views.get(index).copied().unwrap_or_default();
        let centre_across = (view.left + view.right) / 2.0;
        let half_across = ((view.right - view.left) / 2.0).max(f32::MIN_POSITIVE);
        let centre_up = (view.low + view.high) / 2.0;
        let half_up = ((view.high - view.low) / 2.0).max(f32::MIN_POSITIVE);
        let depth = self.depth_radius.max(f32::MIN_POSITIVE);
        let rows = [
            (axes.right / half_across, -centre_across / half_across),
            (
                (Vec3::Y - axes.toward * axes.slope) / half_up,
                -centre_up / half_up,
            ),
            (
                (axes.toward * axes.cosine + Vec3::Y * axes.sine) / (2.0 * depth),
                0.5,
            ),
        ];
        let linear = entity_from_model();
        let lift = Vec3::Y * ORIGIN_LIFT;
        rows.map(|(row, constant)| {
            let over_model = linear.transpose() * row;
            [
                over_model.x,
                over_model.y,
                over_model.z,
                row.dot(lift) + constant,
            ]
        })
    }
}

/// The directions of a view in entity space at the body yaw 0.
struct ViewAxes {
    /// The horizontal direction from the entity to the camera.
    toward: Vec3,
    right: Vec3,
    sine: f32,
    cosine: f32,
    /// The tangent of the elevation.
    slope: f32,
}

impl ViewAxes {
    fn of(index: usize) -> Self {
        let yaw = (index % IMPOSTOR_YAWS as usize) as f32;
        let elevation = IMPOSTOR_ELEVATIONS
            .get(index / IMPOSTOR_YAWS as usize)
            .copied()
            .unwrap_or(0.0);
        let angle = (yaw * 360.0 / IMPOSTOR_YAWS as f32).to_radians();
        let toward = Vec3::new(angle.sin(), 0.0, angle.cos());
        let (sine, cosine) = elevation.to_radians().sin_cos();
        Self {
            toward,
            right: Vec3::new(toward.z, 0.0, -toward.x),
            sine,
            cosine,
            slope: sine / cosine,
        }
    }
}

/// The entity-space point of a model point at the body yaw 0 and the scale 1.
#[must_use]
pub fn entity_point(model: Vec3) -> Vec3 {
    entity_from_model() * model + Vec3::Y * ORIGIN_LIFT
}

/// The model turns by `180° − body yaw` about the vertical axis, and a block is 16 model units.
fn entity_from_model() -> Mat3 {
    Mat3::from_rotation_y(std::f32::consts::PI) * (1.0 / 16.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_points_fill_the_image_of_each_view() {
        let corners: Vec<Vec3> = (0..8)
            .map(|corner| {
                Vec3::new(
                    if corner & 1 == 0 { -5.0 } else { 3.0 },
                    if corner & 2 == 0 { 0.0 } else { 14.0 },
                    if corner & 4 == 0 { -9.0 } else { 7.0 },
                )
            })
            .collect();
        let points: Vec<Vec3> = corners.iter().map(|corner| entity_point(*corner)).collect();
        let frames = ImpostorFrames::enclosing(&points);
        for index in 0..IMPOSTOR_VIEWS {
            let rows = frames.bake_rows(index);
            let mut extent = [1.0f32, -1.0, 1.0, -1.0];
            for corner in &corners {
                let clip = rows
                    .map(|row| row[0] * corner.x + row[1] * corner.y + row[2] * corner.z + row[3]);
                assert!(
                    clip[0].abs() < 1.0 && clip[1].abs() < 1.0,
                    "the view holds each point: {clip:?} in view {index}"
                );
                assert!(
                    (0.0..=1.0).contains(&clip[2]),
                    "the depth stays in the target: {clip:?}"
                );
                extent = [
                    extent[0].min(clip[0]),
                    extent[1].max(clip[0]),
                    extent[2].min(clip[1]),
                    extent[3].max(clip[1]),
                ];
            }
            assert!(
                extent[0] < -0.9 && extent[1] > 0.9 && extent[2] < -0.9 && extent[3] > 0.9,
                "the points fill the image of view {index}: {extent:?}"
            );
        }
    }
}
