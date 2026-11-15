/// A baked int range or float range (LOOT-CND-021).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NumberRange<P> {
    Point(P),
    Bounds { min: Option<P>, max: Option<P> },
}

impl<P> NumberRange<P> {
    pub(crate) fn providers(&self) -> impl Iterator<Item = &P> {
        let (first, second) = match self {
            NumberRange::Point(point) => (Some(point), None),
            NumberRange::Bounds { min, max } => (min.as_ref(), max.as_ref()),
        };
        first.into_iter().chain(second)
    }
}
