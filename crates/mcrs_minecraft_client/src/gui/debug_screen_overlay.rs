use bevy::prelude::*;
use bevy::text::{FontSize, Justify, LineBreak};

use crate::gui::debug::{DebugScreenDisplayer, DebugScreenEntryList};

const MARGIN: f32 = 2.0;
const BACKGROUND: Color = Color::srgba(0.314, 0.314, 0.314, 0.565);
const TEXT: Color = Color::srgb(0.878, 0.878, 0.878);
/// The vanilla font is a 9-pixel bitmap this client cannot draw yet, so the
/// columns are laid out as two text blocks in Bevy's default font instead of a
/// per-line fill behind a per-line string.
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

pub fn toggle_overlay(keys: Res<ButtonInput<KeyCode>>, mut list: ResMut<DebugScreenEntryList>) {
    if keys.just_pressed(KeyCode::F3) {
        list.toggle_overlay();
    }
}

pub fn render(
    displayer: Res<DebugScreenDisplayer>,
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
        let wanted = if end == 0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        visibility.set_if_neq(wanted);
        let block = lines[..end].join("\n");
        if text.0 != block {
            text.0 = block;
        }
    }
}
