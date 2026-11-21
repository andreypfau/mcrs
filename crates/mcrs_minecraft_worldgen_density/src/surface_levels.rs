use crate::router::NoiseRouter;
use crate::router_key::RouterKey;

const SIDE_BITS: u32 = 7;
const SIDE: usize = 1 << SIDE_BITS;

#[derive(Clone, Copy, Debug)]
struct Known {
    quart_x: i32,
    quart_z: i32,
    level: i32,
}

#[derive(Default, Debug)]
pub struct SurfaceLevels {
    owner: Option<RouterKey>,
    slots: Box<[Option<Known>]>,
}

impl SurfaceLevels {
    pub(crate) fn retarget(&mut self, router: &NoiseRouter) {
        let owner = router.key();
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
        self.owner == Some(router.key())
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
