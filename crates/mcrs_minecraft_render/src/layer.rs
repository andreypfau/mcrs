use bevy::render::render_resource::BlendState;

use mcrs_minecraft_mesh::DrawKind;
use mcrs_minecraft_mesh::block::Pass;

pub const fn blend(pass: Pass) -> Option<BlendState> {
    match pass {
        Pass::Translucent => Some(BlendState::ALPHA_BLENDING),
        _ => None,
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LayerGroup {
    Opaque,
    Translucent,
}

impl LayerGroup {
    pub(crate) const ALL: [LayerGroup; 2] = [LayerGroup::Opaque, LayerGroup::Translucent];

    pub(crate) const fn holds(self, kind: DrawKind) -> bool {
        kind.translucent() == matches!(self, LayerGroup::Translucent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_draw_kind_is_named_after_its_layer_and_format_and_sits_in_one_group() {
        for kind in DrawKind::ALL {
            let name = format!("{} {}", kind.layer.label(), kind.format.label());
            assert_eq!(name, kind.label());
            let held = LayerGroup::ALL
                .into_iter()
                .filter(|group| group.holds(kind))
                .count();
            assert_eq!(held, 1, "{} is in {held} groups", kind.label());
        }
    }
}
