use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::definition::schema::NoteBlockInstrument;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_core::Direction;
use mcrs_voxel::BlockStateId;

use crate::component::{Connection, DiodeMode, Face, Half, RailShape, instrument_named};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Digit {
    stride: u16,
    count: u16,
}

impl Digit {
    fn read(self, offset: u16) -> u16 {
        offset
            .checked_div(self.stride)
            .and_then(|digit| digit.checked_rem(self.count))
            .unwrap_or(0)
    }

    fn write(self, offset: u16, digit: u16) -> Option<u16> {
        let current = self.read(offset);
        let base = offset.checked_sub(current.checked_mul(self.stride)?)?;
        base.checked_add(digit.checked_mul(self.stride)?)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Property<T> {
    digit: Digit,
    values: Vec<T>,
}

impl<T: Copy + PartialEq> Property<T> {
    fn read(&self, offset: u16) -> Option<T> {
        self.values
            .get(usize::from(self.digit.read(offset)))
            .copied()
    }

    fn write(&self, offset: u16, value: T) -> Option<u16> {
        let index = self.values.iter().position(|known| *known == value)?;
        self.digit.write(offset, u16::try_from(index).ok()?)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BlockLayout {
    base: u16,
    count: u16,
    pub(crate) powered: Option<Property<bool>>,
    pub(crate) lit: Option<Property<bool>>,
    pub(crate) locked: Option<Property<bool>>,
    pub(crate) extended: Option<Property<bool>>,
    pub(crate) short: Option<Property<bool>>,
    pub(crate) power: Option<Property<u8>>,
    pub(crate) delay: Option<Property<u8>>,
    pub(crate) facing: Option<Property<Direction>>,
    pub(crate) face: Option<Property<Face>>,
    pub(crate) mode: Option<Property<DiodeMode>>,
    pub(crate) sticky: Option<Property<bool>>,
    pub(crate) sides: [Option<Property<Connection>>; 4],
    pub(crate) open: Option<Property<bool>>,
    pub(crate) enabled: Option<Property<bool>>,
    pub(crate) triggered: Option<Property<bool>>,
    pub(crate) attached: Option<Property<bool>>,
    pub(crate) disarmed: Option<Property<bool>>,
    pub(crate) inverted: Option<Property<bool>>,
    pub(crate) note: Option<Property<u8>>,
    pub(crate) half: Option<Property<Half>>,
    pub(crate) shape: Option<Property<RailShape>>,
    pub(crate) instrument: Option<Property<NoteBlockInstrument>>,
    pub(crate) links: [Option<Property<bool>>; 4],
}

fn property<T: Copy>(
    entry: &BlockEntry,
    name: &str,
    decode: impl Fn(&PropertyValue) -> Option<T>,
) -> Option<Property<T>> {
    let (property, stride) = entry.digit(name)?;
    let count = u16::try_from(property.values.len()).ok()?;
    let values = property
        .values
        .iter()
        .map(decode)
        .collect::<Option<Vec<T>>>()?;
    Some(Property {
        digit: Digit { stride, count },
        values,
    })
}

fn bool_value(value: &PropertyValue) -> Option<bool> {
    match value {
        PropertyValue::Bool(value) => Some(*value),
        _ => None,
    }
}

fn small_int(value: &PropertyValue) -> Option<u8> {
    match value {
        PropertyValue::Int(value) => u8::try_from(*value).ok(),
        _ => None,
    }
}

fn direction(value: &PropertyValue) -> Option<Direction> {
    let PropertyValue::Str(text) = value else {
        return None;
    };
    Direction::all()
        .into_iter()
        .find(|direction| direction.name() == &**text)
}

fn face(value: &PropertyValue) -> Option<Face> {
    match value {
        PropertyValue::Str(text) => Face::parse(text),
        _ => None,
    }
}

fn mode(value: &PropertyValue) -> Option<DiodeMode> {
    match value {
        PropertyValue::Str(text) => DiodeMode::parse(text),
        _ => None,
    }
}

fn sticky(value: &PropertyValue) -> Option<bool> {
    match value {
        PropertyValue::Str(text) if &**text == "sticky" => Some(true),
        PropertyValue::Str(text) if &**text == "normal" => Some(false),
        _ => None,
    }
}

fn connection(value: &PropertyValue) -> Option<Connection> {
    match value {
        PropertyValue::Str(text) => Connection::parse(text),
        _ => None,
    }
}

fn half(value: &PropertyValue) -> Option<Half> {
    match value {
        PropertyValue::Str(text) => Half::parse(text),
        _ => None,
    }
}

fn rail_shape(value: &PropertyValue) -> Option<RailShape> {
    match value {
        PropertyValue::Str(text) => RailShape::parse(text),
        _ => None,
    }
}

fn instrument(value: &PropertyValue) -> Option<NoteBlockInstrument> {
    match value {
        PropertyValue::Str(text) => instrument_named(text),
        _ => None,
    }
}

impl BlockLayout {
    pub(crate) fn of(entry: &BlockEntry) -> Self {
        Self {
            base: entry.base_state_id.number(),
            count: entry.state_count,
            powered: property(entry, "powered", bool_value),
            lit: property(entry, "lit", bool_value),
            locked: property(entry, "locked", bool_value),
            extended: property(entry, "extended", bool_value),
            short: property(entry, "short", bool_value),
            power: property(entry, "power", small_int),
            delay: property(entry, "delay", small_int),
            facing: property(entry, "facing", direction),
            face: property(entry, "face", face),
            mode: property(entry, "mode", mode),
            sticky: property(entry, "type", sticky),
            sides: [
                property(entry, "north", connection),
                property(entry, "east", connection),
                property(entry, "south", connection),
                property(entry, "west", connection),
            ],
            open: property(entry, "open", bool_value),
            enabled: property(entry, "enabled", bool_value),
            triggered: property(entry, "triggered", bool_value),
            attached: property(entry, "attached", bool_value),
            disarmed: property(entry, "disarmed", bool_value),
            inverted: property(entry, "inverted", bool_value),
            note: property(entry, "note", small_int),
            half: property(entry, "half", half),
            shape: property(entry, "shape", rail_shape),
            instrument: property(entry, "instrument", instrument),
            links: [
                property(entry, "north", bool_value),
                property(entry, "east", bool_value),
                property(entry, "south", bool_value),
                property(entry, "west", bool_value),
            ],
        }
    }

    fn offset(&self, state: BlockStateId) -> Option<u16> {
        let offset = state.number().checked_sub(self.base)?;
        (offset < self.count).then_some(offset)
    }

    fn state(&self, offset: u16) -> Option<BlockStateId> {
        if offset >= self.count {
            return None;
        }
        let number = self.base.checked_add(offset)?;
        BlockStateId::checked(
            number,
            usize::from(self.base).saturating_add(usize::from(self.count)),
        )
    }

    pub(crate) fn read<T: Copy + PartialEq>(
        &self,
        property: Option<&Property<T>>,
        state: BlockStateId,
    ) -> Option<T> {
        property?.read(self.offset(state)?)
    }

    pub(crate) fn write<T: Copy + PartialEq>(
        &self,
        property: Option<&Property<T>>,
        state: BlockStateId,
        value: T,
    ) -> Option<BlockStateId> {
        let offset = property?.write(self.offset(state)?, value)?;
        self.state(offset)
    }

    fn horizontal_index(direction: Direction) -> Option<usize> {
        match direction {
            Direction::North => Some(0),
            Direction::East => Some(1),
            Direction::South => Some(2),
            Direction::West => Some(3),
            Direction::Down | Direction::Up => None,
        }
    }

    pub(crate) fn side(&self, direction: Direction) -> Option<&Property<Connection>> {
        self.sides.get(Self::horizontal_index(direction)?)?.as_ref()
    }

    pub(crate) fn link(&self, direction: Direction) -> Option<&Property<bool>> {
        self.links.get(Self::horizontal_index(direction)?)?.as_ref()
    }
}
