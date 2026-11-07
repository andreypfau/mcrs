use std::collections::BTreeMap;

use bevy::math::{DVec3, Vec3};
use mcrs_minecraft_core::Direction;
use serde::Deserialize;

use crate::resource_set::full_id;

/// The light that a GUI draws an item model with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuiLight {
    Front,
    #[default]
    Side,
}

/// A slot value that is not a reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureValue {
    pub sprite: String,
    pub force_translucent: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotValue {
    /// The name of another slot, without its `#`.
    Reference(String),
    Texture(TextureValue),
}

/// The contexts a model can declare a display transform for, in the order of [`DisplayContext::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DisplayContext {
    ThirdPersonRightHand,
    ThirdPersonLeftHand,
    FirstPersonRightHand,
    FirstPersonLeftHand,
    Head,
    Gui,
    Ground,
    Fixed,
    OnShelf,
}

impl DisplayContext {
    pub const ALL: [DisplayContext; 9] = [
        DisplayContext::ThirdPersonRightHand,
        DisplayContext::ThirdPersonLeftHand,
        DisplayContext::FirstPersonRightHand,
        DisplayContext::FirstPersonLeftHand,
        DisplayContext::Head,
        DisplayContext::Gui,
        DisplayContext::Ground,
        DisplayContext::Fixed,
        DisplayContext::OnShelf,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            DisplayContext::ThirdPersonRightHand => "thirdperson_righthand",
            DisplayContext::ThirdPersonLeftHand => "thirdperson_lefthand",
            DisplayContext::FirstPersonRightHand => "firstperson_righthand",
            DisplayContext::FirstPersonLeftHand => "firstperson_lefthand",
            DisplayContext::Head => "head",
            DisplayContext::Gui => "gui",
            DisplayContext::Ground => "ground",
            DisplayContext::Fixed => "fixed",
            DisplayContext::OnShelf => "on_shelf",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A display transform: degrees, then blocks after the clamp, then the clamped scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayTransform {
    pub rotation_deg: Vec3,
    pub translation: Vec3,
    pub scale: Vec3,
}

impl DisplayTransform {
    pub const IDENTITY: DisplayTransform = DisplayTransform {
        rotation_deg: Vec3::ZERO,
        translation: Vec3::ZERO,
        scale: Vec3::ONE,
    };
}

impl Default for DisplayTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// The turn of an element about its origin, in pixel space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementRotation {
    pub origin: DVec3,
    pub turn: ElementTurn,
    pub rescale: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ElementTurn {
    /// A right-handed turn about the positive end of one axis.
    Axis { axis: Axis, degrees: f64 },
    /// Degrees about X, then Y, then Z.
    Euler(DVec3),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// One face entry of an element.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceEntry {
    /// The `texture` string as the file writes it, such as `#side`.
    pub texture: String,
    pub uv: Option<[f64; 4]>,
    pub cullface: Option<Direction>,
    /// Quarter turns, `0` to `3`.
    pub rotation: u8,
    pub tint: Option<u32>,
}

/// One element of a model, in pixel space.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub from: DVec3,
    pub to: DVec3,
    pub rotation: Option<ElementRotation>,
    /// The face entries, indexed in direction order.
    pub faces: [Option<FaceEntry>; 6],
    pub shade_direction_override: Option<Direction>,
    pub light_emission: u8,
}

impl Element {
    #[must_use]
    pub fn face(&self, direction: Direction) -> Option<&FaceEntry> {
        self.faces.get(direction as usize)?.as_ref()
    }
}

/// A model file. A field is `None` when the file has no key for it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelFile {
    pub parent: Option<String>,
    pub elements: Option<Vec<Element>>,
    pub textures: BTreeMap<String, SlotValue>,
    pub ambient_occlusion: Option<bool>,
    pub gui_light: Option<GuiLight>,
    /// The transforms of the contexts this file declares, indexed by [`DisplayContext::index`].
    pub display: [Option<DisplayTransform>; 9],
}

