#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Depth {
    Shape,
    Surface,
}

impl Depth {
    pub fn parse(text: &str) -> Option<Depth> {
        match text {
            "shape" => Some(Depth::Shape),
            "surface" => Some(Depth::Surface),
            _ => None,
        }
    }
}
