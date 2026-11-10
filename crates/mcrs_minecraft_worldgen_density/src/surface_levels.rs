use crate::router::NoiseRouter;
use core::ptr;

const SIDE_BITS: u32 = 7;
const SIDE: usize = 1 << SIDE_BITS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Owner {
    router: usize,
    seed: u64,
}

impl Owner {
    fn of(router: &NoiseRouter) -> Self {
        Self {
            router: ptr::from_ref(router) as usize,
            seed: router.world_seed,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Known {
    quart_x: i32,
    quart_z: i32,
    level: i32,
}

/// The surface levels a worker has sampled, kept so that the fluid field of
/// one column starts with what the fields of its neighbours already paid for.
///
/// A level is a function of the router and the quart column alone, so a kept
/// level is the level a new sample would give. The table is a torus of quart
/// columns with one slot each: its size is fixed, and a quart column that
/// lands on a taken slot replaces what was there.
#[derive(Default, Debug)]
pub struct SurfaceLevels {
    owner: Option<Owner>,
    slots: Box<[Option<Known>]>,
}

impl SurfaceLevels {
    /// One worker fills columns for every dimension, and a level differs by
    /// seed and by graph.
    pub(crate) fn retarget(&mut self, router: &NoiseRouter) {
        let owner = Owner::of(router);
        if self.owner == Some(owner) {
            return;
        }
        self.owner = Some(owner);
        if self.slots.is_empty() {
            self.slots = vec![None; SIDE * SIDE].into_boxed_slice();
        } else {
            self.slots.fill(None);
        }
    }

    pub(crate) fn serves(&self, router: &NoiseRouter) -> bool {
        self.owner == Some(Owner::of(router))
    }

    pub(crate) fn get(&self, quart_x: i32, quart_z: i32) -> Option<i32> {
        let known = self.slots.get(slot(quart_x, quart_z)).copied().flatten()?;
        (known.quart_x == quart_x && known.quart_z == quart_z).then_some(known.level)
    }

    pub(crate) fn put(&mut self, quart_x: i32, quart_z: i32, level: i32) {
        if let Some(kept) = self.slots.get_mut(slot(quart_x, quart_z)) {
            *kept = Some(Known {
                quart_x,
                quart_z,
                level,
            });
        }
    }
}

fn slot(quart_x: i32, quart_z: i32) -> usize {
    let mask = SIDE - 1;
    ((quart_z as usize & mask) << SIDE_BITS) | (quart_x as usize & mask)
}
