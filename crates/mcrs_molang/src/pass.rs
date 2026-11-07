//! Rewrites that keep the bits of every result, for every input: constant folding through the
//! same functions that the VMs call, exact identities, value numbering, select formation and
//! dead code removal. No pass reassociates, contracts or drops a NaN, an infinity or a `-0.0`.

use alloc::vec;
use alloc::vec::Vec;
use core::mem;

use hashbrown::HashMap;

use crate::ir::{Function, Op, Region, ValueId};
use crate::layout::VariableSlot;
use crate::math::{self, BinaryFn, TernaryFn, UnaryFn};
use crate::query::QuerySlot;
use crate::value::ValueType;

/// The most instructions, constants aside, that one arm of a select may evaluate eagerly.
const SELECT_ARM_LIMIT: usize = 8;

pub(crate) fn optimize(function: &mut Function) {
    let mut numbering = Numbering {
        replaced: vec![None; function.values.len()],
        scopes: Vec::new(),
        epochs: HashMap::new(),
        next_epoch: 0,
    };
    let body = mem::take(&mut function.body);
    function.body = numbering.region(function, body);
    let mut body = mem::take(&mut function.body);
    form_selects(function, &mut body);
    let mut live = vec![false; function.values.len()];
    remove_dead(function, &mut body, &mut live);
    function.body = body;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Key {
    Const(u32),
    ConstBits(u64, ValueType),
    This,
    Query(QuerySlot),
    QueryMissing(QuerySlot),
    LoadVar(VariableSlot, u32),
    VarMissing(VariableSlot, u32),
    Unary(UnaryFn, ValueId),
    Binary(BinaryFn, ValueId, ValueId),
    Ternary(TernaryFn, ValueId, ValueId, ValueId),
    CompareBits(bool, ValueId, ValueId),
    Select(ValueId, ValueId, ValueId),
    Table(u16, ValueId),
}

struct Numbering {
    replaced: Vec<Option<ValueId>>,
    scopes: Vec<HashMap<Key, ValueId>>,
    epochs: HashMap<VariableSlot, u32>,
    next_epoch: u32,
}

enum Outcome {
    Keep,
    Drop,
    Replace(ValueId),
    /// A `return` ran; the instruction is in the list already.
    Returned,
    /// An inlined region ended with a `return`; the instruction itself is gone.
    Stop,
}

impl Numbering {
    fn resolve(&self, mut id: ValueId) -> ValueId {
        while let Some(Some(next)) = self.replaced.get(id.index()) {
            id = *next;
        }
        id
    }

    fn region(&mut self, function: &mut Function, region: Region) -> Region {
        self.scopes.push(HashMap::new());
        let mut insts = Vec::with_capacity(region.insts.len());
        self.insts(function, region.insts, &mut insts);
        let yields = region.yields.iter().map(|id| self.resolve(*id)).collect();
        self.scopes.pop();
        Region { insts, yields }
    }

    /// Returns whether a `return` ended the instruction list.
    fn insts(
        &mut self,
        function: &mut Function,
        ids: Vec<ValueId>,
        out: &mut Vec<ValueId>,
    ) -> bool {
        for id in ids {
            match self.inst(function, id, out) {
                Outcome::Keep => out.push(id),
                Outcome::Drop => {}
                Outcome::Replace(with) => {
                    if let Some(slot) = self.replaced.get_mut(id.index()) {
                        *slot = Some(with);
                    }
                }
                Outcome::Returned => {
                    out.push(id);
                    return true;
                }
                Outcome::Stop => return true,
            }
        }
        false
    }

    fn lookup(&self, key: Key) -> Option<ValueId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&key).copied())
    }

    fn epoch(&self, slot: VariableSlot) -> u32 {
        self.epochs.get(&slot).copied().unwrap_or(0)
    }

    fn inst(&mut self, function: &mut Function, id: ValueId, out: &mut Vec<ValueId>) -> Outcome {
        let Some(data) = function.values.get_mut(id.index()) else {
            return Outcome::Drop;
        };
        let mut op = mem::replace(&mut data.op, Op::Phi);
        op.map_operands(|operand| self.resolve(operand));
        let value_type = data.value_type;

        if let Op::If {
            condition,
            then,
            otherwise,
            results,
        } = op
        {
            if let Some(test) = function.constant(condition) {
                let chosen = if math::truthy(test) { then } else { otherwise };
                let returned = self.insts(function, chosen.insts, out);
                for (result, value) in results.iter().zip(&chosen.yields) {
                    let value = self.resolve(*value);
                    if let Some(slot) = self.replaced.get_mut(result.index()) {
                        *slot = Some(value);
                    }
                }
                return if returned {
                    Outcome::Stop
                } else {
                    Outcome::Drop
                };
            }
            let then = self.region(function, then);
            let otherwise = self.region(function, otherwise);
            set_op(
                function,
                id,
                Op::If {
                    condition,
                    then,
                    otherwise,
                    results,
                },
            );
            return Outcome::Keep;
        }

        let outcome = self.simplify(function, &mut op, value_type);
        let key = match &op {
            Op::StoreVar { slot, .. } => {
                self.next_epoch = self.next_epoch.wrapping_add(1);
                self.epochs.insert(*slot, self.next_epoch);
                None
            }
            _ => self.key(&op, value_type),
        };
        let is_return = matches!(op, Op::Return(_));
        set_op(function, id, op);
        if let Some(outcome) = outcome {
            return outcome;
        }
        if let Some(key) = key {
            if let Some(existing) = self.lookup(key) {
                return Outcome::Replace(existing);
            }
            if let Some(scope) = self.scopes.last_mut() {
                scope.insert(key, id);
            }
        }
        if is_return {
            Outcome::Returned
        } else {
            Outcome::Keep
        }
    }

    fn key(&self, op: &Op, value_type: ValueType) -> Option<Key> {
        Some(match *op {
            Op::Const(value) => Key::Const(value.to_bits()),
            Op::ConstBits(bits) => Key::ConstBits(bits, value_type),
            Op::This => Key::This,
            Op::Query(slot) => Key::Query(slot),
            Op::QueryMissing(slot) => Key::QueryMissing(slot),
            Op::LoadVar(slot) => Key::LoadVar(slot, self.epoch(slot)),
            Op::VarMissing(slot) => Key::VarMissing(slot, self.epoch(slot)),
            Op::Unary(function, a) => Key::Unary(function, a),
            Op::Binary(function, a, b) => {
                let (a, b) = if function.commutes() && b < a {
                    (b, a)
                } else {
                    (a, b)
                };
                Key::Binary(function, a, b)
            }
            Op::Ternary(function, a, b, c) => Key::Ternary(function, a, b, c),
            Op::CompareBits { negate, a, b } => {
                let (a, b) = if b < a { (b, a) } else { (a, b) };
                Key::CompareBits(negate, a, b)
            }
            Op::Select(a, b, c) => Key::Select(a, b, c),
            Op::Table(table, index) => Key::Table(table, index),
            _ => return None,
        })
    }

    /// Folds constants and applies the exact identities. `None` means that the instruction
    /// stays, possibly rewritten.
    fn simplify(&self, function: &Function, op: &mut Op, value_type: ValueType) -> Option<Outcome> {
        let constant = |id: ValueId| function.constant(id);
        let bits = |id: ValueId| match function.op(id) {
            Some(Op::ConstBits(bits)) => Some(*bits),
            _ => None,
        };
        match *op {
            Op::MissOr(flag) if constant(flag).is_some_and(|flag| !math::truthy(flag)) => {
                return Some(Outcome::Drop);
            }
            Op::Unary(function_kind, a) => {
                if let Some(a) = constant(a) {
                    *op = Op::Const(function_kind.apply(a));
                } else if function_kind == UnaryFn::Truthy && is_boolean(function, a) {
                    return Some(Outcome::Replace(a));
                }
            }
            Op::Binary(function_kind, a, b) => match (constant(a), constant(b)) {
                (Some(a), Some(b)) => *op = Op::Const(function_kind.apply(a, b)),
                (_, Some(k)) if is_right_identity(function_kind, k) => {
                    return Some(Outcome::Replace(a));
                }
                (Some(k), _) if is_left_identity(function_kind, k) => {
                    return Some(Outcome::Replace(b));
                }
                _ => {}
            },
            Op::Ternary(function_kind, a, b, c) => {
                if let (Some(a), Some(b), Some(c)) = (constant(a), constant(b), constant(c)) {
                    *op = Op::Const(function_kind.apply(a, b, c));
                }
            }
            Op::CompareBits { negate, a, b } => {
                if let (Some(a), Some(b)) = (bits(a), bits(b)) {
                    *op = Op::Const(math::number((a == b) != negate));
                }
            }
            Op::Select(condition, a, b) => {
                if let Some(test) = constant(condition) {
                    return Some(Outcome::Replace(if math::truthy(test) { a } else { b }));
                }
            }
            Op::Table(table, index) => {
                if let (Some(index), Some(elements)) =
                    (constant(index), function.tables.get(usize::from(table)))
                    && let Some(raw) = math::table_element(elements, index)
                {
                    *op = if value_type.is_bits() {
                        Op::ConstBits(raw)
                    } else {
                        Op::Const(f32::from_bits(raw as u32))
                    };
                }
            }
            _ => {}
        }
        None
    }
}

