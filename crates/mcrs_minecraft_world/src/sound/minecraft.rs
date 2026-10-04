use std::sync::LazyLock;

use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_item::SoundEvent;
use mcrs_minecraft_registry::StaticRegistry;

pub const EMPTY: ResourceLocation<&'static str> = rl!("minecraft:intentionally_empty");

pub const WOOD_BREAK: ResourceLocation<&'static str> = rl!("minecraft:block.wood.break");
pub const WOOD_FALL: ResourceLocation<&'static str> = rl!("minecraft:block.wood.fall");
pub const WOOD_HIT: ResourceLocation<&'static str> = rl!("minecraft:block.wood.hit");
pub const WOOD_PLACE: ResourceLocation<&'static str> = rl!("minecraft:block.wood.place");
pub const WOOD_STEP: ResourceLocation<&'static str> = rl!("minecraft:block.wood.step");

pub const STONE_BREAK: ResourceLocation<&'static str> = rl!("minecraft:block.stone.break");
pub const STONE_FALL: ResourceLocation<&'static str> = rl!("minecraft:block.stone.fall");
pub const STONE_HIT: ResourceLocation<&'static str> = rl!("minecraft:block.stone.hit");
pub const STONE_PLACE: ResourceLocation<&'static str> = rl!("minecraft:block.stone.place");
pub const STONE_PRESSURE_PLATE_CLICK_OFF: ResourceLocation<&'static str> =
    rl!("minecraft:block.stone_pressure_plate.click_off");
pub const STONE_PRESSURE_PLATE_CLICK_ON: ResourceLocation<&'static str> =
    rl!("minecraft:block.stone_pressure_plate.click_on");
pub const STONE_STEP: ResourceLocation<&'static str> = rl!("minecraft:block.stone.step");

const ALL: [ResourceLocation<&'static str>; 13] = [
    EMPTY,
    WOOD_BREAK,
    WOOD_FALL,
    WOOD_HIT,
    WOOD_PLACE,
    WOOD_STEP,
    STONE_BREAK,
    STONE_FALL,
    STONE_HIT,
    STONE_PLACE,
    STONE_PRESSURE_PLATE_CLICK_OFF,
    STONE_PRESSURE_PLATE_CLICK_ON,
    STONE_STEP,
];

static EVENTS: LazyLock<[SoundEvent; 13]> = LazyLock::new(|| {
    ALL.map(|name| SoundEvent {
        sound_id: name.into(),
        range: None,
    })
});

pub fn register_all_sounds(registry: &mut StaticRegistry<SoundEvent>) {
    for (name, event) in ALL.into_iter().zip(EVENTS.iter()) {
        registry.register(name, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sound_table_keeps_its_names_and_ranges() {
        let mut registry = StaticRegistry::new();
        register_all_sounds(&mut registry);

        let expected = [
            EMPTY,
            WOOD_BREAK,
            WOOD_FALL,
            WOOD_HIT,
            WOOD_PLACE,
            WOOD_STEP,
            STONE_BREAK,
            STONE_FALL,
            STONE_HIT,
            STONE_PLACE,
            STONE_PRESSURE_PLATE_CLICK_OFF,
            STONE_PRESSURE_PLATE_CLICK_ON,
            STONE_STEP,
        ];
        let filled: Vec<_> = registry
            .iter()
            .map(|(_, name, event)| {
                (
                    name.as_str().to_owned(),
                    event.sound_id.as_str(),
                    event.range,
                )
            })
            .collect();
        assert_eq!(filled.len(), expected.len());
        for ((key, sound_id, range), name) in filled.iter().zip(expected) {
            assert_eq!(key, name.as_str());
            assert_eq!(*sound_id, name.as_str());
            assert_eq!(*range, None);
        }
    }
}
