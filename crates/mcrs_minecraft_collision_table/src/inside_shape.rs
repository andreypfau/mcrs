use glam::DVec3;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::InsideBoxes;

/// The properties of a state that select its inside shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InsideProperties<'a> {
    pub(crate) level: Option<&'a PropertyValue>,
    pub(crate) attached: Option<&'a PropertyValue>,
}

/// The inside shape of PHY-RCH-008 for a block by its name. `cauldron` holds the collision boxes
/// of the empty cauldron; `out` receives the boxes of a shape that is not the full cube.
pub(crate) fn inside_shape<'a>(
    name: &str,
    properties: InsideProperties<'_>,
    cauldron: &[Aabb],
    out: &'a mut Vec<Aabb>,
) -> InsideBoxes<'a> {
    out.clear();
    let path = name.strip_prefix("minecraft:").unwrap_or(name);
    match path {
        "water_cauldron" | "powder_snow_cauldron" => {
            let level = match properties.level {
                Some(PropertyValue::Int(level)) => f64::from(*level),
                _ => 0.0,
            };
            out.extend_from_slice(cauldron);
            out.push(pixels([2.0, 4.0, 2.0], [14.0, 6.0 + 3.0 * level, 14.0]));
        }
        "lava_cauldron" => {
            out.extend_from_slice(cauldron);
            out.push(pixels([2.0, 4.0, 2.0], [14.0, 15.0, 14.0]));
        }
        "end_portal" => out.push(pixels([0.0, 6.0, 0.0], [16.0, 12.0, 16.0])),
        "tripwire" => match properties.attached {
            Some(PropertyValue::Bool(true)) => {
                out.push(pixels([0.0, 1.0, 0.0], [16.0, 2.5, 16.0]));
            }
            _ => out.push(pixels([0.0; 3], [16.0, 8.0, 16.0])),
        },
        "powder_snow" => return InsideBoxes::Collision,
        _ => return InsideBoxes::FullCube,
    }
    InsideBoxes::Boxes(out)
}

fn pixels(min: [f64; 3], max: [f64; 3]) -> Aabb {
    Aabb::new(DVec3::from_array(min) / 16.0, DVec3::from_array(max) / 16.0)
}
