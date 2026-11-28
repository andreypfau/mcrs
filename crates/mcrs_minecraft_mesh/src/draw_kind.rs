use crate::block::Pass;
use crate::quad_format::QuadFormat;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct DrawKind {
    pub layer: Pass,
    pub format: QuadFormat,
}

impl DrawKind {
    pub const COUNT: usize = Pass::COUNT * QuadFormat::COUNT;

    pub const ALL: [Self; Self::COUNT] = [
        Self::new(Pass::Solid, QuadFormat::Greedy),
        Self::new(Pass::Solid, QuadFormat::Model),
        Self::new(Pass::Cutout, QuadFormat::Greedy),
        Self::new(Pass::Cutout, QuadFormat::Model),
        Self::new(Pass::Translucent, QuadFormat::Greedy),
        Self::new(Pass::Translucent, QuadFormat::Model),
    ];

    #[must_use]
    pub const fn new(layer: Pass, format: QuadFormat) -> Self {
        Self { layer, format }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.layer as usize * QuadFormat::COUNT + self.format as usize
    }

    #[must_use]
    pub const fn from_index(index: usize) -> Option<Self> {
        let (Some(layer), Some(format)) = (
            Pass::from_index(index / QuadFormat::COUNT),
            QuadFormat::from_index(index % QuadFormat::COUNT),
        ) else {
            return None;
        };
        Some(Self::new(layer, format))
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match (self.layer, self.format) {
            (Pass::Solid, QuadFormat::Greedy) => "solid greedy",
            (Pass::Solid, QuadFormat::Model) => "solid model",
            (Pass::Cutout, QuadFormat::Greedy) => "cutout greedy",
            (Pass::Cutout, QuadFormat::Model) => "cutout model",
            (Pass::Translucent, QuadFormat::Greedy) => "translucent greedy",
            (Pass::Translucent, QuadFormat::Model) => "translucent model",
        }
    }

    #[must_use]
    pub const fn writes_depth(self) -> bool {
        self.layer.writes_depth()
    }

    #[must_use]
    pub const fn translucent(self) -> bool {
        self.layer.translucent()
    }
}
