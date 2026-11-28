use bevy::prelude::*;
use bevy::render::render_resource::{DownlevelFlags, WgpuLimits};
use bevy::render::renderer::{RenderAdapter, RenderDevice};
use bevy::shader::ShaderDefVal;

use super::words::WORD_ROW_BITS;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderPath {
    Full,
    Minimal,
}

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