fn set_op(function: &mut Function, id: ValueId, op: Op) {
    if let Some(data) = function.values.get_mut(id.index()) {
        data.op = op;
    }
}

fn is_right_identity(function: BinaryFn, k: f32) -> bool {
    match function {
        BinaryFn::Multiply | BinaryFn::Divide => k.to_bits() == 1.0f32.to_bits(),
        BinaryFn::Add => k.to_bits() == (-0.0f32).to_bits(),
        BinaryFn::Subtract => k.to_bits() == 0.0f32.to_bits(),
        _ => false,
    }
}

fn is_left_identity(function: BinaryFn, k: f32) -> bool {
    match function {
        BinaryFn::Multiply => k.to_bits() == 1.0f32.to_bits(),
        BinaryFn::Add => k.to_bits() == (-0.0f32).to_bits(),
        _ => false,
    }
}

/// Whether the value is always `0.0` or `1.0`.
fn is_boolean(function: &Function, id: ValueId) -> bool {
    match function.op(id) {
        Some(Op::Const(value)) => value.to_bits() == 0 || value.to_bits() == 1.0f32.to_bits(),
        Some(Op::Unary(UnaryFn::Not | UnaryFn::Truthy, _))
        | Some(Op::CompareBits { .. })
        | Some(Op::Ternary(TernaryFn::InRange, ..))
        | Some(Op::QueryMissing(_) | Op::VarMissing(_) | Op::MissTake) => true,
        Some(Op::Binary(function_kind, ..)) => matches!(
            function_kind,
            BinaryFn::Less
                | BinaryFn::LessEqual
                | BinaryFn::Greater
                | BinaryFn::GreaterEqual
                | BinaryFn::Equal
                | BinaryFn::NotEqual
                | BinaryFn::BothTruthy
                | BinaryFn::EitherTruthy
                | BinaryFn::ApproxEqual
        ),
        _ => false,
    }
}

