use bevy::math::IRect;
use bevy::math::IVec2;
use bevy::prelude::*;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::vitals::LocalVitals;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::serverbound::ClientCommandAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundClientCommand;

use crate::language::ClientLanguage;
use crate::scene::GuiQuad::Fill;

use crate::scene::{GuiQuad, GuiSize, sprite};
use crate::text::text_spans;

const HEART: &str = "hud/heart/container";
const FOOD: &str = "hud/food_empty";
const XP_BACKGROUND: &str = "hud/experience_bar_background";
const XP_PROGRESS: &str = "hud/experience_bar_progress";
const AIR: &str = "hud/air";
const AIR_EMPTY: &str = "hud/air_empty";
const ARMOR: &str = "hud/armor_empty";
const ARMOR_FULL: &str = "hud/armor_full";
const ARMOR_HALF: &str = "hud/armor_half";
const EFFECT_BACKGROUND: &str = "hud/effect_background";
const EFFECT_BACKGROUND_AMBIENT: &str = "hud/effect_background_ambient";

const EFFECT_ICONS: [&str; 41] = [
    "mob_effect/speed",
    "mob_effect/slowness",
    "mob_effect/haste",
    "mob_effect/mining_fatigue",
    "mob_effect/strength",
    "mob_effect/instant_health",
    "mob_effect/instant_damage",
    "mob_effect/jump_boost",
    "mob_effect/nausea",
    "mob_effect/regeneration",
    "mob_effect/resistance",
    "mob_effect/fire_resistance",
    "mob_effect/water_breathing",
    "mob_effect/invisibility",
    "mob_effect/blindness",
    "mob_effect/night_vision",
    "mob_effect/hunger",
    "mob_effect/weakness",
    "mob_effect/poison",
    "mob_effect/wither",
    "mob_effect/health_boost",
    "mob_effect/absorption",
    "mob_effect/saturation",
    "mob_effect/glowing",
    "mob_effect/levitation",
    "mob_effect/luck",
    "mob_effect/unluck",
    "mob_effect/slow_falling",
    "mob_effect/conduit_power",
    "mob_effect/dolphins_grace",
    "mob_effect/bad_omen",
    "mob_effect/hero_of_the_village",
    "mob_effect/darkness",
    "mob_effect/trial_omen",
    "mob_effect/raid_omen",
    "mob_effect/wind_charged",
    "mob_effect/weaving",
    "mob_effect/oozing",
    "mob_effect/infested",
    "mob_effect/breath_of_the_nautilus",
    "mob_effect/freezing",
];

pub fn vitals(size: GuiSize, vitals: &LocalVitals, game_time: Option<i64>, out: &mut Vec<GuiQuad>) {
    environment(size, vitals, out);
    experience_bar(size, vitals, out);
    let (cx, h) = (size.width / 2, size.height);
    let heart = if vitals.has(19) {
        2
    } else if vitals.has(18) {
        1
    } else {
        0
    };
    let max = vitals.max_health.max(vitals.health).max(0.0) + vitals.absorption.max(0.0);
    let total = vitals.health.max(0.0) + vitals.absorption.max(0.0);
    let hearts = ((max.ceil() as i32).max(0) + 1) / 2;
    let rows = ((hearts + 9) / 10).max(1);
    for i in 0..hearts {
        let row = i / 10;
        let at = IVec2::new(cx - 91 + (i % 10) * 8, h - 39 - row * 10);
        out.push(sprite(at, IVec2::new(9, 9), HEART));
        let points = total - i as f32 * 2.0;
        let absorbing = i as f32 * 2.0 < vitals.absorption;
        let fill = if points >= 2.0 {
            2
        } else if points > 0.0 {
            1
        } else {
            continue;
        };
        out.push(sprite(
            at,
            IVec2::new(9, 9),
            heart_region(heart, absorbing, fill),
        ));
    }
    if vitals.armor > 0.0 {
        for i in 0..10 {
            let at = IVec2::new(cx - 91 + i * 8, h - 49 - (rows - 1) * 10);
            out.push(sprite(at, IVec2::new(9, 9), ARMOR));
            let points = vitals.armor - i as f32 * 2.0;
            let region = if points >= 2.0 {
                ARMOR_FULL
            } else if points > 0.0 {
                ARMOR_HALF
            } else {
                continue;
            };
            out.push(sprite(at, IVec2::new(9, 9), region));
        }
    }
    if vitals.air < vitals.max_air {
        let air_start = cx + 91 - 9;
        let bubbles = (vitals.air.max(0) + 19) / 20;
        for i in 0..10 {
            let at = IVec2::new(air_start - i * 8, h - 49);
            out.push(sprite(at, IVec2::new(9, 9), AIR_EMPTY));
            if i < bubbles {
                out.push(sprite(at, IVec2::new(9, 9), AIR));
            }
        }
    }

    let food_start = cx + 91 - 9;
    let ticks = game_time.unwrap_or(0).max(0) as i32;
    for i in 0..10 {
        let shake = if vitals.food <= 6 {
            (ticks.wrapping_mul(31).wrapping_add(i).rem_euclid(3)) - 1
        } else {
            0
        };
        let at = IVec2::new(food_start - i * 8, h - 39 + shake);
        out.push(sprite(at, IVec2::new(9, 9), FOOD));
        let points = vitals.food - i * 2;
        let region = if points >= 2 {
            "hud/food_full"
        } else if points > 0 {
            "hud/food_half"
        } else {
            continue;
        };
        out.push(sprite(at, IVec2::new(9, 9), region));
    }

    effects(size, vitals, out);
}

