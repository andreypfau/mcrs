use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::input::common_conditions::input_just_released;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::{ExtractSchedule, RenderApp};
use bevy::text::{FontSize, Justify, LineBreak};
use bevy::ui::UiSystems;
use bevy::ui_render::RenderUiSystems;

use crate::chunk_map::ChunkMap;
use crate::debug_displayer::DebugScreenDisplayer;
use crate::light_levels::LightLevels;

#[derive(Resource, Clone, Copy, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct UiNeeded {
    now: bool,
    before: bool,
}

impl UiNeeded {
    fn wanted(&self) -> bool {
        self.now || self.before
    }
}

fn ui_needed(needed: Option<Res<UiNeeded>>) -> bool {
    needed.is_some_and(|needed| needed.wanted())
}

fn debug_view_shown(
    overlay: &DebugOverlay,
    map: Option<&ChunkMap>,
    levels: Option<&LightLevels>,
) -> bool {
    **overlay
        || map.is_some_and(|map| map.visible())
        || levels.is_some_and(|levels| levels.visible())
}

fn ui_needed_stale(
    overlay: Res<DebugOverlay>,
    map: Option<Res<ChunkMap>>,
    levels: Option<Res<LightLevels>>,
    needed: Res<UiNeeded>,
) -> bool {
    let now = debug_view_shown(&overlay, map.as_deref(), levels.as_deref());
    now != needed.now || needed.before != needed.now
}

fn update_ui_needed(
    overlay: Res<DebugOverlay>,
    map: Option<Res<ChunkMap>>,
    levels: Option<Res<LightLevels>>,
    mut needed: ResMut<UiNeeded>,
) {
    let now = debug_view_shown(&overlay, map.as_deref(), levels.as_deref());
    let next = UiNeeded {
        now,
        before: needed.now,
    };
    if (next.now, next.before) != (needed.now, needed.before) {
        *needed = next;
    }
}

#[derive(Resource)]
pub struct Refresh {
    active: bool,
    logging: bool,
    log_every: Option<f32>,
    always_timed: bool,
    next_refresh: f32,
    next_log: f32,
}

impl Refresh {
    const SHOWN_INTERVAL: f32 = 0.1;

    fn new(log_every: Option<f32>, always_timed: bool) -> Self {
        Self {
            active: false,
            logging: false,
            log_every,
            always_timed,
            next_refresh: 0.0,
            next_log: 0.0,
        }
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct DebugOverlay(bool);

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DebugScreenSet {
    Clear,
    Collect,
    Render,
}

pub struct DebugScreenPlugin {
    pub stats_interval: Option<f32>,
    pub always_time_passes: bool,
}

impl Plugin for DebugScreenPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::new(
                crate::debug_entry_fps::HISTORY,
            ));
        }
        app.init_resource::<DebugOverlay>()
            .init_resource::<mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink>(
            )
            .init_resource::<DebugScreenDisplayer>()
            .init_resource::<UiNeeded>()
            .add_plugins(ExtractResourcePlugin::<UiNeeded>::default())
            .add_systems(Last, update_ui_needed.run_if(ui_needed_stale))
            .configure_sets(PreUpdate, UiSystems::Focus.run_if(ui_needed))
            .configure_sets(
                PostUpdate,
                (
                    UiSystems::Prepare,
                    UiSystems::Propagate,
                    UiSystems::Content,
                    UiSystems::Layout,
                    UiSystems::PostLayout,
                    UiSystems::Stack,
                )
                    .run_if(ui_needed),
            )
            .insert_resource(Refresh::new(self.stats_interval, self.always_time_passes))
            .configure_sets(
                Update,
                (
                    DebugScreenSet::Clear,
                    DebugScreenSet::Collect,
                    DebugScreenSet::Render,
                )
                    .chain()
                    .run_if(refresh_active),
            )
            .init_resource::<DebugModifier>()
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                (
                    (
                        toggle_overlay.run_if(input_just_released(KeyCode::F3)),
                        schedule_refresh.run_if(refresh_scheduled),
                    )
                        .chain()
                        .before(DebugScreenSet::Clear),
                    clear_displayer.in_set(DebugScreenSet::Clear),
                    (render, log_debug_screen.run_if(refresh_logging))
                        .in_set(DebugScreenSet::Render),
                ),
            );
        app.add_systems(
            Update,
            (
                crate::debug_entry_fps::display,
                crate::debug_entry_fps::display_version,
                crate::debug_entry_network::display,
                crate::debug_entry_position::display,
                crate::debug_entry_system_specs::display,
            )
                .chain()
                .in_set(DebugScreenSet::Collect)
                .run_if(collecting),
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.configure_sets(
                ExtractSchedule,
                (
                    RenderUiSystems::ExtractCameraViews,
                    RenderUiSystems::ExtractBoxShadows,
                    RenderUiSystems::ExtractBackgrounds,
                    RenderUiSystems::ExtractImages,
                    RenderUiSystems::ExtractTextureSlice,
                    RenderUiSystems::ExtractBorders,
                    RenderUiSystems::ExtractViewportNodes,
                    RenderUiSystems::ExtractTextBackgrounds,
                    RenderUiSystems::ExtractTextShadows,
                    RenderUiSystems::ExtractText,
                    RenderUiSystems::ExtractCursor,
                    RenderUiSystems::ExtractDebug,
                    RenderUiSystems::ExtractGradient,
                )
                    .run_if(ui_needed),
            );
        }
    }
}

