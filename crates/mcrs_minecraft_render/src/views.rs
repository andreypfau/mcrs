use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;

use crate::RenderPath;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DebugView(u16);

/// Every view a path can show in place of its final shading, in registration order.
#[derive(Resource, Default)]
pub struct DebugViews(Vec<(RenderPath, &'static str)>);

impl DebugViews {
    /// A path's views cycle in the order registered, so a crate registers each view where it
    /// belongs in its path's list.
    pub fn register(&mut self, path: RenderPath, name: &'static str) -> DebugView {
        let view = DebugView(u16::try_from(self.0.len()).expect("fewer than 65536 debug views"));
        self.0.push((path, name));
        view
    }

    pub fn name(&self, view: DebugView) -> &'static str {
        self.0[view.0 as usize].1
    }

    /// The view after `current` among `path`'s, or before it when `back`; final shading (`None`)
    /// closes the cycle, and a selection the path lacks counts as final shading.
    pub fn step(
        &self,
        path: RenderPath,
        current: Option<DebugView>,
        back: bool,
    ) -> Option<DebugView> {
        let cycle: Vec<Option<DebugView>> = (0..self.0.len() as u16)
            .filter(|&index| self.0[index as usize].0 == path)
            .map(|index| Some(DebugView(index)))
            .chain([None])
            .collect();
        let len = cycle.len();
        let at = cycle
            .iter()
            .position(|&view| view == current)
            .unwrap_or(len - 1);
        cycle[if back {
            (at + len - 1) % len
        } else {
            (at + 1) % len
        }]
    }
}

/// The view shown in place of final shading, or `None` for final shading.
#[derive(Resource, Clone, Copy, Default, PartialEq, Eq, Debug, ExtractResource)]
pub struct SelectedView(pub Option<DebugView>);

#[derive(Resource)]
pub(crate) struct ClassicViews {
    pub wireframe: DebugView,
    pub depth: DebugView,
    pub hiz: Option<DebugView>,
}

impl ClassicViews {
    pub fn displays_texture(&self, selected: SelectedView) -> bool {
        selected
            .0
            .is_some_and(|view| view == self.depth || Some(view) == self.hiz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> (DebugViews, [DebugView; 3]) {
        let mut views = DebugViews::default();
        let a = views.register(RenderPath::Classic, "a");
        let other = views.register(RenderPath::Deferred, "other");
        let b = views.register(RenderPath::Classic, "b");
        (views, [a, b, other])
    }

    #[test]
    fn stepping_forward_visits_each_view_of_the_path_then_final_shading() {
        let (views, [a, b, _]) = registry();
        let path = RenderPath::Classic;
        assert_eq!(views.step(path, None, false), Some(a));
        assert_eq!(views.step(path, Some(a), false), Some(b));
        assert_eq!(views.step(path, Some(b), false), None);
    }

    #[test]
    fn stepping_back_reverses_the_cycle() {
        let (views, [a, b, _]) = registry();
        let path = RenderPath::Classic;
        assert_eq!(views.step(path, None, true), Some(b));
        assert_eq!(views.step(path, Some(b), true), Some(a));
        assert_eq!(views.step(path, Some(a), true), None);
    }

    #[test]
    fn a_view_of_another_path_is_never_visited() {
        let (views, [_, _, other]) = registry();
        let mut current = None;
        for _ in 0..6 {
            current = views.step(RenderPath::Classic, current, false);
            assert_ne!(current, Some(other));
        }
        assert_eq!(views.step(RenderPath::Deferred, None, false), Some(other));
        assert_eq!(views.step(RenderPath::Deferred, Some(other), false), None);
    }

    #[test]
    fn a_selection_the_path_lacks_steps_to_its_first_or_last_view() {
        let (views, [a, b, other]) = registry();
        assert_eq!(views.step(RenderPath::Classic, Some(other), false), Some(a));
        assert_eq!(views.step(RenderPath::Classic, Some(other), true), Some(b));
    }

    #[test]
    fn a_path_without_views_stays_at_final_shading() {
        let mut views = DebugViews::default();
        let classic = views.register(RenderPath::Classic, "a");
        assert_eq!(views.step(RenderPath::Deferred, None, false), None);
        assert_eq!(views.step(RenderPath::Deferred, None, true), None);
        assert_eq!(views.step(RenderPath::Deferred, Some(classic), false), None);
    }
}
