use bevy::prelude::*;
use mcrs_minecraft_render::{DebugView, DebugViews, RenderPath, SelectedView};

/// A fullscreen program the lighting stage can run in place of the lit one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Variant {
    LightingTerm,
    Unlit,
    Albedo,
    Ao,
    Normals,
    BlockLight,
    SkyLight,
    Grid,
}

impl Variant {
    pub const ALL: [Self; 8] = [
        Self::LightingTerm,
        Self::Unlit,
        Self::Albedo,
        Self::Ao,
        Self::Normals,
        Self::BlockLight,
        Self::SkyLight,
        Self::Grid,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Lighting {
    Lit,
    Variant(Variant),
    Depth,
    Pyramid,
}

/// What each deferred stage draws for the selected view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Display {
    pub lighting: Lighting,
    pub wireframe: bool,
    pub forward: bool,
}

impl Display {
    pub const FINAL: Self = Self {
        lighting: Lighting::Lit,
        wireframe: false,
        forward: true,
    };

    const fn lighting(lighting: Lighting) -> Self {
        Self {
            lighting,
            wireframe: false,
            forward: false,
        }
    }
}

#[derive(Resource)]
pub(crate) struct DeferredViews {
    wireframe: DebugView,
    albedo: DebugView,
    ao: DebugView,
    normals: DebugView,
    block_light: DebugView,
    sky_light: DebugView,
    depth: DebugView,
    block_grid: DebugView,
    hiz: Option<DebugView>,
    lighting_term: DebugView,
    forward: DebugView,
}

impl DeferredViews {
    /// The pyramid exists only while occlusion is on.
    pub fn register(views: &mut DebugViews, occlusion: bool) -> Self {
        let mut register = |name| views.register(RenderPath::Deferred, name);
        Self {
            wireframe: register("wireframe"),
            albedo: register("albedo"),
            ao: register("AO"),
            normals: register("normals"),
            block_light: register("block GI"),
            sky_light: register("sky GI"),
            depth: register("depth"),
            block_grid: register("block grid"),
            hiz: occlusion.then(|| register("Hi-Z")),
            lighting_term: register("lighting term"),
            forward: register("forward translucents"),
        }
    }

    pub fn display(&self, selected: SelectedView) -> Display {
        let Some(view) = selected.0 else {
            return Display::FINAL;
        };
        let variant = |variant| Display::lighting(Lighting::Variant(variant));
        if view == self.wireframe {
            Display {
                wireframe: true,
                ..variant(Variant::Albedo)
            }
        } else if view == self.albedo {
            variant(Variant::Albedo)
        } else if view == self.ao {
            variant(Variant::Ao)
        } else if view == self.normals {
            variant(Variant::Normals)
        } else if view == self.block_light {
            variant(Variant::BlockLight)
        } else if view == self.sky_light {
            variant(Variant::SkyLight)
        } else if view == self.depth {
            Display::lighting(Lighting::Depth)
        } else if view == self.block_grid {
            variant(Variant::Grid)
        } else if Some(view) == self.hiz {
            Display::lighting(Lighting::Pyramid)
        } else if view == self.lighting_term {
            variant(Variant::LightingTerm)
        } else if view == self.forward {
            Display {
                forward: true,
                ..variant(Variant::Unlit)
            }
        } else {
            Display::FINAL
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cycle(views: &DebugViews) -> Vec<DebugView> {
        std::iter::successors(views.step(RenderPath::Deferred, None, false), |&view| {
            views.step(RenderPath::Deferred, Some(view), false)
        })
        .collect()
    }

    #[test]
    fn the_deferred_cycle_follows_the_stages_and_holds_the_pyramid_only_with_occlusion() {
        for occlusion in [true, false] {
            let mut views = DebugViews::default();
            views.register(RenderPath::Classic, "wireframe");
            DeferredViews::register(&mut views, occlusion);
            let names: Vec<_> = cycle(&views)
                .into_iter()
                .map(|view| views.name(view))
                .collect();
            let mut expected = vec![
                "wireframe",
                "albedo",
                "AO",
                "normals",
                "block GI",
                "sky GI",
                "depth",
                "block grid",
                "Hi-Z",
                "lighting term",
                "forward translucents",
            ];
            expected.retain(|&name| occlusion || name != "Hi-Z");
            assert_eq!(names, expected);
        }
    }

    #[test]
    fn only_final_shading_and_the_forward_view_draw_forward_on_the_deferred_path() {
        let mut views = DebugViews::default();
        let deferred = DeferredViews::register(&mut views, true);
        assert_eq!(deferred.display(SelectedView(None)), Display::FINAL);
        for view in cycle(&views) {
            let display = deferred.display(SelectedView(Some(view)));
            let name = views.name(view);
            assert_eq!(display.forward, name == "forward translucents", "{name}");
            assert_eq!(display.wireframe, name == "wireframe", "{name}");
            assert_ne!(display.lighting, Lighting::Lit, "{name}");
        }
    }
}
