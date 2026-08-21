use bevy::render::render_resource::BlendState;

use crate::blocks::Pass;
use crate::mesh::{stream_is_model, stream_pass};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Layer {
    Solid,
    Cutout,
    Translucent,
}

impl Layer {
    pub const ALL: [Layer; 3] = [Layer::Solid, Layer::Cutout, Layer::Translucent];

    pub fn of(pass: Pass) -> Self {
        match pass {
            Pass::Solid => Layer::Solid,
            Pass::Cutout => Layer::Cutout,
            Pass::Translucent => Layer::Translucent,
        }
    }

    pub fn of_stream(stream: u32) -> Self {
        Self::of(stream_pass(stream))
    }

    pub const fn label(self) -> &'static str {
        match self {
            Layer::Solid => "solid",
            Layer::Cutout => "cutout",
            Layer::Translucent => "translucent",
        }
    }

    pub const fn translucent(self) -> bool {
        matches!(self, Layer::Translucent)
    }

    pub const fn writes_depth(self) -> bool {
        !self.translucent()
    }

    pub const fn blend(self) -> Option<BlendState> {
        match self {
            Layer::Translucent => Some(BlendState::ALPHA_BLENDING),
            _ => None,
        }
    }

    // Wireframe draws by discarding the inside of every quad, and the solid pipeline has no
    // discard in it, so it borrows the cutout one for as long as wireframe is on.
    pub const fn drawn_as(self, wireframe: bool) -> Self {
        match self {
            Layer::Solid if wireframe => Layer::Cutout,
            other => other,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum LayerGroup {
    Opaque,
    Translucent,
}

impl LayerGroup {
    pub const fn layers(self) -> &'static [Layer] {
        match self {
            LayerGroup::Opaque => &[Layer::Solid, Layer::Cutout],
            LayerGroup::Translucent => &[Layer::Translucent],
        }
    }

    pub fn holds(self, stream: u32) -> bool {
        let layer = Layer::of_stream(stream);
        self.layers().contains(&layer)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Shape {
    Greedy,
    Model,
}

impl Shape {
    pub const ALL: [Shape; 2] = [Shape::Greedy, Shape::Model];

    pub fn of_stream(stream: u32) -> Self {
        if stream_is_model(stream) {
            Shape::Model
        } else {
            Shape::Greedy
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Shape::Greedy => "greedy",
            Shape::Model => "model",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{STREAMS, STREAM_NAMES};

    #[test]
    fn a_stream_is_named_after_the_layer_and_shape_it_stands_for() {
        for stream in 0..STREAMS as u32 {
            let name = format!(
                "{} {}",
                Layer::of_stream(stream).label(),
                Shape::of_stream(stream).label()
            );
            assert_eq!(name, STREAM_NAMES[stream as usize]);
        }
    }

    #[test]
    fn the_two_groups_between_them_hold_every_stream_exactly_once() {
        for stream in 0..STREAMS as u32 {
            let held = [LayerGroup::Opaque, LayerGroup::Translucent]
                .into_iter()
                .filter(|group| group.holds(stream))
                .count();
            assert_eq!(held, 1, "stream {stream} is in {held} groups");
        }
    }

    #[test]
    fn wireframe_moves_the_solid_layer_onto_a_pipeline_that_can_discard() {
        assert_eq!(Layer::Solid.drawn_as(false), Layer::Solid);
        assert_eq!(Layer::Solid.drawn_as(true), Layer::Cutout);
        assert_eq!(Layer::Translucent.drawn_as(true), Layer::Translucent);
    }
}
