use bevy::platform::collections::HashSet;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_resource::{CachedPipelineState, CachedRenderPipelineId, PipelineCache};
use bevy::shader::ShaderCacheError;
use mcrs_minecraft_render::{EffectivePath, RenderPath, ShownPath};

use crate::gbuffer::DeferredFrame;
use crate::pipelines::DeferredPipelines;
use crate::volume::Volume;

fn effective(requested: RenderPath, frame_exists: bool, pipelines_ready: bool) -> EffectivePath {
    EffectivePath(
        if requested == RenderPath::Deferred && frame_exists && pipelines_ready {
            RenderPath::Deferred
        } else {
            RenderPath::Classic
        },
    )
}

pub(crate) fn derive_effective_path(
    requested: Res<RenderPath>,
    frame: Option<Res<DeferredFrame>>,
    volume: Option<Res<Volume>>,
    pipelines: Res<DeferredPipelines>,
    cache: Res<PipelineCache>,
    mut effective_path: ResMut<EffectivePath>,
    shown: Res<ShownPath>,
    mut waiting: Local<Option<(Instant, u32)>>,
    mut failed: Local<HashSet<CachedRenderPipelineId>>,
) {
    for id in pipelines.ids() {
        if let CachedPipelineState::Err(error) = cache.get_render_pipeline_state(id)
            && !matches!(
                error,
                ShaderCacheError::ShaderNotLoaded(_)
                    | ShaderCacheError::ShaderImportNotYetAvailable
            )
            && failed.insert(id)
        {
            let label = cache.get_render_pipeline_descriptor(id).label.as_deref();
            error!(?label, "a deferred pipeline cannot be built: {error}");
        }
    }
    let allocated = frame.is_some() && volume.is_some();
    let next = effective(*requested, allocated, pipelines.ready(&cache));
    if next.0 == RenderPath::Deferred && effective_path.0 == RenderPath::Classic {
        let (since, frames) = waiting.take().unwrap_or((Instant::now(), 0));
        let ms = since.elapsed().as_millis();
        info!(frames, ms, "the deferred path is ready");
    } else if *requested == RenderPath::Deferred && next.0 == RenderPath::Classic {
        waiting.get_or_insert((Instant::now(), 0)).1 += 1;
    } else {
        *waiting = None;
    }
    effective_path.set_if_neq(next);
    shown.set(next.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_effective_path_is_deferred_only_once_the_frame_and_every_pipeline_exist() {
        let mut deferred = Vec::new();
        for requested in [RenderPath::Classic, RenderPath::Deferred] {
            for frame_exists in [false, true] {
                for pipelines_ready in [false, true] {
                    if effective(requested, frame_exists, pipelines_ready)
                        == EffectivePath(RenderPath::Deferred)
                    {
                        deferred.push((requested, frame_exists, pipelines_ready));
                    }
                }
            }
        }
        assert_eq!(deferred, [(RenderPath::Deferred, true, true)]);
    }
}