fn experience_bar(size: GuiSize, vitals: &LocalVitals, out: &mut Vec<GuiQuad>) {
    let (cx, h) = (size.width / 2, size.height);
    let bar = IVec2::new(cx - 91, h - 29);
    out.push(sprite(bar, IVec2::new(182, 5), XP_BACKGROUND));
    let width = (vitals.experience_progress.clamp(0.0, 1.0) * 182.0).round() as i32;
    if width > 0 {
        out.push(GuiQuad::Sprite {
            rect: IRect::from_corners(bar, bar + IVec2::new(width, 5)),
            region: XP_PROGRESS,
            color: 0xFFFF_FFFF,
        });
    }
    let level = vitals.experience_level.max(0).to_string();
    let start = cx - (level.len() as i32 * 6) / 2;
    for (index, digit) in level.chars().enumerate() {
        let origin = IVec2::new(start + index as i32 * 6, h - 38);
        out.push(GuiQuad::Glyph {
            origin: origin + IVec2::new(1, 1),
            glyph: digit,
            color: 0xFF3F_3F3F,
        });
        out.push(GuiQuad::Glyph {
            origin,
            glyph: digit,
            color: 0xFFFF_FFFF,
        });
    }
}

fn effects(size: GuiSize, vitals: &LocalVitals, out: &mut Vec<GuiQuad>) {
    let shown = vitals
        .effects
        .iter()
        .filter(|effect| effect.show_icon)
        .take(8);
    for (index, effect) in shown.enumerate() {
        let Some(icon) = EFFECT_ICONS.get(effect.effect as usize) else {
            continue;
        };
        let at = IVec2::new(size.width - 25 - index as i32 * 24, 1);
        out.push(sprite(
            at,
            IVec2::new(24, 24),
            if effect.ambient {
                EFFECT_BACKGROUND_AMBIENT
            } else {
                EFFECT_BACKGROUND
            },
        ));
        out.push(sprite(at + IVec2::new(3, 3), IVec2::new(18, 18), icon));
        let text = if effect.duration < 0 {
            "**:**".to_owned()
        } else {
            let seconds = effect.duration / 20;
            if seconds >= 60 {
                format!("{}:{:02}", seconds / 60, seconds % 60)
            } else {
                seconds.to_string()
            }
        };
        draw_text(out, at.x + 12, at.y + 24, &text, 0xFFFF_FFFF, 1);
    }
}

