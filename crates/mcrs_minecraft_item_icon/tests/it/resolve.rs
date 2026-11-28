use mcrs_minecraft_item_icon::{Durability, StackOverlay};
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::FireworkExplosion;
use mcrs_minecraft_item_stack::FireworkShape;

use crate::corpus::{icon, stack, view};

#[test]
fn item_stacks_resolve_against_the_corpus() {
    an_overlay_carries_the_count_and_the_durability();
    a_firework_star_tints_only_its_overlay();
    an_icon_is_dynamic_only_when_its_model_reads_the_holder_or_the_time();
}

fn an_overlay_carries_the_count_and_the_durability() {
    let pick = stack("diamond_pickaxe", 1).set(Damage(100));
    assert_eq!(
        StackOverlay::of(view(&pick)),
        StackOverlay {
            count: 1,
            durability: Some(Durability {
                damage: 100,
                max_damage: 1561
            })
        }
    );
    assert_eq!(
        StackOverlay::of(view(&stack("stone", 17))),
        StackOverlay {
            count: 17,
            durability: None
        }
    );
}

fn a_firework_star_tints_only_its_overlay() {
    let star = stack("firework_star", 1).set(FireworkExplosion {
        shape: FireworkShape::SmallBall,
        colors: vec![0x112233, 0x445566, 0x778899],
        fade_colors: Vec::new(),
        has_trail: false,
        has_twinkle: false,
    });
    let tints: Vec<[u32; 4]> = icon(&star).layers().map(|layer| layer.tints).collect();
    assert_eq!(
        tints,
        [[0xFFFF_FFFF, 0xFF44_5566, 0xFFFF_FFFF, 0xFFFF_FFFF]],
        "the star keeps its constant white base, tints its overlay with the mean colour and leaves the free tints white"
    );
}

fn an_icon_is_dynamic_only_when_its_model_reads_the_holder_or_the_time() {
    for (item, dynamic) in [
        ("stone", false),
        ("stick", false),
        ("diamond_pickaxe", false),
        ("clock", true),
        ("compass", true),
        ("bow", true),
    ] {
        assert_eq!(icon(&stack(item, 1)).is_dynamic(), dynamic, "{item}");
    }
}
