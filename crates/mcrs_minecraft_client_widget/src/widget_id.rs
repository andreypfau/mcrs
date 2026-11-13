/// The id that a screen gives a widget: unique in its screen, and named in each event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(pub u32);
