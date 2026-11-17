use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FigureFrame {
    pub clip_from_figure: Mat4,
    pub lights: [Vec3; 2],
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct FigureView(pub Option<FigureFrame>);
