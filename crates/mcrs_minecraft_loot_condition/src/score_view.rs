use mcrs_minecraft_predicate::NoWorld;

pub trait ScoreView<E> {
    fn score(&self, holder: ScoreHolder<'_, E>, objective: &str) -> Option<i32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreHolder<'a, E> {
    Entity(&'a E),
    Name(&'a str),
}

impl<E> ScoreView<E> for NoWorld {
    fn score(&self, _: ScoreHolder<'_, E>, _: &str) -> Option<i32> {
        None
    }
}
