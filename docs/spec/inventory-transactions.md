# Inventory transactions: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| World version | `5127` |

Source of these values: `assets/minecraft/version.json`.

This document states the request: an ordered list of operations on slots that applies in full or
not at all. The request does not depend on an edition. This document also states how a click of
Java Edition and a stack request of Bedrock Edition become operations. It does not prescribe an
architecture.

Scope: the operations, their conditions and results, the rejection reasons, and atomicity.
Section 8 lists what this document does not state. `item-stack.md` states the stack. This document
refers to its rules by id.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**: as `login.md`,
  section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **maximum stack size**, **strict validation**: as
  `item-stack.md`, section 0.1, defines them.
- **Holder**: an object that has a fixed number of slots, for example a player or a chest.
- **Slot**: one place of a holder. A slot holds one stack or the empty stack.
- **Slot reference**: a holder and the index of one of its slots.
- **Cursor**: the holder that holds the stack that a player carries with the pointer. Each player
  has one cursor. The cursor has one slot, with the index 0.
- **Operation**: one of `split`, `merge`, `swap` and `drop` (section 3).
- **Request**: a request id and an ordered list of operations (ITX-MOD-001).
- **Request id**: a number that the sender of a request selects. The result repeats it.
- **Working state**: the stacks of the slots while the server examines a request (ITX-ATM-001).
- **Slot limit**: the largest count that a slot can hold (ITX-LIM-001).
- **Capacity**: the largest count of one stack that a slot can hold (ITX-LIM-002).
- **Take filter**: a condition of a slot that permits the removal of items (ITX-LIM-004).
- **Place filter**: a condition of a slot that permits a given stack to enter (ITX-LIM-005).
- **Dropped stack**: a stack that a `drop` operation removes from the slots (ITX-OP-020).
- **Rejection**: the result of a request that does not apply. It has a reason and the index of an
  operation.
- **Edge**: the part of the server that changes the packets of one edition into requests.
- **Clicked slot**: the slot that a `container_click` packet names.
- **Menu**: the ordered set of slots that a player has open. A later specification defines it.
- **Drag**: the sequence of `container_click` packets of the click kind quick craft.

### 0.2 Provenance

- `black_box_confirmed`: shown by an artefact that the game writes.
- `source_suggested_black_box_confirmed`: taken from the reference behaviour, and consistent with
  an artefact that the game writes.
- `source_suggested`: taken from the reference behaviour, with no result. The confidence is at
  most medium. Section 11 attaches an experiment.
- `documentation`: taken from public documentation of a protocol.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 10 collects these items.

No experiment of section 11 has a recorded run. Each one is a proposal.

### 0.3 Artefacts cited

- `assets/mcrs/reports/packets.json`: the packet id report of the game. It gives the packet names
  `container_click`, `container_set_slot`, `container_set_content` and `set_cursor_item`.
