#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WidgetValue<'a> {
    Text(&'a str),
    Number(f64),
    Int(i32),
    Choice(usize),
    Bool(bool),
}
