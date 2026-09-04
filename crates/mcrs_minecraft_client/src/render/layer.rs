use bevy::render::render_resource::BlendState;

use crate::blocks::Pass;
use crate::mesh::{stream_is_model, stream_pass};

impl Pass {
    pub const ALL: [Pass; Pass::COUNT] = [Pass::Solid, Pass::Cutout, Pass::Translucent];

    pub const fn label(self) -> &'static str {
        match self {
            Pass::Solid => "solid",
            Pass::Cutout => "cutout",
            Pass::Translucent => "translucent",
        }
    }

    pub const fn translucent(self) -> bool {
        matches!(self, Pass::Translucent)
    }

    pub const fn writes_depth(self) -> bool {
        !self.translucent()
    }

    pub const fn blend(self) -> Option<BlendState> {
        match self {
            Pass::Translucent => Some(BlendState::ALPHA_BLENDING),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum LayerGroup {
    Opaque,
    Translucent,
}

impl LayerGroup {
    pub const ALL: [LayerGroup; 2] = [LayerGroup::Opaque, LayerGroup::Translucent];

    // Blending is not commutative, so translucent draws have to reach the rasteriser in the order
    // the list holds them; opaque ones may be compacted.
    pub const fn culls_in_order(self) -> bool {
        matches!(self, LayerGroup::Translucent)
    }

    pub fn holds(self, stream: u32) -> bool {
        stream_pass(stream).translucent() == matches!(self, LayerGroup::Translucent)
    }

    pub fn of(stream: u32) -> Self {
        if stream_pass(stream).translucent() {
            LayerGroup::Translucent
        } else {
            LayerGroup::Opaque
        }
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
    use crate::mesh::{STREAM_NAMES, STREAMS};

    #[test]
    fn a_stream_is_named_after_the_layer_and_shape_it_stands_for() {
        for stream in 0..STREAMS as u32 {
            let name = format!(
                "{} {}",
                stream_pass(stream).label(),
                Shape::of_stream(stream).label()
            );
            assert_eq!(name, STREAM_NAMES[stream as usize]);
        }
    }

    #[test]
    fn the_two_groups_between_them_hold_every_stream_exactly_once() {
        for stream in 0..STREAMS as u32 {
            let held = LayerGroup::ALL
                .into_iter()
                .filter(|group| group.holds(stream))
                .count();
            assert_eq!(held, 1, "stream {stream} is in {held} groups");
        }
    }
}
