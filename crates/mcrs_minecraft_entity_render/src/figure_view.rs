use bevy::prelude::*;

/// The projection and the light of the figure view in one frame. Figure space has the unit of
/// one block, and the figure stands at its origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FigureFrame {
    /// The parallel projection from figure space to the clip space of the target of the host.
    pub clip_from_figure: Mat4,
    /// The two light directions of the face factor, in figure space.
    pub lights: [Vec3; 2],
}

/// The figure frame of this frame, or `None` while no host draws a figure. A resource of the
/// main world that the host writes; with `None`, no pass of the figure view runs.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct FigureView(pub Option<FigureFrame>);
