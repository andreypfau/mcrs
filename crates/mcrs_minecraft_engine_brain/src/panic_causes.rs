use bevy_ecs::resource::Resource;

/// The network ids of the damage types in the tag `panic_causes`, sorted.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PanicCauses(Vec<u32>);

impl PanicCauses {
    #[must_use]
    pub fn new(ids: impl IntoIterator<Item = u32>) -> Self {
        let mut ids: Vec<u32> = ids.into_iter().collect();
        ids.sort_unstable();
        ids.dedup();
        Self(ids)
    }

    #[must_use]
    pub fn contains(&self, damage_type: u32) -> bool {
        self.0.binary_search(&damage_type).is_ok()
    }
}
