use alloc::borrow::Cow;
use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_text::Text;

use crate::Neutral;

pub(crate) const MAX_LINES: usize = 256;

/// The lines of text below the name in the tooltip.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Lore {
    pub lines: Cow<'static, [Text]>,
}

impl Lore {
    /// Gives `None` for more than 256 lines, which the saved form does not hold.
    #[must_use]
    pub fn new(lines: impl Into<Cow<'static, [Text]>>) -> Option<Self> {
        let lines: Cow<'static, [Text]> = lines.into();
        (lines.len() <= MAX_LINES).then_some(Self { lines })
    }
}

impl Default for Lore {
    fn default() -> Self {
        Self {
            lines: Cow::Borrowed(&[]),
        }
    }
}

// chisle: the hash ignores the text of each line, so lores of one length share a hash and cost
// one more equality check each. A `Hash` on `Text` lifts it.
impl Hash for Lore {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.lines.len());
    }
}

impl Neutral for Lore {
    const NEUTRAL: &'static Self = &Self {
        lines: Cow::Borrowed(&[]),
    };
}