fn environment(size: GuiSize, vitals: &LocalVitals, out: &mut Vec<GuiQuad>) {
    let screen = IRect::from_corners(IVec2::ZERO, IVec2::new(size.width, size.height));
    if vitals.on_fire {
        out.push(Fill {
            rect: screen,
            color: 0x30FF_5500,
        });
    }
    if vitals.has(14) {
        out.push(Fill {
            rect: screen,
            color: 0xB000_0000,
        });
    } else if vitals.has(15) {
        out.push(Fill {
            rect: screen,
            color: 0x1800_3050,
        });
    }
    if vitals.frozen_ticks > 0 {
        let border = 20;
        out.push(Fill {
            rect: IRect::from_corners(IVec2::ZERO, IVec2::new(size.width, border)),
            color: 0x5099_CCFF,
        });
        out.push(Fill {
            rect: IRect::from_corners(
                IVec2::new(0, size.height - border),
                IVec2::new(size.width, size.height),
            ),
            color: 0x5099_CCFF,
        });
        out.push(Fill {
            rect: IRect::from_corners(IVec2::ZERO, IVec2::new(border, size.height)),
            color: 0x5099_CCFF,
        });
        out.push(Fill {
            rect: IRect::from_corners(
                IVec2::new(size.width - border, 0),
                IVec2::new(size.width, size.height),
            ),
            color: 0x5099_CCFF,
        });
    }
}

pub fn death(
    screen: GuiSize,
    vitals: &LocalVitals,
    language: Option<&ClientLanguage>,
    out: &mut Vec<GuiQuad>,
) {
    if !vitals.dead {
        return;
    }
    let size = IRect::from_corners(IVec2::ZERO, IVec2::new(screen.width, screen.height));
    out.push(Fill {
        rect: size,
        color: 0x8000_0000,
    });
    let (cx, cy) = (screen.width / 2, screen.height / 2);
    let title = language.map_or("YOU DIED", |language| language.get("deathScreen.title"));
    let respawn = language.map_or("Respawn", |language| language.get("deathScreen.respawn"));
    draw_text(out, cx, cy - 30, title, 0xFFFF_FFFF, 3);
    if let (Some(language), Some(message)) = (language, vitals.death_message.as_ref()) {
        let message: String = text_spans(message, language, 0xFFFF_FFFF)
            .iter()
            .map(|span| span.text.as_str())
            .collect();
        draw_text(out, cx, cy - 8, &message, 0xFFFF_FFFF, 1);
    }
    let button = IRect::from_corners(IVec2::new(cx - 100, cy + 10), IVec2::new(cx + 100, cy + 30));
    out.push(Fill {
        rect: button,
        color: 0xFF60_6060,
    });
    out.push(Fill {
        rect: IRect::from_corners(button.min + IVec2::ONE, button.max - IVec2::ONE),
        color: 0xFF30_3030,
    });
    draw_text(out, cx, cy + 17, respawn, 0xFFFF_FFFF, 1);
}

fn draw_text(out: &mut Vec<GuiQuad>, cx: i32, y: i32, text: &str, color: u32, scale: i32) {
    let width = text.chars().count() as i32 * 6 * scale;
    let start = cx - width / 2;
    for (index, glyph) in text.chars().enumerate() {
        for dx in 0..scale {
            for dy in 0..scale {
                out.push(GuiQuad::Glyph {
                    origin: IVec2::new(start + index as i32 * 6 * scale + dx, y + dy),
                    glyph,
                    color,
                });
            }
        }
    }
}

pub fn respawn_button(
    buttons: Res<ButtonInput<MouseButton>>,
    config: Res<crate::scene::GuiConfig>,
    vitals: Query<&LocalVitals, With<Player>>,
    connection: Option<Single<&mut ClientConnection>>,
    window: Single<(&Window, &mut bevy::window::CursorOptions), With<bevy::window::PrimaryWindow>>,
    mut was_dead: Local<bool>,
) {
    let (window, mut cursor_options) = window.into_inner();
    let Ok(vitals) = vitals.single() else {
        return;
    };
    if vitals.dead != *was_dead {
        *was_dead = vitals.dead;
        crate::cursor_grab::grab(&mut cursor_options, !vitals.dead);
    }
    if !buttons.just_pressed(MouseButton::Left) || !vitals.dead {
        return;
    }
    let scale = config.scale.max(1) as i32;
    let cursor = config.cursor.unwrap_or_else(|| {
        window.cursor_position().map_or(IVec2::new(-1, -1), |at| {
            IVec2::new(at.x as i32, at.y as i32) / scale
        })
    });
    let half = IVec2::new(
        window.width() as i32 / scale / 2,
        window.height() as i32 / scale / 2,
    );
    let inside = (cursor.x - half.x).abs() <= 100 && (cursor.y - half.y - 20).abs() <= 10;
    if inside && let Some(mut connection) = connection {
        connection.write_packet(&ServerboundClientCommand {
            action: ClientCommandAction::PerformRespawn,
        });
    }
}

