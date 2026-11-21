use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_core::Direction;
use mcrs_voxel::BlockStateId;

use crate::component::{Connection, DiodeMode, Face};

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

    pub(crate) fn side(&self, direction: Direction) -> Option<&Property<Connection>> {
        let index = match direction {
            Direction::North => 0,
            Direction::East => 1,
            Direction::South => 2,
            Direction::West => 3,
            Direction::Down | Direction::Up => return None,
        };
        self.sides.get(index)?.as_ref()
    }
}