/// Turns an `If` whose arms are pure and short into the arms' instructions and a `Select` for
/// each result. Both arms then run for every lane, which no input can observe.
fn form_selects(function: &mut Function, region: &mut Region) {
    let insts = mem::take(&mut region.insts);
    let mut out = Vec::with_capacity(insts.len());
    for id in insts {
        let Some(Op::If { .. }) = function.op(id) else {
            out.push(id);
            continue;
        };
        let Some(data) = function.values.get_mut(id.index()) else {
            continue;
        };
        let Op::If {
            condition,
            mut then,
            mut otherwise,
            results,
        } = mem::replace(&mut data.op, Op::Phi)
        else {
            continue;
        };
        form_selects(function, &mut then);
        form_selects(function, &mut otherwise);
        if is_cheap_and_pure(function, &then) && is_cheap_and_pure(function, &otherwise) {
            out.extend(&then.insts);
            out.extend(&otherwise.insts);
            for (index, result) in results.iter().enumerate() {
                let (Some(a), Some(b)) = (then.yields.get(index), otherwise.yields.get(index))
                else {
                    continue;
                };
                set_op(function, *result, Op::Select(condition, *a, *b));
                out.push(*result);
            }
            continue;
        }
        set_op(
            function,
            id,
            Op::If {
                condition,
                then,
                otherwise,
                results,
            },
        );
        out.push(id);
    }
    region.insts = out;
}

fn is_cheap_and_pure(function: &Function, region: &Region) -> bool {
    let mut cost = 0;
    for id in &region.insts {
        match function.op(*id) {
            Some(Op::Const(_) | Op::ConstBits(_)) => {}
            Some(Op::If { .. }) | None => return false,
            Some(_) if function.has_effect(*id) => return false,
            Some(_) => cost += 1,
        }
    }
    cost <= SELECT_ARM_LIMIT
}

fn mark(live: &mut [bool], id: ValueId) {
    if let Some(flag) = live.get_mut(id.index()) {
        *flag = true;
    }
}

fn is_live(live: &[bool], id: ValueId) -> bool {
    live.get(id.index()).copied().unwrap_or(false)
}

fn remove_dead(function: &mut Function, region: &mut Region, live: &mut [bool]) {
    let insts = mem::take(&mut region.insts);
    let mut kept = Vec::with_capacity(insts.len());
    for id in insts.into_iter().rev() {
        let Some(data) = function.values.get_mut(id.index()) else {
            continue;
        };
        if matches!(data.op, Op::If { .. })
            && let Op::If {
                condition,
                mut then,
                mut otherwise,
                results,
            } = mem::replace(&mut data.op, Op::Phi)
        {
            let mut any_result = false;
            for (index, result) in results.iter().enumerate() {
                if is_live(live, *result) {
                    any_result = true;
                    if let Some(value) = then.yields.get(index) {
                        mark(live, *value);
                    }
                    if let Some(value) = otherwise.yields.get(index) {
                        mark(live, *value);
                    }
                }
            }
            remove_dead(function, &mut then, live);
            remove_dead(function, &mut otherwise, live);
            let effect =
                function.region_has_effect(&then) || function.region_has_effect(&otherwise);
            set_op(
                function,
                id,
                Op::If {
                    condition,
                    then,
                    otherwise,
                    results,
                },
            );
            if any_result || effect {
                mark(live, condition);
                kept.push(id);
            }
            continue;
        }
        let needed = is_live(live, id) || function.has_effect(id);
        if needed {
            let operands = function.op(id).map(Op::operands).unwrap_or_default();
            for operand in operands.as_slice() {
                mark(live, *operand);
            }
            kept.push(id);
        }
    }
    kept.reverse();
    region.insts = kept;
}
