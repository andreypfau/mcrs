use bevy::math::IVec2;
use bevy::prelude::*;
use mcrs_minecraft_client_world::food_level::FoodLevel;
use mcrs_minecraft_client_world::game_mode::LocalGameMode;
use mcrs_minecraft_client_world::hardcore_mode::HardcoreMode;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_game_mode::GameMode;

use crate::hud_systems::HudSystems;
use crate::scene::{GuiQuad, GuiScene, GuiSize, sprite};

const ICON: IVec2 = IVec2::splat(9);
const HALF_HOTBAR_WIDTH: i32 = 91;
const ROW_ZERO_ABOVE_BOTTOM: i32 = 39;
const ICON_STEP: i32 = 8;
const ICONS_PER_ROW: i32 = 10;
const HEALTH_PER_ROW: f32 = 20.0;
const ROW_HEIGHT_BASE: i32 = 12;
const MIN_ROW_HEIGHT: i32 = 3;
const SHAKE_HEALTH: i32 = 4;

pub struct HealthHudPlugin;

impl Plugin for HealthHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudTicks>()
            .add_systems(FixedUpdate, count_hud_ticks)
            .add_systems(
                PostUpdate,
                draw_health_hud
                    .in_set(HudSystems)
                    .run_if(resource_exists::<GuiScene>),
            );
    }
}

#[derive(Resource, Default)]
struct HudTicks(u32);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Hearts {
    health: f32,
    max_health: f32,
    hardcore: HardcoreMode,
    tick: u32,
}

struct HeartSprites {
    container: &'static str,
    full: &'static str,
    half: &'static str,
}

const NORMAL_HEARTS: HeartSprites = HeartSprites {
    container: "hud/heart/container",
    full: "hud/heart/full",
    half: "hud/heart/half",
};

const HARDCORE_HEARTS: HeartSprites = HeartSprites {
    container: "hud/heart/container_hardcore",
    full: "hud/heart/hardcore_full",
    half: "hud/heart/hardcore_half",
};

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        mcrs::missed_set_if_neq,
        reason = "the heart shake animation counts every fixed tick, so each run writes a new value"
    )
)]
fn count_hud_ticks(mut ticks: ResMut<HudTicks>) {
    ticks.0 = ticks.0.wrapping_add(1);
}

fn draw_health_hud(
    player: Single<
        (
            &LocalGameMode,
            &Vitals,
            &FoodLevel,
            Option<&AttributeInstances>,
            Option<&HardcoreMode>,
        ),
        With<Player>,
    >,
    screen: Option<Res<Screen>>,
    ticks: Res<HudTicks>,
    mut scene: ResMut<GuiScene>,
) {
    let (mode, vitals, food, attributes, hardcore) = *player;
    if !matches!(mode.current, GameMode::Survival | GameMode::Adventure)
        || screen.is_some_and(|screen| *screen == Screen::LevelLoading)
    {
        return;
    }
    let hearts = Hearts {
        health: vitals.health,
        max_health: attributes
            .and_then(|attributes| attributes.value(Attribute::MaxHealth))
            .unwrap_or(Attribute::MaxHealth.definition().default) as f32,
        hardcore: hardcore.copied().unwrap_or_default(),
        tick: ticks.0,
    };
    let scene = &mut *scene;
    push_hearts(hearts, scene.size, &mut scene.quads);
    push_food_bar(food.food, scene.size, &mut scene.quads);
}

fn push_hearts(hearts: Hearts, size: GuiSize, quads: &mut Vec<GuiQuad>) {
    let limit = Attribute::MaxHealth.definition().range.max as f32;
    let health = hearts.health.ceil().clamp(0.0, limit) as i32;
    let health_limit = hearts.max_health.clamp(0.0, limit).max(health as f32);
    let count = (health_limit / 2.0).ceil() as i32;
    let rows = (health_limit / HEALTH_PER_ROW).ceil() as i32;
    let row_height = (ROW_HEIGHT_BASE - rows).max(MIN_ROW_HEIGHT);
    let sprites = match hearts.hardcore {
        HardcoreMode::Off => &NORMAL_HEARTS,
        HardcoreMode::On => &HARDCORE_HEARTS,
    };
    let left = size.width.div_euclid(2) - HALF_HOTBAR_WIDTH;
    let bottom = size.height - ROW_ZERO_ABOVE_BOTTOM;
    for index in (0..count).rev() {
        let shake = if health <= SHAKE_HEALTH {
            shake(hearts.tick, index)
        } else {
            0
        };
        let at = IVec2::new(
            left + ICON_STEP * (index % ICONS_PER_ROW),
            bottom - row_height * (index / ICONS_PER_ROW) + shake,
        );
        quads.push(sprite(at, ICON, sprites.container));
        let points = 2 * index;
        if points < health {
            let fill = if points + 1 == health {
                sprites.half
            } else {
                sprites.full
            };
            quads.push(sprite(at, ICON, fill));
        }
    }
}

