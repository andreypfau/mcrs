use alloc::boxed::Box;
use alloc::vec;
use core::iter;
use core::mem;

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::world::DeferredWorld;

use crate::Occupant;

const WORD: usize = u64::BITS as usize;

/// The slots of a holder: the stack in each slot, by index.
///
/// The hooks of [`InSlot`](crate::InSlot) keep it equal to the set of stacks whose `InSlot` names
/// this holder. The removal of `Slots`, a new `Slots` in its place and the despawn of the holder
/// despawn the stacks that it holds.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
#[component(on_discard = despawn_stacks)]
pub struct Slots {
    stacks: Box<[Option<Entity>]>,
    occupied: Box<[u64]>,
}

impl Slots {
    /// Empty slots. A size above 65,536 gives 65,536 slots, the count that a `u16` index reaches.
    #[must_use]
    pub fn new(size: usize) -> Self {
        let size = size.min(usize::from(u16::MAX) + 1);
        Self {
            stacks: vec![None; size].into_boxed_slice(),
            occupied: vec![0; size.div_ceil(WORD)].into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.stacks.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stacks.is_empty()
    }

    /// The stack of the slot `index`, or `None` when the slot is empty or does not exist.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Entity> {
        self.stacks.get(index).copied().flatten()
    }

    /// The content of each slot, in the order of the indices.
    pub fn iter(&self) -> impl Iterator<Item = Option<Entity>> + '_ {
        self.stacks.iter().copied()
    }

    /// The stacks, in the order of their slots.
    pub fn occupied(&self) -> impl Iterator<Item = Entity> + '_ {
        self.occupied_indices().filter_map(|index| self.get(index))
    }

    /// The lowest index of an empty slot.
    #[must_use]
    pub fn first_free(&self) -> Option<u16> {
        let index = (self.occupied.iter().enumerate())
            .find_map(|(word, bits)| first_bit(!bits).map(|bit| word * WORD + bit))?;
        if index >= self.len() {
            return None;
        }
        u16::try_from(index).ok()
    }

    /// The stack of the lowest occupied slot.
    #[must_use]
    pub fn first_occupied(&self) -> Option<Occupant> {
        let index = self.occupied_indices().next()?;
        Some(Occupant {
            index: u16::try_from(index).ok()?,
            stack: self.get(index)?,
        })
    }

    /// Puts `stack` in the slot `index` and gives the previous content, or `None` when the slot
    /// does not exist.
    pub(crate) fn replace(&mut self, index: u16, stack: Option<Entity>) -> Option<Option<Entity>> {
        let index = usize::from(index);
        let slot = self.stacks.get_mut(index)?;
        let previous = mem::replace(slot, stack);
        if let Some(bits) = self.occupied.get_mut(index / WORD) {
            let bit = 1u64 << (index % WORD);
            match stack {
                Some(_) => *bits |= bit,
                None => *bits &= !bit,
            }
        }
        Some(previous)
    }

    fn occupied_indices(&self) -> impl Iterator<Item = usize> + '_ {
        (self.occupied.iter().enumerate()).flat_map(|(word, &bits)| {
            let mut bits = bits;
            iter::from_fn(move || {
                let bit = first_bit(bits)?;
                bits &= bits.wrapping_sub(1);
                Some(word * WORD + bit)
            })
        })
    }
}

fn first_bit(bits: u64) -> Option<usize> {
    (bits != 0).then(|| bits.trailing_zeros() as usize)
}

fn despawn_stacks(mut world: DeferredWorld, context: HookContext) {
    let holder = context.entity;
    let size = world.get::<Slots>(holder).map_or(0, Slots::len);
    for index in 0..size {
        let stack = world
            .get::<Slots>(holder)
            .and_then(|slots| slots.get(index));
        if let Some(stack) = stack {
            world.commands().entity(stack).try_despawn();
        }
    }
}