pub fn collecting(overlay: Option<Res<DebugOverlay>>, refresh: Option<Res<Refresh>>) -> bool {
    overlay.is_some_and(|overlay| **overlay) || refresh.is_some_and(|refresh| refresh.logging)
}

fn refresh_scheduled(refresh: Res<Refresh>, overlay: Res<DebugOverlay>) -> bool {
    **overlay
        || overlay.is_changed()
        || refresh.active
        || refresh.log_every.is_some()
        || refresh.always_timed
}

fn schedule_refresh(
    mut refresh: ResMut<Refresh>,
    overlay: Res<DebugOverlay>,
    time: Res<Time>,
    gpu: Res<mcrs_minecraft_render_probe::probe::GpuTimings>,
) {
    gpu.want(refresh.always_timed || **overlay || refresh.log_every.is_some());
    let now = time.elapsed_secs();
    let refresh = refresh.bypass_change_detection();
    let shown = **overlay && now >= refresh.next_refresh;
    if shown {
        refresh.next_refresh = now + Refresh::SHOWN_INTERVAL;
    }
    refresh.logging = refresh.log_every.is_some_and(|every| {
        let due = now >= refresh.next_log;
        if due {
            refresh.next_log = now + every;
        }
        due
    });
    refresh.active = shown || refresh.logging || overlay.is_changed();
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn refresh_active(refresh: Res<Refresh>) -> bool {
    refresh.active
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn refresh_logging(refresh: Res<Refresh>) -> bool {
    refresh.logging
}

fn log_debug_screen(displayer: Res<DebugScreenDisplayer>) {
    let (left, right) = displayer.columns();
    let line = left
        .into_iter()
        .chain(right)
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    info!(target: "mcrs_client::stats", "{line}");
}

fn clear_displayer(mut displayer: ResMut<DebugScreenDisplayer>) {
    displayer.clear();
}

const MARGIN: f32 = 2.0;
const BACKGROUND: Color = Color::srgba(0.314, 0.314, 0.314, 0.565);
const TEXT: Color = Color::srgb(0.878, 0.878, 0.878);
const FONT_SIZE: f32 = 16.0;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum DebugScreenColumn {
    Left,
    Right,
}

pub fn spawn(mut commands: Commands) {
    for column in [DebugScreenColumn::Left, DebugScreenColumn::Right] {
        let (left, right) = match column {
            DebugScreenColumn::Left => (Val::Px(MARGIN), Val::Auto),
            DebugScreenColumn::Right => (Val::Auto, Val::Px(MARGIN)),
        };
        let justify = match column {
            DebugScreenColumn::Left => Justify::Left,
            DebugScreenColumn::Right => Justify::Right,
        };
        commands.spawn((
            column,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextLayout::new(justify, LineBreak::NoWrap),
            TextColor(TEXT),
            BackgroundColor(BACKGROUND),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(MARGIN),
                left,
                right,
                ..default()
            },
            Visibility::Hidden,
        ));
    }
}

#[derive(Resource, Default, PartialEq)]
pub struct DebugModifier {
    pub used: bool,
}

pub fn toggle_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut modifier: ResMut<DebugModifier>,
    mut overlay: ResMut<DebugOverlay>,
) {
    if keys.just_released(KeyCode::F3) && !std::mem::take(&mut modifier.used) {
        **overlay = !**overlay;
    }
}

pub fn render(
    displayer: Res<DebugScreenDisplayer>,
    overlay: Res<DebugOverlay>,
    mut columns: Query<(&DebugScreenColumn, &mut Text, &mut Visibility)>,
) {
    let (left, right) = displayer.columns();
    for (column, mut text, mut visibility) in &mut columns {
        let lines = match column {
            DebugScreenColumn::Left => &left,
            DebugScreenColumn::Right => &right,
        };
        let end = lines
            .iter()
            .rposition(|line| !line.is_empty())
            .map_or(0, |last| last + 1);
        let wanted = if end == 0 || !**overlay {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        visibility.set_if_neq(wanted);
        if wanted == Visibility::Hidden {
            continue;
        }
        let block = lines.get(..end).unwrap_or(lines).join("\n");
        if text.0 != block {
            text.0 = block;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    #[test]
    fn nothing_collects_without_the_debug_screen() {
        let mut world = World::new();
        assert!(!world.run_system_once(collecting).unwrap());
    }

    #[test]
    fn the_overlay_alone_needs_the_ui() {
        let mut world = World::new();
        world.insert_resource(DebugOverlay(true));
        world.init_resource::<UiNeeded>();
        world.run_system_once(update_ui_needed).unwrap();
        assert!(world.resource::<UiNeeded>().wanted());
    }
}
