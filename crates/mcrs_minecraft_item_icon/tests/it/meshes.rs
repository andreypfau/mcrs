use std::num::NonZeroU8;

use glam::Vec3;
use mcrs_minecraft_item_icon::{GuiVertex, IconLayer, ItemIcon, light_directions};
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_model_file::model_file::GuiLight;

use crate::corpus::{MESHES, icon, stack};

fn near(a: Vec3, b: [f32; 3]) -> bool {
    a.abs_diff_eq(Vec3::from(b), 1e-4)
}

fn only_layer(icon: &ItemIcon) -> IconLayer {
    let layers: Vec<IconLayer> = icon.layers().copied().collect();
    let [layer]: [IconLayer; 1] = layers.try_into().expect("the icon has one layer");
    layer
}

fn vertices(layer: IconLayer) -> Vec<[GuiVertex; 4]> {
    MESHES
        .get(layer.model)
        .expect("every model of an icon has a mesh")
        .iter()
        .map(|quad| quad.tinted(&layer.tints))
        .collect()
}

fn facing(quad: &[GuiVertex; 4]) -> Vec3 {
    let [first, second, third, _] = quad.map(|vertex| Vec3::from(vertex.pos));
    (third - first).cross(second - first).normalize()
}

#[test]
fn light_directions_match_the_vanilla_poses() {
    let [flat0, flat1] = light_directions(GuiLight::Front);
    assert!(near(flat0, [-0.222519, -0.1714986, 0.9597257]), "{flat0}");
    assert!(near(flat1, [-0.2150121, -0.9718252, 0.0965678]), "{flat1}");
    let [side0, side1] = light_directions(GuiLight::Side);
    assert!(near(side0, [-0.9334392, -0.2626947, -0.2443002]), "{side0}");
    assert!(near(side1, [-0.1035714, -0.9766068, 0.1884464]), "{side1}");
}

#[test]
fn icons_draw_the_gui_posed_shaded_meshes_of_the_corpus() {
    a_block_item_shows_its_top_north_and_east_faces_lit_from_the_side();
    a_flat_item_is_drawn_at_sprite_colour_and_glints_when_enchanted();
}

fn a_block_item_shows_its_top_north_and_east_faces_lit_from_the_side() {
    let stone = icon(&stack("stone", 1));
    assert_eq!(stone.gui_light(), GuiLight::Side);
    assert!(!stone.foil());
    let quads = vertices(only_layer(&stone));
    assert_eq!(quads.len(), 6);
    let front: Vec<Vec3> = quads.iter().map(facing).filter(|n| n.z > 0.0).collect();
    assert_eq!(front.len(), 3, "{front:?}");
    for (normal, brightness) in [
        ([0.0, -0.8660254, 0.5], 255),
        ([0.7071066, 0.3535534, 0.6123725], 102),
        ([-0.7071069, 0.3535533, 0.6123724], 166),
    ] {
        let quad = quads
            .iter()
            .find(|quad| near(facing(quad), normal))
            .expect("the stone shows a face with each listed normal");
        assert!(
            quad.iter()
                .all(|v| v.color == [brightness, brightness, brightness, 255])
        );
    }
    let all: Vec<Vec3> = quads.iter().flatten().map(|v| Vec3::from(v.pos)).collect();
    assert!(
        all.iter().any(|p| near(*p, [15.071068, 3.6698732, 2.5])),
        "{all:?}"
    );
    assert!(all.iter().any(|p| near(*p, [0.9289322, 3.6698723, 2.5])));
}

fn a_flat_item_is_drawn_at_sprite_colour_and_glints_when_enchanted() {
    let stick = icon(&stack("stick", 1));
    assert_eq!(stick.gui_light(), GuiLight::Front);
    let quads = vertices(only_layer(&stick));
    let front = quads
        .iter()
        .find(|quad| near(facing(quad), [0.0, 0.0, 1.0]))
        .expect("a front face");
    assert!(front.iter().all(|v| v.color == [255; 4]));
    assert!(
        front
            .iter()
            .all(|v| (0.0..=16.0).contains(&v.pos[0]) && (0.0..=16.0).contains(&v.pos[1]))
    );
    assert!(quads.iter().flatten().all(|v| v.sprite == front[0].sprite));
    assert!(!stick.foil());

    let level = EnchantmentLevel {
        enchantment: EnchantmentId(0),
        level: NonZeroU8::MIN,
    };
    let sharp = icon(&stack("stick", 1).set(Enchantments::new([level]).expect("one level")));
    assert!(sharp.foil());
    assert_ne!(sharp, stick, "a glint draws differently");
}
