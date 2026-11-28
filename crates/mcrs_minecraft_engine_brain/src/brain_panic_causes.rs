use std::marker::PhantomData;

use bevy_ecs::resource::Resource;
use mcrs_minecraft_brain::PanicCauses;

#[derive(Resource, Debug)]
pub struct BrainPanicCauses<B: 'static> {
    pub causes: PanicCauses,
    brain: PhantomData<fn() -> B>,
}

impl<B> BrainPanicCauses<B> {
    pub fn new(causes: PanicCauses) -> Self {
        Self {
            causes,
            brain: PhantomData,
        }
    }
}

impl<B> Default for BrainPanicCauses<B> {
    fn default() -> Self {
        Self::new(PanicCauses::default())
    }
}