fn push_food_bar(food: i32, size: GuiSize, quads: &mut Vec<GuiQuad>) {
    let right = size.width.div_euclid(2) + HALF_HOTBAR_WIDTH - ICON.x;
    let top = size.height - ROW_ZERO_ABOVE_BOTTOM;
    for index in 0..ICONS_PER_ROW {
        let at = IVec2::new(right - ICON_STEP * index, top);
        quads.push(sprite(at, ICON, "hud/food_empty"));
        let points = 2 * index + 1;
        if points < food {
            quads.push(sprite(at, ICON, "hud/food_full"));
        } else if points == food {
            quads.push(sprite(at, ICON, "hud/food_half"));
        }
    }
}

fn shake(tick: u32, heart: i32) -> i32 {
    let mut mixed =
        tick.wrapping_mul(0x9E37_79B9) ^ heart.cast_unsigned().wrapping_mul(0x85EB_CA6B);
    mixed ^= mixed >> 16;
    mixed = mixed.wrapping_mul(0x7FEB_352D);
    mixed ^= mixed >> 15;
    i32::from(mixed & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Bar {
        Hearts(Hearts),
        Food(i32),
    }

    struct Case {
        bar: Bar,
        width: i32,
        height: i32,
        count: usize,
        first: &'static [(i32, i32, &'static str)],
    }

    const fn hearts(health: f32, max_health: f32, hardcore: HardcoreMode) -> Bar {
        Bar::Hearts(Hearts {
            health,
            max_health,
            hardcore,
            tick: 0,
        })
    }

    const CASES: [Case; 7] = [
        Case {
            bar: hearts(6.5, 8.0, HardcoreMode::Off),
            width: 480,
            height: 270,
            count: 8,
            first: &[
                (173, 231, "hud/heart/container"),
                (173, 231, "hud/heart/half"),
                (165, 231, "hud/heart/container"),
                (165, 231, "hud/heart/full"),
                (157, 231, "hud/heart/container"),
                (157, 231, "hud/heart/full"),
                (149, 231, "hud/heart/container"),
                (149, 231, "hud/heart/full"),
            ],
        },
        Case {
            bar: hearts(5.0, 7.0, HardcoreMode::On),
            width: 479,
            height: 260,
            count: 7,
            first: &[
                (172, 221, "hud/heart/container_hardcore"),
                (164, 221, "hud/heart/container_hardcore"),
                (164, 221, "hud/heart/hardcore_half"),
                (156, 221, "hud/heart/container_hardcore"),
                (156, 221, "hud/heart/hardcore_full"),
                (148, 221, "hud/heart/container_hardcore"),
                (148, 221, "hud/heart/hardcore_full"),
            ],
        },
        Case {
            bar: hearts(22.0, 22.0, HardcoreMode::Off),
            width: 480,
            height: 270,
            count: 22,
            first: &[
                (149, 221, "hud/heart/container"),
                (149, 221, "hud/heart/full"),
                (221, 231, "hud/heart/container"),
                (221, 231, "hud/heart/full"),
            ],
        },
        Case {
            bar: hearts(21.5, 2.0, HardcoreMode::Off),
            width: 480,
            height: 270,
            count: 22,
            first: &[
                (149, 221, "hud/heart/container"),
                (149, 221, "hud/heart/full"),
            ],
        },
        Case {
            bar: hearts(200.0, 200.0, HardcoreMode::Off),
            width: 480,
            height: 270,
            count: 200,
            first: &[
                (221, 204, "hud/heart/container"),
                (221, 204, "hud/heart/full"),
            ],
        },
        Case {
            bar: Bar::Food(7),
            width: 480,
            height: 270,
            count: 14,
            first: &[
                (322, 231, "hud/food_empty"),
                (322, 231, "hud/food_full"),
                (314, 231, "hud/food_empty"),
                (314, 231, "hud/food_full"),
                (306, 231, "hud/food_empty"),
                (306, 231, "hud/food_full"),
                (298, 231, "hud/food_empty"),
                (298, 231, "hud/food_half"),
                (290, 231, "hud/food_empty"),
            ],
        },
        Case {
            bar: Bar::Food(0),
            width: 481,
            height: 270,
            count: 10,
            first: &[(322, 231, "hud/food_empty"), (314, 231, "hud/food_empty")],
        },
    ];

    #[test]
    fn hearts_and_food_follow_the_layout() {
        for (number, case) in CASES.iter().enumerate() {
            let size = GuiSize {
                width: case.width,
                height: case.height,
                scale: 1,
            };
            let mut quads = Vec::new();
            match case.bar {
                Bar::Hearts(hearts) => push_hearts(hearts, size, &mut quads),
                Bar::Food(food) => push_food_bar(food, size, &mut quads),
            }
            let drawn: Vec<(i32, i32, &str)> = quads
                .iter()
                .filter_map(|quad| match *quad {
                    GuiQuad::Sprite { rect, region, .. } if rect.size() == ICON => {
                        Some((rect.min.x, rect.min.y, region))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(drawn.len(), case.count, "case {number}");
            assert_eq!(
                drawn.get(..case.first.len()),
                Some(case.first),
                "case {number}"
            );
        }
    }
}
