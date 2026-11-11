use bevy::prelude::*;
use bevy::render::render_resource::{DownlevelFlags, WgpuLimits};
use bevy::render::renderer::{RenderAdapter, RenderDevice};
use bevy::shader::ShaderDefVal;

use super::words::WORD_ROW_BITS;

/// How the world reaches the screen, fixed for the life of a device by what the device can do.
/// It is a resource of the main world and of the render world once the plugins have finished.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderPath {
    /// Compute passes cull on the GPU, shaders read storage buffers and draws are indirect.
    Full,
    /// No compute and no storage buffers, which is WebGL2: shaders read the same words from
    /// integer textures and a draw is one instance a group, with nothing culled before it.
    Minimal,
}

/// Storage buffers the widest stage of the full path binds: the quad cull's.
const FULL_STORAGE_BUFFERS: u32 = 8;

impl RenderPath {
    #[must_use]
    pub fn of(device: &RenderDevice, adapter: &RenderAdapter) -> Self {
        let flags = adapter.get_downlevel_capabilities().flags;
        Self::of_limits(
            &device.limits(),
            flags.contains(
                DownlevelFlags::COMPUTE_SHADERS
                    | DownlevelFlags::INDIRECT_EXECUTION
                    | DownlevelFlags::VERTEX_STORAGE,
            ),
        )
    }

    /// The limits decide as well as the adapter: a device may be opened under WebGL2 limits on
    /// an adapter that could do more.
    fn of_limits(limits: &WgpuLimits, compute: bool) -> Self {
        if compute
            && limits.max_compute_workgroups_per_dimension != 0
            && limits.max_storage_buffers_per_shader_stage >= FULL_STORAGE_BUFFERS
        {
            Self::Full
        } else {
            Self::Minimal
        }
    }

    #[must_use]
    pub fn is_full(self) -> bool {
        self == Self::Full
    }

    /// What a shader reading terrain words is compiled with on this path.
    #[must_use]
    pub fn shader_defs(self) -> Vec<ShaderDefVal> {
        match self {
            Self::Full => Vec::new(),
            Self::Minimal => vec![
                "MINIMAL".into(),
                ShaderDefVal::UInt("WORD_ROW_BITS".into(), WORD_ROW_BITS),
            ],
        }
    }
}

/// A run condition: the device runs the full path.
pub(crate) fn full_path(path: Option<Res<RenderPath>>) -> bool {
    path.is_some_and(|path| path.is_full())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limits_of_a_device_pick_its_path() {
        let cases = [
            (WgpuLimits::default(), true, RenderPath::Full),
            (WgpuLimits::default(), false, RenderPath::Minimal),
            (
                WgpuLimits::downlevel_webgl2_defaults(),
                true,
                RenderPath::Minimal,
            ),
            (
                WgpuLimits {
                    max_storage_buffers_per_shader_stage: FULL_STORAGE_BUFFERS - 1,
                    ..WgpuLimits::default()
                },
                true,
                RenderPath::Minimal,
            ),
        ];
        for (limits, compute, path) in cases {
            assert_eq!(RenderPath::of_limits(&limits, compute), path);
        }
    }
}
