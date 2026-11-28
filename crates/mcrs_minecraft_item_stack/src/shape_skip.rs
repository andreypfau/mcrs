use mcrs_minecraft_nbt::MAX_DEPTH;

use crate::ComponentType;
use crate::WireError;
use crate::nbt_depth::LIST_TEMPLATE_LEVELS;
use crate::nbt_depth::SLOT_TEMPLATE_LEVELS;
use crate::wire_scan::scan_template;
use crate::wire_value::read_var_int;

#[derive(Clone, Copy)]
pub(crate) struct ShapeSkip {
    pub(crate) component: ComponentType,
    pub(crate) depth: u32,
}

impl ShapeSkip {
    pub(crate) fn bad(self) -> WireError {
        WireError::BadShape(self.component)
    }

    pub(crate) fn var_int(self, input: &mut &[u8]) -> Result<i32, WireError> {
        read_var_int(input, |error| match error.kind() {
            mcrs_minecraft_wire::ErrorKind::UnexpectedEnd => WireError::UnexpectedEnd,
            _ => self.bad(),
        })
    }

    pub(crate) fn length(self, input: &mut &[u8]) -> Result<usize, WireError> {
        usize::try_from(self.var_int(input)?).map_err(|_| self.bad())
    }

    pub(crate) fn bytes(self, input: &mut &[u8], count: usize) -> Result<(), WireError> {
        *input = input.get(count..).ok_or(WireError::UnexpectedEnd)?;
        Ok(())
    }

    pub(crate) fn boolean(self, input: &mut &[u8]) -> Result<bool, WireError> {
        let (byte, rest) = input.split_first().ok_or(WireError::UnexpectedEnd)?;
        *input = rest;
        match byte {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.bad()),
        }
    }

    pub(crate) fn string(self, input: &mut &[u8]) -> Result<(), WireError> {
        let length = self.length(input)?;
        self.bytes(input, length)
    }

    pub(crate) fn nbt(self, input: &mut &[u8]) -> Result<(), WireError> {
        mcrs_minecraft_nbt::split_wire(input)
            .map(drop)
            .map_err(|error| match error.kind() {
                mcrs_minecraft_wire::ErrorKind::UnexpectedEnd => WireError::UnexpectedEnd,
                _ => self.bad(),
            })
    }

    pub(crate) fn optional(
        self,
        input: &mut &[u8],
        skip: impl FnOnce(Self, &mut &[u8]) -> Result<(), WireError>,
    ) -> Result<(), WireError> {
        if self.boolean(input)? {
            skip(self, input)?;
        }
        Ok(())
    }

    pub(crate) fn list(
        self,
        input: &mut &[u8],
        mut skip: impl FnMut(Self, &mut &[u8]) -> Result<(), WireError>,
    ) -> Result<(), WireError> {
        let count = self.length(input)?;
        for _ in 0..count {
            skip(self, input)?;
        }
        Ok(())
    }

    pub(crate) fn holder(
        self,
        input: &mut &[u8],
        inline: impl FnOnce(Self, &mut &[u8]) -> Result<(), WireError>,
    ) -> Result<(), WireError> {
        match self.var_int(input)? {
            0 => inline(self, input),
            id if id > 0 => Ok(()),
            _ => Err(self.bad()),
        }
    }

    pub(crate) fn holder_set(self, input: &mut &[u8]) -> Result<(), WireError> {
        match self.var_int(input)? {
            0 => self.string(input),
            form if form > 0 => {
                for _ in 1..form {
                    self.var_int(input)?;
                }
                Ok(())
            }
            _ => Err(self.bad()),
        }
    }

    pub(crate) fn nested(self) -> Result<Self, WireError> {
        let depth = self.depth.saturating_add(1);
        if depth > MAX_DEPTH {
            return Err(WireError::TooDeep);
        }
        Ok(Self { depth, ..self })
    }

    pub(crate) fn list_template(self, input: &mut &[u8]) -> Result<(), WireError> {
        scan_template(input, self.depth, LIST_TEMPLATE_LEVELS)
    }

    pub(crate) fn slot_template(self, input: &mut &[u8]) -> Result<(), WireError> {
        scan_template(input, self.depth, SLOT_TEMPLATE_LEVELS)
    }
}