- The public protocol documentation of Bedrock Edition, repository
  `Mojang/bedrock-protocol-docs`, for protocol version 2225. Section 7 uses its names.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ITX-MOD` | the request and its inputs |
| `ITX-LIM` | the slot limit, the capacity and the filters |
| `ITX-OP` | the operations |
| `ITX-REJ` | the rejection reasons and their order |
| `ITX-ATM` | atomicity and the result |
| `ITX-JAV` | the clicks of Java Edition |
| `ITX-BED` | the stack requests of Bedrock Edition |

### 0.5 Notation

`X` and `Y` are slot references. An integer range `a..b` includes `a` and `b`. Each value below
reads the working state.

| Notation | Meaning |
|----------|---------|
| `count(X)` | the count of the stack in `X`, or 0 when `X` holds the empty stack |
| `max(X)` | the maximum stack size of the stack in `X` (STK-CMP-001) |
| `limit(X)` | the slot limit of `X` |
| `cap(X, Y)` | the capacity of `X` for the stack in `Y`: the smaller of `limit(X)` and `max(Y)` |
| `same(X, Y)` | `X` and `Y` hold stacks that are not empty and that have the same item and components (STK-EQ-001) |
| `take(X)` | the take filter of `X` permits the removal |
| `place(X, Y)` | the place filter of `X` permits the stack in `Y` |
| `C` | the slot of the cursor of the player |
| `S` | the clicked slot |

## 1. The request

**ITX-MOD-001** (`design`, semantic). A request has a request id and an ordered list of
operations. The list can be empty. Each operation names its slots by slot reference. One request
can name slots of more than one holder. Reason: a click and a stack request of each edition are
such a list.

**ITX-MOD-002** (`design`, semantic). The cursor is a holder. An operation names the cursor slot
as it names each other slot. No operation is special for the cursor. A move from a slot to the
cursor is a take. A move from the cursor to a slot is a place. Reason: the two editions then use
the same operations.

**ITX-MOD-003** (`design`, semantic). A request has these inputs, and no other input:

- the stack of each slot that the request names;
- the slot limit of each such slot (ITX-LIM-001);
- the take filter and the place filter of each such slot (ITX-LIM-004, ITX-LIM-005).

The result of a request is a function of the operations and of these inputs. It does not depend
on time, on a random number or on the edition of the sender.

**ITX-MOD-004** (`design`, semantic). A request has at most 128 operations. Reason: the longest
list that a click of section 6 gives has 99 operations (ITX-JAV-011, ITX-JAV-014). A fixed
maximum gives a fixed cost for each request. See open question Q2.

**ITX-MOD-005** (`design`, semantic). The server applies the requests of one player in the order
in which they arrive. This document does not state the order of the requests of two players in
one tick. A parity test must not compare that order.

**ITX-MOD-006** (`design`, semantic). An operation does not state a stack that the sender
expects in a slot. An edge that receives such data examines it before it makes the request
(ITX-JAV-004, ITX-BED-003).

## 2. Limits and filters

**ITX-LIM-001** (`source_suggested`, semantic). Each slot has a slot limit in the range `1..99`.
The specification of the holder gives the slot limit. The slot limit is 99 when that
specification gives none. The slot limit of the cursor slot is 99. See experiment E1.

**ITX-LIM-002** (`source_suggested`, semantic). The capacity of a slot for a stack is the smaller
of the slot limit and the maximum stack size of that stack. A `split` and a `merge` must not make
a count larger than the capacity. See experiment E1.

**ITX-LIM-003** (`design`, unspecified). A stack can have a count larger than its maximum stack
size (STK-CNT-002). A `swap` moves such a stack as it is (ITX-OP-012). The result of a `split` or
a `merge` that has such a stack in its source slot is not specified beyond ITX-LIM-002. Reason:
strict validation rejects such a stack, and the vanilla client cannot make one.

**ITX-LIM-004** (`source_suggested`, semantic). Each slot has a take filter. The take filter
permits or refuses the removal of items from the slot by the player. The specification of the
holder gives the take filter. The take filter permits when that specification gives none. The
take filter of the cursor slot always permits. See experiment E2.

**ITX-LIM-005** (`source_suggested`, semantic). Each slot has a place filter. For a given stack,
the place filter permits or refuses that stack in the slot. The count of the stack has no part in
the place filter. The specification of the holder gives the place filter. The place filter
permits when that specification gives none. The place filter of the cursor slot always permits.
See experiment E2.

**ITX-LIM-006** (`design`, semantic). A filter reads only the slot, the player and the stack. A
filter must give the same answer for each operation of one request. Reason: the result of a
request must not depend on the order in which the server reads the filters.

## 3. The operations

Each operation has conditions and a result. When a condition is false, the request has a rejection
with the reason that ITX-REJ-002 gives. The result changes the working state.

### 3.1 Split

**ITX-OP-001** (`design`, semantic). `split(from, to, n)` moves exactly `n` items from the slot
`from` to the slot `to`. It moves `n` items or it is a rejection. It never moves a smaller number.

**ITX-OP-002** (`design`, semantic). A `split` applies when each of these conditions is true:

1. `from` and `to` exist, and they are different slots.
2. `n` is 1 or more.
3. `from` holds a stack that is not empty, and `n` is not larger than `count(from)`.
4. `take(from)` is true.
5. `to` holds the empty stack, or `same(from, to)` is true.
6. `place(to, from)` is true.
7. `count(to) + n` is not larger than `cap(to, from)`.

**ITX-OP-003** (`design`, semantic). The result of a `split` is:

- `count(from)` decreases by `n`. When it becomes 0, `from` holds the empty stack (STK-EMP-001).
- When `to` held the empty stack, `to` holds a new stack with the count `n`. Its item and its
  components are those of the stack of `from`.
- When `to` held a stack, `count(to)` increases by `n`.

**ITX-OP-004** (`design`, semantic). A `split` does not use the condition of STK-EQ-003. Two
stacks with the same item and components can join when the capacity permits. Reason: a click of
the reference joins such stacks (ITX-JAV-008). The difference shows only on a stack that strict
validation rejects. See open question Q4 and experiment E3.

### 3.2 Merge

**ITX-OP-005** (`design`, semantic). `merge(from, to)` moves as many items as `to` can accept from
`from` to `to`. The number is `m`, the smaller of `count(from)` and `cap(to, from) - count(to)`.
The items that do not move stay in `from`.

**ITX-OP-006** (`design`, semantic). A `merge` applies when each of these conditions is true:

1. `from` and `to` exist, and they are different slots.
2. `from` holds a stack that is not empty.
3. `take(from)` is true.
4. `to` holds the empty stack, or `same(from, to)` is true.
5. `place(to, from)` is true.
6. `m` is 1 or more.

**ITX-OP-007** (`design`, semantic). The result of a `merge` is the result of `split(from, to, m)`
(ITX-OP-003). A `merge` that can move no item is a rejection, not an operation without effect.
Reason: the sender then learns that its view of the slots is wrong.

### 3.3 Swap

**ITX-OP-010** (`design`, semantic). `swap(left, right)` exchanges the stacks of two slots. Each
stack keeps its item, its components and its count. One of the two slots can hold the empty
stack. The operation then moves one stack to an empty slot.

**ITX-OP-011** (`design`, semantic). A `swap` applies when each of these conditions is true:

1. `left` and `right` exist, and they are different slots.
2. At least one of the two slots holds a stack that is not empty.
3. The take filter permits for each of the two slots that holds a stack.
4. `place(right, left)` is true when `left` holds a stack.
5. `place(left, right)` is true when `right` holds a stack.
6. `count(left)` is not larger than `limit(right)`.
7. `count(right)` is not larger than `limit(left)`.

**ITX-OP-012** (`design`, semantic). A `swap` compares the count with the slot limit, not with the
capacity. It does not join two stacks, also when `same(left, right)` is true. Reason: a `swap`
makes no new count, so the maximum stack size has no part.

### 3.4 Drop

**ITX-OP-020** (`design`, semantic). `drop(from, n)` removes exactly `n` items from the slot
`from`. The removed items are one dropped stack, with the item and the components of the stack of
`from` and the count `n`. The request gives its dropped stacks to the caller in the order of the
operations. Two `drop` operations give two dropped stacks.

**ITX-OP-021** (`design`, semantic). A `drop` applies when each of these conditions is true:

1. `from` exists.
2. `n` is 1 or more.
3. `from` holds a stack that is not empty, and `n` is not larger than `count(from)`.
4. `take(from)` is true.

**ITX-OP-022** (`design`, semantic). The result of a `drop` is that `count(from)` decreases by
`n`. When it becomes 0, `from` holds the empty stack. This document does not state where a dropped
stack goes in the world (section 8).

## 4. Rejection

**ITX-REJ-001** (`design`, semantic). A request that the server examines has one of two results:
applied, or a rejection. A rejection has one reason from this closed list, and the index of one
operation. The index of the first operation is 0.

| Reason | Condition | Operations |
|--------|-----------|------------|
| `too_many_ops` | the request has more than 128 operations (ITX-MOD-004) | the request |
| `no_such_slot` | a slot reference names a holder that does not exist, or an index that the holder does not have | all |
| `same_slot` | the two slot references of one operation are equal | `split`, `merge`, `swap` |
| `bad_count` | `n` is 0 | `split`, `drop` |
| `empty_source` | `from` holds the empty stack; for a `swap`, the two slots hold the empty stack | all |
| `not_enough` | `n` is larger than `count(from)` | `split`, `drop` |
| `take_denied` | the take filter refuses for a slot that loses items | all |
| `not_equal` | `to` holds a stack, and `same(from, to)` is false | `split`, `merge` |
| `place_denied` | the place filter of a slot refuses the stack that enters | `split`, `merge`, `swap` |
| `no_room` | the count is larger than the capacity (`split`), `m` is 0 (`merge`), or a count is larger than a slot limit (`swap`) | `split`, `merge`, `swap` |

No other reason exists. An edge can refuse to make a request from a packet. That refusal is not a
rejection of this document (ITX-MOD-006).

**ITX-REJ-002** (`design`, semantic). When more than one reason applies to one operation, the
rejection has the reason that comes first in this order:

1. `no_such_slot`
2. `same_slot`
3. `bad_count`
4. `empty_source`
5. `not_enough`
6. `take_denied`
7. `not_equal`
8. `place_denied`
9. `no_room`

This is the order of the table of ITX-REJ-001 without `too_many_ops`. Reason: the checks that
need no stack come first, and a filter comes before the check that needs it.

**ITX-REJ-003** (`design`, semantic). The server examines `too_many_ops` before each other
reason. The index of that rejection is 128, the index of the first operation past the maximum.

**ITX-REJ-004** (`design`, semantic). The server examines the operations in the order of the
list. The rejection names the first operation that has a false condition. The server does not
examine the operations after it.

**ITX-REJ-005** (`design`, semantic). For a reason that applies to two slots of one operation,
the first slot reference of the operation decides before the second. This has no effect on the
reason or on the index. It has an effect only on a log.

## 5. Atomicity and the result

**ITX-ATM-001** (`design`, semantic). The server examines each operation against the working
state. The working state starts as the stacks of the slots when the server starts the request.
The result of each operation that applies changes the working state for the operations after it.
Thus a later operation can use a stack that an earlier operation moved.

**ITX-ATM-002** (`design`, semantic). A request applies in full or not at all. When each
operation applies, the stacks of the slots become the working state, and the caller gets the
dropped stacks. When one operation is a rejection, no slot changes and no dropped stack exists.

**ITX-ATM-003** (`design`, semantic). No other change to the slots that the request names occurs
between the first operation and the last operation of one request.

**ITX-ATM-004** (`design`, semantic). A request with an empty list is applied. It changes no slot.

**ITX-ATM-005** (`design`, semantic). An applied request keeps the items. For each item and
component patch, this sum is the same before and after the request: the counts of the slots that
the request names, plus the counts of the dropped stacks. No operation of this document makes or
destroys an item.

**ITX-ATM-006** (`design`, semantic). The result of a request has the request id, and one of
these two:

- applied: each slot whose stack changed, with the stack before and the stack after, and the
  dropped stacks in order;
- rejection: the reason and the index of the operation.

**ITX-ATM-007** (`design`, semantic). After a rejection, the client must get the stack that the
server holds for each slot that the request names. The edge of each edition sends it
(ITX-JAV-005, ITX-BED-004). Reason: the client showed its own result before the answer came.

## 6. Java Edition: the click

A `container_click` packet has a container id, a state id, a slot index, a button and a click
kind. It also has a list of changed slots and a cursor stack, as hashed stacks (STK-HSH-001). The edge
reads the slots and makes one request for each packet. The operations of that request apply by
construction, because the edge computes them from the stacks that it reads.

### 6.1 The packet and the answer

**ITX-JAV-001** (`source_suggested`, semantic). The server does these checks on the packet, in
this order. The first check that fails ends the work on the packet.

| # | Check | When it fails |
|---|-------|---------------|
| 1 | The container id is the id of the menu that the player has open. | The server does nothing and sends nothing. |
| 2 | The player is not a spectator and is not dead. | The server changes no slot. It sends each slot of the menu and the cursor stack. |
| 3 | The menu is valid for the player. A later specification states this check. | The server does nothing and sends nothing. |
| 4 | The slot index is -1, -999, or the index of a slot of the menu. | The server does nothing and sends nothing. |

See experiment E4.

**ITX-JAV-002** (`source_suggested`, semantic). After the checks, the edge makes the request of
the click kind (ITX-JAV-006) and the server applies it. A state id that is not the state id of
the server does not stop the click. See experiment E5.

**ITX-JAV-003** (`design`, semantic). The edge reads the slots and the server applies the request
with no other change to those slots between the two. A request of a click that the edge makes
correctly has no rejection. A rejection of such a request shows a defect of the edge.

**ITX-JAV-004** (`source_suggested`, client agreement). After the request, the server compares
the packet with the slots. A stack of the server matches a hashed stack of the packet as
STK-HSH-003 states.

| Condition | What the server sends |
|-----------|-----------------------|
| The state id of the packet is not the state id of the server. | Each slot of the menu and the cursor stack, in one `container_set_content` packet. |
| The state id is equal, and a slot does not match what the client holds for it. | That slot, in a `container_set_slot` packet. |
| The state id is equal, and the cursor stack does not match. | The cursor stack, in a `set_cursor_item` packet. |
| The state id is equal, and each slot and the cursor stack match. | Nothing. |

For a slot that the packet lists as changed, the client holds the hashed stack of the packet. For
each other slot, the client holds the stack that the server sent last. See experiment E5.

**ITX-JAV-005** (`design`, semantic). When a request of a click has a rejection, the server
changes no slot and then does ITX-JAV-004. The client thus gets each stack that differs from its
own result. The server does not disconnect the player. Reason: ITX-ATM-007, and the reference
also only corrects the client.

### 6.2 The click kinds

**ITX-JAV-006** (`source_suggested`, semantic). Each click kind gives the operations of this
table. A click whose condition is false gives a request with an empty list.

| Click kind | Wire value | Buttons | Operations | Rule |
|------------|------------|---------|------------|------|
| pick up | 0 | 0 primary, 1 secondary | `swap`, `split` or `drop` between `S` and `C`; at most 1 | ITX-JAV-007, ITX-JAV-008 |
| quick move | 1 | 0, 1 | `split` from `S` to slots of the menu, or the `drop` of ITX-JAV-007 | ITX-JAV-009 |
| swap with a hotbar key | 2 | `0..8` hotbar slot, 40 off hand | `swap` or `split` between `S` and a slot of the player; at most 2 | ITX-JAV-010 |
| clone | 3 | not used | none; the click makes items | ITX-JAV-012 |
| throw | 4 | 0 one item, other values the full stack | `drop` from `S` | ITX-JAV-013 |
| quick craft | 5 | stage and drag kind (ITX-JAV-015) | `split` from `C` to each slot of the drag, on the last packet | ITX-JAV-014 to ITX-JAV-018 |
| pick up all | 6 | 0 forward, other values backward | `split` from slots of the menu to `C` | ITX-JAV-011 |

A click kind with a button that the table does not list gives an empty list. See experiment E6.

**ITX-JAV-007** (`source_suggested`, semantic). Pick up and quick move with the slot index -999
drop from the cursor. When `C` holds the empty stack, the list is empty. With button 0 the list
is `drop(C, count(C))`. With button 1 the list is `drop(C, 1)`. Pick up and quick move with the
slot index -1 give an empty list. See experiment E6.

**ITX-JAV-008** (`source_suggested`, semantic). Pick up on a slot of the menu gives the operation
of the first row of this table whose condition is true. `k` is `count(C)` with button 0, and 1
with button 1.

| # | Condition | Operation |
|---|-----------|-----------|
| 1 | An item of `C` or of `S` takes the click for itself. A later specification states these items. | none in this document |
| 2 | `S` and `C` hold the empty stack. | none |
| 3 | `S` holds the empty stack, and `place(S, C)` is false. | none |
| 4 | `S` holds the empty stack. | `split(C, S, n)`, where `n` is the smaller of `k` and `cap(S, C)` |
| 5 | `take(S)` is false. | none |
| 6 | `C` holds the empty stack, button 0. | `swap(S, C)` |
| 7 | `C` holds the empty stack, button 1. | `split(S, C, n)`, where `n` is `count(S)` divided by 2, rounded up |
| 8 | `place(S, C)` is true, and `same(S, C)` is true. | `split(C, S, n)`, where `n` is the smaller of `k` and `cap(S, C) - count(S)` |
| 9 | `place(S, C)` is true, and `count(C)` is not larger than `cap(S, C)`. | `swap(S, C)` |
| 10 | `place(S, C)` is true. | none |
| 11 | `same(S, C)` is true. | `split(S, C, n)`, where `n` is the smaller of `count(S)` and `max(C) - count(C)` |
| 12 | each other case | none |

When `n` is 0 or less, the click gives no operation. In row 11, the click gives no operation
when `n` is smaller than `count(S)` and `place(S, S)` is false. See experiments E3, E7 and E8.

**ITX-JAV-009** (`source_suggested`, semantic). Quick move on a slot of the menu gives no
operation when `take(S)` is false or `S` holds the empty stack. In each other case, the
specification of the menu gives an ordered list of destination slots for `S`. The edge then makes
the operations in two steps:

1. When the stack of `S` can stack (STK-EQ-003), for each destination `D` in order where
   `same(S, D)` and `place(D, S)` are true: `split(S, D, n)`. `n` is the smaller of `count(S)` and
   `cap(D, D) - count(D)`. The step stops when `S` holds the empty stack.
2. When `S` still holds a stack, for the first destination `D` in order that holds the empty
   stack and for which `place(D, S)` is true: `split(S, D, n)`. `n` is the smaller of `count(S)`
   and `cap(D, S)`.

Deviation D4 applies to the place filter in step 1. A `split` with `n` of 0 or less is not in the list. The
destination order of each menu, and the repeat of a quick move, are not in this document
(section 8). See experiment E9.

**ITX-JAV-010** (`source_suggested`, semantic). Swap with a hotbar key uses the slot `H` of the
player that the button names. The click gives the operations of the first row whose condition is
true.

| # | Condition | Operations |
|---|-----------|------------|
| 1 | The slot index is negative. | none (deviation D1) |
| 2 | `S` and `H` hold the empty stack. | none |
| 3 | `H` holds the empty stack. | `swap(S, H)` when `take(S)` is true; none in the other case |
| 4 | `S` holds the empty stack, and `place(S, H)` is false. | none |
| 5 | `S` holds the empty stack, and `count(H)` is not larger than `cap(S, H)`. | `swap(H, S)` |
| 6 | `S` holds the empty stack. | `split(H, S, cap(S, H))` |
| 7 | `take(S)` is false, or `place(S, H)` is false. | none |
| 8 | `count(H)` is not larger than `cap(S, H)`. | `swap(S, H)` |
| 9 | each other case | The stack of `S` goes to the inventory of the player, then `split(H, S, cap(S, H))`. |

In row 9, the stack of `S` enters the inventory of the player by the insert rule of the player
inventory. The part that does not fit becomes a dropped stack. A later specification states that
rule (section 8). See experiment E10.

**ITX-JAV-011** (`source_suggested`, semantic). Pick up all gives operations when each of these
conditions is true: the slot index is 0 or more, `C` holds a stack, and `S` holds the empty stack
or `take(S)` is false. The edge visits the slots of the menu two times. With button 0 the order
is from the first slot to the last slot. With each other button the order is from the last slot
to the first slot. A visit stops when `count(C)` is equal to `max(C)`. For each slot `X` where
each condition of the list below is true, the edge adds `split(X, C, n)`. `n` is the smaller of
`count(X)` and `max(C) - count(C)`.

- `same(X, C)` is true, and `count(X)` is not larger than `max(C)`.
- `take(X)` is true.
- The menu permits pick up all from `X`. A later specification states this condition.
- In the first visit only: `count(X)` is not equal to `max(X)`.
- `n` is equal to `count(X)`, or `place(X, X)` is true.

The list has at most 98 operations, because each `split` adds 1 or more to `count(C)`. See
experiment E11.

**ITX-JAV-012** (`source_suggested`, semantic). Clone has an effect only when each of these
conditions is true: the player has infinite materials (creative mode), `C` holds the empty stack,
the slot index is 0 or more, and `S` holds a stack. The cursor then holds a copy of the stack of
`S` with the count `max(S)`. `S` does not change. This click makes items, so no operation of this
document states it (ITX-ATM-005). See open question Q1 and experiment E12.

**ITX-JAV-013** (`source_suggested`, semantic). Throw gives an operation when each of these
conditions is true: `C` holds the empty stack, the slot index is 0 or more, `S` holds a stack,
`take(S)` is true, and the player can drop items. A later specification states the last
condition. With button 0 the list is `drop(S, 1)`. With each other button the list is
`drop(S, count(S))`. A later specification states the repeat of a throw for a slot that fills
again (section 8). See experiment E13.

### 6.3 The drag

**ITX-JAV-014** (`source_suggested`, semantic). A drag is a sequence of quick craft packets: one
start packet, one packet for each slot, and one end packet. The server holds a drag state for each
menu: idle or open, the drag kind, and a set of slots. Only the end packet gives operations. The
list has at most 99 operations.

**ITX-JAV-015** (`source_suggested`, wire). The button of a quick craft packet holds two numbers.
The stage is `button & 3`. The drag kind is `(button >> 2) & 3`.

| Stage | Meaning | | Drag kind | Meaning |
|-------|---------|-|-----------|---------|
| 0 | start | | 0 | an equal part for each slot |
| 1 | add the clicked slot | | 1 | one item for each slot |
| 2 | end | | 2 | a full stack for each slot; creative mode only |

Stage 3 and drag kind 3 are not valid. See experiment E14.

**ITX-JAV-016** (`source_suggested`, semantic). The server does this for each quick craft packet,
by the first row whose condition is true. "Reset" sets the drag state to idle with an empty set.

| # | Condition | Result |
|---|-----------|--------|
| 1 | Stage 0 or stage 3 when the drag is open; stage 1, 2 or 3 when the drag is idle. | reset |
| 2 | `C` holds the empty stack. | reset |
| 3 | Stage 0, and the drag kind is 0 or 1, or is 2 for a player with infinite materials. | The drag is open with that drag kind and an empty set. |
| 4 | Stage 0. | reset |
| 5 | Stage 1. | The server adds `S` to the set when ITX-JAV-017 permits. |
| 6 | Stage 2. | The edge makes the request of ITX-JAV-018. Then reset. |

A click of each other click kind while the drag is open resets the drag. That click gives an
empty list. See experiment E14.

**ITX-JAV-017** (`source_suggested`, semantic). A slot `S` enters the set when each of these
conditions is true:

- `S` holds the empty stack, or `same(S, C)` is true and `count(S)` is not larger than `max(C)`;
- `place(S, C)` is true;
- the drag kind is 2, or `count(C)` is larger than the number of slots in the set;
- the menu permits a drag to `S`. A later specification states this condition.

A slot that is in the set does not enter it again. A stage 1 packet with a negative slot index
adds no slot (deviation D1). See experiment E14.

**ITX-JAV-018** (`source_suggested`, semantic). The end packet gives these operations. `z` is the
number of slots in the set.

- When `z` is 0, the list is empty.
- When `z` is 1, the result is the pick up click of ITX-JAV-008 on that slot. The button of that
  click is the drag kind. Drag kind 2 thus gives an empty list.
- When `z` is 2 or more and the drag kind is 0 or 1, each slot `X` of the set gets one
  `split(C, X, n)`.

The part `a` is `count(C)` divided by `z`, rounded down, for drag kind 0. It is 1 for drag kind 1.
For each slot, `n` is the smaller of `a` and `cap(X, C) - count(X)`. The edge reads `count(C)` one
time, before the first `split`. A slot gets a `split` only when the conditions of ITX-JAV-017 are
true again, with this change: `count(C)` is `z` or more. A `split` with `n` of 0 or less is not in
the list. The items that no slot gets stay in the cursor. See experiment E15.

**ITX-JAV-019** (`design`, semantic). The `split` operations of ITX-JAV-018 are in the order of
the slot index in the menu, smallest first. Reason: the result does not depend on the order, and
a fixed order makes the request the same in each run.

**ITX-JAV-020** (`source_suggested`, semantic). A drag of drag kind 2 with 2 or more slots makes
items. Each slot of the set gets a count of up to `cap(X, C)`. No operation of this document
states it (ITX-ATM-005). See open question Q1 and experiment E12.

## 7. Bedrock Edition: the stack request

The public protocol documentation names the packet `ItemStackRequest`. One request of that packet
has a request id and a list of actions. A slot of an action has a container, a slot index and a
stack network id. The answer has the request id, a result, and for a success the new count and
stack network id of each changed slot.

**ITX-BED-001** (`documentation`, semantic). The actions map to the operations as this table
states. The names and the numbers come from the public protocol documentation.

| Action | Number | Fields | Operation |
|--------|--------|--------|-----------|
| `Take` | 0 | amount, source, destination | `split(source, destination, amount)` |
| `Place` | 1 | amount, source, destination | `split(source, destination, amount)` |
| `Swap` | 2 | source, destination | `swap(source, destination)` |
| `Drop` | 3 | amount, source, randomly | `drop(source, amount)` |
| `Destroy` | 4 | amount, source | none in this document; the action destroys items |
| `Consume` | 5 | amount, source | none in this document; a craft uses it |
| `Create` | 6 | result slot | none in this document; the action makes items |
| each other action, `9..20` | | | none in this document |

See open question Q5 and experiment E16.

**ITX-BED-002** (`documentation`, semantic). One Bedrock request is one request of this document.
The actions give the operations in the same order. The request id of the packet is the request
id. Thus a Bedrock request applies in full or not at all (ITX-ATM-002).

**ITX-BED-003** (`design`, semantic). The stack network id is state of the edge for one
connection. The edge compares each stack network id of a request with its own state before it
makes the request. A stack network id that is wrong is a refusal of the edge, not a rejection of
ITX-REJ-001. Reason: the request must not depend on
an edition (ITX-MOD-003).

**ITX-BED-004** (`design`, semantic). After a rejection or a refusal, the edge sends an answer
with the request id and a result that is not a success. The slots do not change. A later
specification of the Bedrock edge states which result value each reason gives.

**ITX-BED-005** (`design`, semantic). A `Take` or a `Place` whose amount makes a count larger
than the capacity has the rejection `no_room` (ITX-OP-002). The server must not accept a count
that the stack does not permit. Reason: the limits of a stack are the same for each edition.

## 8. What this document does not state

A later specification states each of these items. A rule above names the place where each one
connects.

- The slots of each holder type, their slot limits and their filters (ITX-LIM-001, ITX-LIM-004,
  ITX-LIM-005).
- The menu of each menu type: its slots, their order, and when a menu is valid (ITX-JAV-001).
- **The destination order of a quick move for each menu type** (ITX-JAV-009). This document
  states only the two steps over a given list of destinations.
- The repeat of a quick move, and of a throw, on a slot that fills again (ITX-JAV-009,
  ITX-JAV-013).
- The slots from which pick up all cannot take, and the slots that a drag cannot use
  (ITX-JAV-011, ITX-JAV-017).
- The result slot of a craft, a furnace, a trade and each other machine: what a take from it does
  and how many items it gives.
- The items that take a click for themselves, for example a bundle (ITX-JAV-008).
- The insert rule of the player inventory (ITX-JAV-010).
- The operations that make or destroy items: the clone click, the drag of drag kind 2, the
  creative inventory, and the `Destroy`, `Consume`, `Create` and craft actions of Bedrock Edition.
- What a dropped stack does in the world: its position, its motion and its owner.
- The cursor stack when a menu closes, and when a player leaves.
- The state id of a menu: when it changes and what the server sends without a click.
- The wire form of each packet that this document names.

## 9. Intentional deviations

**D1.**

- **Rule**: ITX-JAV-010 and ITX-JAV-017.
- **Reference**: two clicks with the slot index -1 or -999 give an internal error in the
  reference. They are the swap with a hotbar key and the stage 1 packet of a drag.
- **Visible to the client**: no. The vanilla client sends these clicks only for a slot of the
  menu.
- **Test**: a differential test must not send these two click kinds with a negative slot index.

**D2.**

- **Rule**: ITX-LIM-003.
- **Reference**: the reference moves a stack with a count larger than its maximum stack size
  with limits that differ between the click kinds. A drag can decrease the count of a slot.
- **Visible to the client**: yes, but only for a stack that strict validation rejects.
- **Test**: a differential test must not make a stack with a count larger than its maximum stack
  size.

**D3.**

- **Rule**: ITX-JAV-019.
- **Reference**: the reference visits the slots of a drag in an order that it does not define.
- **Visible to the client**: no. The stacks after the click are the same in each order.
- **Test**: a differential test compares the stacks, not the order of the packets for the slots.

**D4.**

- **Rule**: ITX-JAV-009.
- **Reference**: step 1 of the reference does not read the place filter of a destination.
- **Visible to the client**: not known. It shows only if a menu lists a destination that holds a
  stack and refuses an equal stack. The specification of each menu must examine this.
- **Test**: a differential test must not use such a destination until that specification exists.

## 10. Open questions

For the owner:

- **Q1**: ITX-JAV-012 and ITX-JAV-020. The clone click and the creative drag make items. The
  `Destroy` and `Create` actions of ITX-BED-001 destroy and make items. Does the request get a
  `create` and a `destroy` operation, or does the edge do these changes outside the request?
- **Q2**: ITX-MOD-004. Is 128 the maximum for the number of operations? The repeat of a quick
  move on a result slot (section 8) can need a larger number. It can also use more than one
  request.
- **Q3**: section 4.3 of `docs/item-design.md` differs from this document. It has no `drop`
  operation. It has the reason `Occupied`, which this document does not use. It does not have
  the reasons `same_slot`, `bad_count`, `not_enough`, `take_denied` and `place_denied`. It gives
  one maximum for each stack, and no slot limit and no filter. Which document changes?
- **Q4**: ITX-OP-004. STK-EQ-004 states that the stack that receives must be able to stack
  (STK-EQ-003). The click of ITX-JAV-008 does not use that condition. The quick move of
  ITX-JAV-009 uses it for the stack that moves. Experiment E3 decides if STK-EQ-004 needs a
  correction.
- **Q5**: ITX-BED-001. The public documentation gives the fields of each action and not its
  conditions. Does a `Take` to a slot that holds a different stack fail, and with which result?
  Does a `Swap` join two equal stacks? Experiment E16.
- **Q6**: ITX-JAV-005. Is a rejection of a click request only a correction to the client, or
  does the server also write a warning to the log?

Not decided by an experiment yet:

- **U1**: ITX-JAV-001. If the server sends a correction for a click on a menu that is not valid,
  and for a slot index out of range. Experiment E4.
- **U2**: ITX-JAV-020. The count of the cursor after a creative drag over 2 or more slots.
  Experiment E12.
- **U3**: ITX-JAV-008, row 7. The result of a secondary click on a result slot. A later
  specification owns the answer. Experiment E8.

## 11. Experiments

Each experiment uses the game version of the table at the top of this document. An experiment of
the kind "server" uses a vanilla server with `online-mode=false` and one scripted client that is
an operator. The scripted client sends `container_click` packets and records each clientbound
packet of the menu. `tools/vanilla-oracle` has no such harness now. The first step of each
"server" experiment adds it. An experiment of the kind "bedrock" needs a Bedrock server and a
scripted Bedrock client.

| Experiment | Kind | Steps | Record |
|------------|------|-------|--------|
| E1 | server | Open a brewing stand and a chest. Hold 64 potions that have `max_stack_size` 64, and 64 stone. Click primary on a bottle slot and on a chest slot. Repeat with stone that has `max_stack_size` 16 in the patch. | The count of each slot and of the cursor after each click. |
| E2 | server | Open a furnace. Hold stone. Click primary and secondary on the fuel slot and on the result slot. Put a stack in the result slot with `item replace` and click it with an empty cursor. | Which clicks move items, and the counts. |
| E3 | server | Give two stacks of 1 item with `max_stack_size` 16, `max_damage` 10 and `damage` 3. Click primary with one on the other. Quick move one to a row that holds the other. Repeat with `damage` 0. | If the stacks join in each case, and the counts. |
| E4 | server | Send a click with a container id that is not open. Send a click as a spectator. Send a click with the slot index 500 and with -2. Send a click on a chest after the player moved 10 blocks from it. | The clientbound packets after each click. |
| E5 | server | Send a valid pick up click with the correct state id and a wrong hashed stack for the changed slot. Repeat with a wrong cursor stack. Repeat with the state id plus 1 and correct stacks. | The clientbound packets after each click, and the slots that they name. |
| E6 | server | Send each click kind with the buttons 0, 1, 2, 9 and 40 on a slot that holds 10 stone, with an empty cursor. Send pick up and quick move with the slot index -999 and -1 while the cursor holds 10 stone. | The slots, the cursor and the item entities after each click. |
| E7 | server | With 40 stone in the cursor, click primary and secondary on: an empty slot, a slot with 30 stone, a slot with 64 stone, a slot with 10 dirt. Repeat with an empty cursor on a slot with 7 stone. | The count of the slot and of the cursor after each click. |
| E8 | server | Open a crafting table with a recipe ready that gives 4 planks. Click the result slot: primary with an empty cursor, secondary with an empty cursor, primary with 62 planks in the cursor. | The cursor, the result slot and the grid after each click. |
| E9 | server | Open a chest. Put 40 stone in the slots 0 and 5 of the chest and 64 stone in the player inventory. Quick move the 64 stone. Repeat with a chest that is full of dirt. | The count of each slot after the click. |
| E10 | server | Open a brewing stand. Put 16 potions with `max_stack_size` 16 in hotbar slot 0 and one potion of a different kind in a bottle slot. Send a swap click with the button 0 on the bottle slot. Repeat with a full player inventory. Repeat with the button 40. | Each slot, and the item entities, after each click. |
| E11 | server | Open a chest. Put stone stacks of 10, 64, 20 and 5 in the chest and 3 stone in the cursor. Send pick up all with the button 0 on an empty slot. Repeat with the button 1. Repeat on a slot that holds stone. | The count of each slot and of the cursor; the order in which the slots become empty. |
| E12 | server | In creative mode, open a chest that holds 5 stone. Send a clone click on that slot. Then send a drag of drag kind 2 over 3 empty slots. Repeat the clone click in survival mode. | The cursor and each slot after each packet. |
| E13 | server | Put 10 stone in a slot. Send a throw click with the button 0, then with the button 1. Send a throw click while the cursor holds a stack. Send a throw click on a crafting result slot with the button 1. | The slot, and the count of each item entity. |
| E14 | server | With 5 stone in the cursor, send drag packets: start, 3 slots, end. Repeat with: no start packet; two start packets; a pick up click after the second slot; 7 slots; a slot two times; a slot that holds dirt; the stage 3; the drag kind 2 in survival mode. | Each slot and the cursor after each sequence. |
| E15 | server | With 10 stone in the cursor, drag with the drag kind 0 over 3 slots, of which one holds 62 stone. Repeat with the drag kind 1. Repeat each with one slot only. | The count of each slot and of the cursor. |
| E16 | bedrock | Send stack requests: a `Take` of 5 from a slot of 10 to the cursor; a `Place` on a slot that holds a different item; a `Place` that makes a count of 65; a `Swap` of two equal stacks; a request with one valid and one bad action. | The result value and the slots of each answer; the slots that the server holds after it. |
