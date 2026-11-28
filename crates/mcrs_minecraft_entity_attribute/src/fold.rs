use crate::HomeSlot;
use crate::Modifier;
use crate::ModifierOperation;
use crate::home_slot::RING_SLOTS;

#[must_use]
pub fn fold<I>(base: f64, modifiers: I) -> f64
where
    I: IntoIterator<Item = Modifier>,
    I::IntoIter: Clone,
{
    let modifiers = modifiers.into_iter();
    let mut sum = base;
    in_order(
        of(modifiers.clone(), ModifierOperation::AddValue),
        |amount| {
            sum += amount;
        },
    );
    let mut result = sum;
    in_order(
        of(modifiers.clone(), ModifierOperation::AddMultipliedBase),
        |amount| result += sum * amount,
    );
    in_order(
        of(modifiers, ModifierOperation::AddMultipliedTotal),
        |amount| {
            result *= 1.0 + amount;
        },
    );
    result
}

fn of(
    modifiers: impl Iterator<Item = Modifier>,
    operation: ModifierOperation,
) -> impl Iterator<Item = Slotted<f64>> {
    modifiers
        .filter(move |modifier| modifier.operation == operation)
        .map(|modifier| Slotted {
            slot: modifier.slot,
            value: modifier.amount,
        })
}

struct Slotted<T> {
    slot: HomeSlot,
    value: T,
}

fn in_order<T: Copy>(entries: impl Iterator<Item = Slotted<T>>, mut visit: impl FnMut(T)) {
    let mut ring: [Option<T>; RING_SLOTS as usize] = [None; RING_SLOTS as usize];
    let mut taken: u8 = 0;
    for entry in entries {
        if taken == RING_SLOTS {
            drain(&mut ring, &mut visit);
            taken = 0;
        }
        let free = (0..ring.len())
            .map(|offset| (entry.slot.index() + offset) % ring.len())
            .find(|&index| ring.get(index).is_some_and(Option::is_none));
        if let Some(slot) = free.and_then(|index| ring.get_mut(index)) {
            *slot = Some(entry.value);
            taken += 1;
        }
    }
    drain(&mut ring, &mut visit);
}

fn drain<T>(ring: &mut [Option<T>], visit: &mut impl FnMut(T)) {
    for slot in ring.iter_mut().rev() {
        if let Some(value) = slot.take() {
            visit(value);
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    extern crate std;

    use alloc::string::String;
    use alloc::vec::Vec;

    use mcrs_minecraft_resource::ResourceLocation;
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Fixture {
        order_cases: Vec<(u8, Vec<String>, Vec<String>)>,
    }

    struct Entry<'a> {
        id: ResourceLocation<'a>,
        operation: ModifierOperation,
    }

    fn order(sequence: &[Entry<'_>], operation: ModifierOperation) -> Vec<String> {
        let mut order = Vec::new();
        in_order(
            sequence
                .iter()
                .filter(|entry| entry.operation == operation)
                .map(|entry| Slotted {
                    slot: HomeSlot::from(entry.id),
                    value: entry.id,
                }),
            |id| order.push(alloc::format!("{id}")),
        );
        order
    }

    fn sequence<'a>(steps: &'a [String], operation: ModifierOperation) -> Vec<Entry<'a>> {
        let mut sequence: Vec<Entry<'a>> = Vec::new();
        for step in steps {
            if let Some(removed) = step.strip_prefix('-') {
                let removed = ResourceLocation::read(removed).unwrap();
                sequence.retain(|entry| entry.id != removed);
            } else if let Some((id, other)) = step.split_once('|') {
                let other: u8 = other.parse().unwrap();
                sequence.push(Entry {
                    id: ResourceLocation::read(id).unwrap(),
                    operation: ModifierOperation::try_from(other).unwrap(),
                });
            } else {
                sequence.push(Entry {
                    id: ResourceLocation::read(step).unwrap(),
                    operation,
                });
            }
        }
        sequence
    }

    #[test]
    fn each_step_takes_its_modifiers_in_the_order_of_the_game() {
        let text = mcrs_minecraft_fixtures::fixture("attributes").text();
        let fixture: Fixture = serde_json::from_str(&text).unwrap();
        assert_eq!(fixture.order_cases.len(), 37, "the fixture holds 37 orders");
        for (operation, steps, expected) in &fixture.order_cases {
            let operation = ModifierOperation::try_from(*operation).unwrap();
            let sequence = sequence(steps, operation);
            assert_eq!(
                &order(&sequence, operation),
                expected,
                "{operation:?}, {steps:?}"
            );
        }
    }

    #[test]
    fn more_than_a_ring_of_modifiers_drops_none() {
        let ids: Vec<String> = (0..40)
            .map(|index| alloc::format!("test:m{index}"))
            .collect();
        let operation = ModifierOperation::AddValue;
        let mut taken = order(&sequence(&ids, operation), operation);
        taken.sort();
        let mut expected = ids.clone();
        expected.sort();
        assert_eq!(taken, expected, "each of the 40 modifiers is taken once");
    }
}
