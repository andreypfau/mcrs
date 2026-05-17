//! `BlockLightPlugin` is the block-channel composition point. Today it
//! registers the block-channel wire message; the per-channel system
//! registrations remain in `LightingPlugin` because their `chain()` /
//! `.after()` constraints still cross the channel boundary. As the
//! channel-shared files (enqueue/propagate/emit_dirty) split into their
//! per-channel siblings, the registrations migrate here.

use crate::codec::BlockLightDirty;
use bevy_app::{App, Plugin};

pub struct BlockLightPlugin;

impl Plugin for BlockLightPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockLightDirty>();
    }
}
