use crate::task_end::TaskId;

#[derive(Debug)]
pub struct InFlight<P, H> {
    tasks: Vec<Running<P, H>>,
}

#[derive(Debug)]
struct Running<P, H> {
    id: TaskId,
    payload: P,
    _handle: H,
}

impl<P, H> Default for InFlight<P, H> {
    fn default() -> Self {
        Self { tasks: Vec::new() }
    }
}

impl<P, H> InFlight<P, H> {
    pub fn push(&mut self, id: TaskId, payload: P, handle: H) {
        self.tasks.push(Running {
            id,
            payload,
            _handle: handle,
        });
    }

    pub fn retire(&mut self, id: TaskId) -> Option<P> {
        let at = self.tasks.iter().position(|task| task.id == id)?;
        Some(self.tasks.swap_remove(at).payload)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &P> {
        self.tasks.iter().map(|task| &task.payload)
    }
}
