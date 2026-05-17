//! `SkyLightPlugin` is the sky-channel composition point. Today it
//! registers the sky-channel wire message; the per-channel system
//! registrations remain in `LightingPlugin` because their `chain()` /
//! `.after()` constraints still cross the channel boundary. As the
//! channel-shared files (enqueue/propagate/emit_dirty) split into their
//! per-channel siblings, the registrations migrate here.

use crate::codec::SkyLightDirty;
use bevy_app::{App, Plugin};

pub struct SkyLightPlugin;

impl Plugin for SkyLightPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SkyLightDirty>();
    }
}
