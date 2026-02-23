use mcrs_core::block_state::{Property, PropertyDef, PropertyStr, PropertyValue};

const TRUE: PropertyStr = PropertyStr::new("true");
const FALSE: PropertyStr = PropertyStr::new("false");

// ── PropertyDef statics ─────────────────────────────────────────────────

pub static SNOWY: PropertyDef = PropertyDef {
    name: PropertyStr::new("snowy"),
    values: &[TRUE, FALSE],
};

pub static WATERLOGGED: PropertyDef = PropertyDef {
    name: PropertyStr::new("waterlogged"),
    values: &[TRUE, FALSE],
};

pub static POWERED: PropertyDef = PropertyDef {
    name: PropertyStr::new("powered"),
    values: &[TRUE, FALSE],
};

pub static HANGING: PropertyDef = PropertyDef {
    name: PropertyStr::new("hanging"),
    values: &[TRUE, FALSE],
};

pub static UNSTABLE: PropertyDef = PropertyDef {
    name: PropertyStr::new("unstable"),
    values: &[TRUE, FALSE],
};

pub static STAGE: PropertyDef = PropertyDef {
    name: PropertyStr::new("stage"),
    values: &[PropertyStr::new("0"), PropertyStr::new("1")],
};

pub static AGE_4: PropertyDef = PropertyDef {
    name: PropertyStr::new("age"),
    values: &[
        PropertyStr::new("0"),
        PropertyStr::new("1"),
        PropertyStr::new("2"),
        PropertyStr::new("3"),
        PropertyStr::new("4"),
    ],
};

pub static AXIS: PropertyDef = PropertyDef {
    name: PropertyStr::new("axis"),
    values: &[
        PropertyStr::new("x"),
        PropertyStr::new("y"),
        PropertyStr::new("z"),
    ],
};

pub static INSTRUMENT: PropertyDef = PropertyDef {
    name: PropertyStr::new("instrument"),
    values: &[
        PropertyStr::new("harp"),
        PropertyStr::new("basedrum"),
        PropertyStr::new("snare"),
        PropertyStr::new("hat"),
        PropertyStr::new("bass"),
        PropertyStr::new("flute"),
        PropertyStr::new("bell"),
        PropertyStr::new("guitar"),
        PropertyStr::new("chime"),
        PropertyStr::new("xylophone"),
        PropertyStr::new("iron_xylophone"),
        PropertyStr::new("cow_bell"),
        PropertyStr::new("didgeridoo"),
        PropertyStr::new("bit"),
        PropertyStr::new("banjo"),
        PropertyStr::new("pling"),
        PropertyStr::new("zombie"),
        PropertyStr::new("skeleton"),
        PropertyStr::new("creeper"),
        PropertyStr::new("dragon"),
        PropertyStr::new("wither_skeleton"),
        PropertyStr::new("piglin"),
        PropertyStr::new("custom_head"),
    ],
};

pub static NOTE: PropertyDef = PropertyDef {
    name: PropertyStr::new("note"),
    values: &[
        PropertyStr::new("0"),
        PropertyStr::new("1"),
        PropertyStr::new("2"),
        PropertyStr::new("3"),
        PropertyStr::new("4"),
        PropertyStr::new("5"),
        PropertyStr::new("6"),
        PropertyStr::new("7"),
        PropertyStr::new("8"),
        PropertyStr::new("9"),
        PropertyStr::new("10"),
        PropertyStr::new("11"),
        PropertyStr::new("12"),
        PropertyStr::new("13"),
        PropertyStr::new("14"),
        PropertyStr::new("15"),
        PropertyStr::new("16"),
        PropertyStr::new("17"),
        PropertyStr::new("18"),
        PropertyStr::new("19"),
        PropertyStr::new("20"),
        PropertyStr::new("21"),
        PropertyStr::new("22"),
        PropertyStr::new("23"),
        PropertyStr::new("24"),
    ],
};

// ── Typed Property handles ──────────────────────────────────────────────

pub static SNOWY_PROP: Property<bool> = Property::new(&SNOWY);
pub static WATERLOGGED_PROP: Property<bool> = Property::new(&WATERLOGGED);
pub static POWERED_PROP: Property<bool> = Property::new(&POWERED);
pub static HANGING_PROP: Property<bool> = Property::new(&HANGING);
pub static UNSTABLE_PROP: Property<bool> = Property::new(&UNSTABLE);
pub static STAGE_PROP: Property<u8> = Property::new(&STAGE);
pub static AGE_4_PROP: Property<u8> = Property::new(&AGE_4);
pub static NOTE_PROP: Property<u8> = Property::new(&NOTE);
pub static AXIS_PROP: Property<Axis> = Property::new(&AXIS);

// ── Axis enum ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Axis {
    X = 0,
    Y = 1,
    Z = 2,
}

impl PropertyValue for Axis {
    fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Axis::X),
            1 => Some(Axis::Y),
            2 => Some(Axis::Z),
            _ => None,
        }
    }
    fn to_index(self) -> u8 {
        self as u8
    }
}
