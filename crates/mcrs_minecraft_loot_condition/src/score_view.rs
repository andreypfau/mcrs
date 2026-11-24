use mcrs_minecraft_predicate::NoWorld;

pub trait ScoreView<E> {
    fn score(&self, _holder: ScoreHolder<'_, E>, _objective: &str) -> Option<i32> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreHolder<'a, E> {
    Entity(&'a E),
    Name(&'a str),
}

impl<E> ScoreView<E> for NoWorld {}