fn heart_region(heart: u8, absorbing: bool, fill: u8) -> &'static str {
    match (heart, absorbing, fill) {
        (2, false, 2) => "hud/heart/withered_full",
        (2, false, 1) => "hud/heart/withered_half",
        (2, true, 2) => "hud/heart/absorbing_full",
        (2, true, 1) => "hud/heart/absorbing_half",
        (1, false, 2) => "hud/heart/poisoned_full",
        (1, false, 1) => "hud/heart/poisoned_half",
        (1, true, 2) => "hud/heart/absorbing_full",
        (1, true, 1) => "hud/heart/absorbing_half",
        (_, true, 2) => "hud/heart/absorbing_full",
        (_, true, 1) => "hud/heart/absorbing_half",
        (_, false, 2) => "hud/heart/full",
        _ => "hud/heart/half",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size() -> GuiSize {
        GuiSize {
            width: 320,
            height: 240,
            scale: 1,
        }
    }

    fn sprite_at(out: &[GuiQuad], region: &str) -> Option<IRect> {
        out.iter().find_map(|quad| match quad {
            GuiQuad::Sprite {
                rect,
                region: found,
                ..
            } if *found == region => Some(*rect),
            _ => None,
        })
    }

    #[test]
    fn the_armor_row_is_drawn_above_the_hearts() {
        let state = LocalVitals {
            armor: 20.0,
            ..LocalVitals::default()
        };
        let mut out = Vec::new();
        vitals(size(), &state, Some(0), &mut out);
        let empty = sprite_at(&out, ARMOR).expect("empty armor icon");
        let full = sprite_at(&out, ARMOR_FULL).expect("full armor icon");
        assert_eq!(empty.min, IVec2::new(69, 191));
        assert_eq!(full.min, IVec2::new(69, 191));
        assert_eq!(
            sprite_at(&out, HEART).expect("heart").min,
            IVec2::new(69, 201)
        );
    }

    #[test]
    fn dmg_hud_001_a_high_max_health_draws_a_second_heart_row() {
        let state = LocalVitals {
            health: 40.0,
            max_health: 40.0,
            armor: 20.0,
            ..LocalVitals::default()
        };
        let mut out = Vec::new();
        vitals(size(), &state, Some(0), &mut out);
        let hearts: Vec<IRect> = out
            .iter()
            .filter_map(|quad| match quad {
                GuiQuad::Sprite { rect, region, .. } if *region == HEART => Some(*rect),
                _ => None,
            })
            .collect();
        assert_eq!(hearts.len(), 20);
        assert!(hearts.iter().any(|rect| rect.min.y == 191));
        assert_eq!(
            sprite_at(&out, ARMOR).expect("armor").min,
            IVec2::new(69, 181)
        );
    }

    #[test]
    fn a_hungry_player_food_bar_shakes() {
        let state = LocalVitals {
            food: 6,
            ..LocalVitals::default()
        };
        let mut out = Vec::new();
        vitals(size(), &state, Some(1), &mut out);
        assert_eq!(
            sprite_at(&out, FOOD).expect("food").min,
            IVec2::new(242, 201)
        );
        let mut out = Vec::new();
        vitals(size(), &state, Some(0), &mut out);
        assert_eq!(
            sprite_at(&out, FOOD).expect("food").min,
            IVec2::new(242, 200)
        );
    }

    #[test]
    fn experience_progress_fills_the_bar() {
        let state = LocalVitals {
            experience_progress: 0.5,
            ..LocalVitals::default()
        };
        let mut out = Vec::new();
        vitals(size(), &state, Some(0), &mut out);
        let progress = out
            .iter()
            .find_map(|quad| match quad {
                GuiQuad::Sprite {
                    rect,
                    region: XP_PROGRESS,
                    ..
                } => Some(*rect),
                _ => None,
            })
            .expect("experience progress");
        assert_eq!(progress.width(), 91);
    }
}