impl ModelFile {
    /// Reads a model file. An error is a content error: the model is then absent.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let raw: RawModel = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let gui_light = match raw.gui_light.as_deref() {
            None => None,
            Some("front") => Some(GuiLight::Front),
            Some("side") => Some(GuiLight::Side),
            Some(other) => return Err(format!("unknown gui_light `{other}`")),
        };
        let elements = raw
            .elements
            .map(|elements| elements.into_iter().map(RawElement::compile).collect())
            .transpose()?;
        let textures = raw
            .textures
            .unwrap_or_default()
            .into_iter()
            .map(|(slot, value)| (slot, value.compile()))
            .collect();
        Ok(Self {
            parent: raw
                .parent
                .filter(|parent| !parent.is_empty())
                .map(|p| full_id(&p)),
            elements,
            textures,
            ambient_occlusion: raw.ambientocclusion,
            gui_light,
            display: raw
                .display
                .map(display_table)
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

fn display_table(
    raw: BTreeMap<String, RawTransform>,
) -> Result<[Option<DisplayTransform>; 9], String> {
    let mut table = [None; 9];
    for context in DisplayContext::ALL {
        if let Some(transform) = raw.get(context.name())
            && let Some(slot) = table.get_mut(context.index())
        {
            *slot = Some(transform.compile()?);
        }
    }
    for (right, left) in [
        (
            DisplayContext::ThirdPersonRightHand,
            DisplayContext::ThirdPersonLeftHand,
        ),
        (
            DisplayContext::FirstPersonRightHand,
            DisplayContext::FirstPersonLeftHand,
        ),
    ] {
        let right = table.get(right.index()).copied().flatten();
        if let Some(slot) = table.get_mut(left.index())
            && slot.is_none()
        {
            *slot = right;
        }
    }
    Ok(table)
}

/// The direction that a name of a direction stands for.
#[must_use]
pub fn direction_named(name: &str) -> Option<Direction> {
    Direction::all()
        .into_iter()
        .find(|direction| direction.name() == name)
}

#[derive(Deserialize)]
struct RawModel {
    parent: Option<String>,
    elements: Option<Vec<RawElement>>,
    textures: Option<BTreeMap<String, RawSlot>>,
    ambientocclusion: Option<bool>,
    gui_light: Option<String>,
    display: Option<BTreeMap<String, RawTransform>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawSlot {
    Text(String),
    Sprite {
        sprite: String,
        #[serde(default)]
        force_translucent: bool,
    },
}

impl RawSlot {
    fn compile(self) -> SlotValue {
        match self {
            RawSlot::Text(text) => match text.strip_prefix('#') {
                Some(slot) => SlotValue::Reference(slot.to_owned()),
                None => SlotValue::Texture(TextureValue {
                    sprite: full_id(&text),
                    force_translucent: false,
                }),
            },
            RawSlot::Sprite {
                sprite,
                force_translucent,
            } => SlotValue::Texture(TextureValue {
                sprite: full_id(&sprite),
                force_translucent,
            }),
        }
    }
}

#[derive(Deserialize)]
struct RawTransform {
    rotation: Option<Vec<f32>>,
    translation: Option<Vec<f32>>,
    scale: Option<Vec<f32>>,
}

impl RawTransform {
    fn compile(&self) -> Result<DisplayTransform, String> {
        let triple = |values: &Option<Vec<f32>>, default: Vec3| -> Result<Vec3, String> {
            match values.as_deref() {
                None => Ok(default),
                Some(&[x, y, z]) => Ok(Vec3::new(x, y, z)),
                Some(other) => Err(format!("a transform array of {} numbers", other.len())),
            }
        };
        Ok(DisplayTransform {
            rotation_deg: triple(&self.rotation, Vec3::ZERO)?,
            translation: (triple(&self.translation, Vec3::ZERO)? / 16.0)
                .clamp(Vec3::splat(-5.0), Vec3::splat(5.0)),
            scale: triple(&self.scale, Vec3::ONE)?.clamp(Vec3::splat(-4.0), Vec3::splat(4.0)),
        })
    }
}

#[derive(Deserialize)]
struct RawElement {
    from: [f64; 3],
    to: [f64; 3],
    faces: BTreeMap<String, RawFace>,
    rotation: Option<RawRotation>,
    shade_direction_override: Option<String>,
    light_emission: Option<i64>,
}

impl RawElement {
    fn compile(self) -> Result<Element, String> {
        let corner = |values: [f64; 3]| -> Result<DVec3, String> {
            if values.iter().all(|value| (-16.0..=32.0).contains(value)) {
                Ok(DVec3::from_array(values))
            } else {
                Err(format!("the corner {values:?} is outside -16 to 32"))
            }
        };
        if self.faces.is_empty() {
            return Err("an element with no face".to_owned());
        }
        let mut faces: [Option<FaceEntry>; 6] = Default::default();
        for (key, face) in self.faces {
            let direction = direction_named(&key).ok_or_else(|| format!("the face key `{key}`"))?;
            if let Some(slot) = faces.get_mut(direction as usize) {
                *slot = Some(face.compile()?);
            }
        }
        let shade_direction_override = self
            .shade_direction_override
            .map(|name| {
                direction_named(&name)
                    .ok_or_else(|| format!("the shade_direction_override `{name}`"))
            })
            .transpose()?;
        let light_emission = match self.light_emission {
            None => 0,
            Some(level @ 0..=15) => level as u8,
            Some(level) => return Err(format!("the light_emission {level}")),
        };
        Ok(Element {
            from: corner(self.from)?,
            to: corner(self.to)?,
            rotation: self.rotation.map(RawRotation::compile).transpose()?,
            faces,
            shade_direction_override,
            light_emission,
        })
    }
}

#[derive(Deserialize)]
struct RawFace {
    texture: String,
    uv: Option<Vec<f64>>,
    cullface: Option<String>,
    rotation: Option<i64>,
    tintindex: Option<i64>,
}

impl RawFace {
    fn compile(self) -> Result<FaceEntry, String> {
        let uv = match self.uv.as_deref() {
            None => None,
            Some(&[u1, v1, u2, v2]) => Some([u1, v1, u2, v2]),
            Some(other) => return Err(format!("a uv of {} numbers", other.len())),
        };
        let degrees = self.rotation.unwrap_or(0).rem_euclid(360);
        if degrees % 90 != 0 {
            return Err(format!("the face rotation {degrees}"));
        }
        Ok(FaceEntry {
            texture: self.texture,
            uv,
            cullface: self.cullface.as_deref().and_then(direction_named),
            rotation: (degrees / 90) as u8,
            tint: self.tintindex.and_then(|index| u32::try_from(index).ok()),
        })
    }
}

#[derive(Deserialize)]
struct RawRotation {
    origin: [f64; 3],
    axis: Option<String>,
    angle: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    #[serde(default)]
    rescale: bool,
}

impl RawRotation {
    fn compile(self) -> Result<ElementRotation, String> {
        let turn = if self.axis.is_some() || self.angle.is_some() {
            let (Some(axis), Some(degrees)) = (self.axis, self.angle) else {
                return Err("an axis rotation needs both `axis` and `angle`".to_owned());
            };
            let axis = match axis.to_ascii_lowercase().as_str() {
                "x" => Axis::X,
                "y" => Axis::Y,
                "z" => Axis::Z,
                _ => return Err(format!("the rotation axis `{axis}`")),
            };
            ElementTurn::Axis { axis, degrees }
        } else if self.x.is_some() || self.y.is_some() || self.z.is_some() {
            ElementTurn::Euler(DVec3::new(
                self.x.unwrap_or(0.0),
                self.y.unwrap_or(0.0),
                self.z.unwrap_or(0.0),
            ))
        } else {
            return Err("a rotation with no axis and no angle".to_owned());
        };
        Ok(ElementRotation {
            origin: DVec3::from_array(self.origin),
            turn,
            rescale: self.rescale,
        })
    }
}
