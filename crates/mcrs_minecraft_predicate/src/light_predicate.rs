use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use mcrs_voxel::BlockPos;
use serde::Deserialize;

use crate::location_view::LocationView;

const MAX_LIGHT: i32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct LightPredicate {
    #[serde(default)]
    light: MinMaxBounds<i32>,
}

impl LightPredicate {
    #[must_use]
    pub(crate) fn matches<W: LocationView>(&self, world: &W, pos: BlockPos) -> bool {
        world.is_loaded(pos)
            && raw_brightness(world, pos).is_some_and(|light| self.light.matches(light))
    }
}

fn raw_brightness<W: LocationView>(world: &W, pos: BlockPos) -> Option<i32> {
    let block = i32::from(world.block_light(pos)?);
    let sky = i32::from(world.sky_light(pos)?);
    let darkening = (MAX_LIGHT as f32 - world.sky_light_level()?) as i32;
    Some(block.max(sky.saturating_sub(darkening)).clamp(0, MAX_LIGHT))
}
