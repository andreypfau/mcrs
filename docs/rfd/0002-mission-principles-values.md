# RFD 2: Mission, principles and values

- Authors: andreypfau
- State: published
- Labels: process

This page says what mcrs is for, what it does not do under any pressure, and what it prefers
when two good things pull apart. An agent or a contributor who meets a choice that no rule
covers decides from this page. The form follows
[Oxide's RFD 2](https://rfd.shared.oxide.computer/rfd/0002): a mission, then principles, which
are constraints, then values, which are preferences and can conflict.

## Mission

To be the crates that everyone builds Minecraft software from.

The mission is met when other people's projects depend on these crates. Today each proxy, bot
and map renderer writes its own protocol, its own NBT and its own idea of a chunk. Success is
the day they stop, because taking a crate from mcrs is easier than writing one. Large servers
that run mcrs, and Java mods that call into its crates for speed, follow from that. They are
welcome results and they are not the goal.

So the first user of mcrs is a developer who builds on its crates. The server and the client
in this repository are two such programs, and they prove that the crates are enough. When a
server owner or a player wants something that would make the crates worse to build on, the
developer's need wins.

Three choices follow from the mission.

Redesigned. A server that keeps the design of the Java server, one thread and objects that
share mutable state, keeps its limit in any language. mcrs
keeps what a player sees and builds the engine again on an ECS, so that the tick is spread
over every core by what each piece of work touches.

A set of libraries. The server and the client are two programs assembled from the same crates.
A proxy, a bot, a lobby, a minigame server, an anti-cheat, a live map, a skin viewer or a save
editor takes the crates it needs and nothing else. Vanilla gameplay is one more module, and a
server can run without it.

In parallel. The design target is ten thousand players in one dimension with an unmodified
vanilla client, and a hundred thousand or more simulated entities at twenty ticks per second.
It is a target and not a measurement. The first measurement against it names the machine.

## Principles

A principle holds when it is expensive, slow or inconvenient. A change that breaks one is
wrong, whatever else it achieves.

Clean room. A specification fixes the result and leaves the method free. It states what a
player, a client or a save file observes, and nothing about how the game arrives there. The
implementer builds from the specification and never reads the game's code. That code is a
reference: a specification author can read it, as `docs/clean-room.md` says, and nobody takes a
design from it. The design is worked out with the maintainer.

The rule gives legal cover, but it exists for the freedom. Nearly every idea of how Minecraft is
built comes from the single-threaded game and the object-oriented habits of its code, and a
person who has read that code designs inside its limits without noticing. Once the result is
fixed and the method is open, the method can be anything. World generation in mcrs follows a
different algorithm from the game's and produces the same world wherever the seed decides the
game's result. Where the game's result depends on the order in which columns were generated, as
trees at the border of two columns do, that is a defect of the game
([MC-55596](https://bugs.mojang.com/browse/MC/issues/MC-55596) is one), and mcrs decides by
coordinates alone. One seed gives one world. Light spreads over all cores at once. The game
moves entities one after another, each seeing what the earlier ones did (PHY-ORD-010 of
`docs/spec/entity-physics.md` states that order). In mcrs an entity
moves, collides and pushes its neighbours from a snapshot taken at the start of the tick and
writes only its own state. Where the result of the game depends on the order inside a tick, as
with explosions and with entities that collide, mcrs runs a sequential group in a fixed order of
keys, so that TNT cannons work as in the game (`docs/physics-architecture.md`). Either way, the
result is the same on any number of threads.

Sometimes a mechanic turns out to be one case of a wider idea, the way water and lava can be
described as one fluid with two sets of numbers. When that generalisation offers itself, the
specification states the wider idea. It is not sought for its own sake.

The game decides behaviour and data, and never the shape of the code. Layouts, constants,
ordering and what the player observes come from the game. Its object graph does not.

Every part can be taken alone. A crate has one job and a small dependency tree. Logic lives in
crates that do not need Bevy. A library crate does not choose a path, read a configuration file
or read an environment variable, and it does not panic on input that a caller can get wrong.

Nothing in the design stands in the way of scale. The result of the simulation does not depend
on the number of threads. State has one owner. The tick never waits.

A claim is something that somebody can check. "Works" needs a test, or a run that a person can
watch. A goal is labelled as a goal. A number has the command that produced it, and a
comparison with another server waits until the compared feature is complete.

A person answers for what carries their name. Agents write most of the code and the text. The
person who merges a change answers for its result and for every consequence of every line in it.
Evidence is how the person knows the result; the lines remain the person's. A contributor's pull
request stays the contributor's.

## Values

Values are in tension with one another, and each of them turns harmful when it is followed to
the end.

Less code. Less code has fewer defects and costs the reader less. A shared helper comes before
a second copy, and a deletion is a result.

Simplicity that can grow. The design is the simplest one that works today, shaped so that the
scalable version is a change to it and not a replacement. A design that is modular and easy to
change is already correct.

Correctness in the types. A function that can fail says so in its signature. A visible error
is better than a silent wrong value.

One model at the boundary. A developer who uses a crate holds one model of it in mind, and that
model is the one the rest of the project already uses. Entity rows were faster than components
at one million entities (issue #97), but they were a second way to store an entity. They grew
workarounds that rebuilt the ECS the project stands on, and anyone who touched entities would
have had to know both. They were removed.

Simple where people build, complex where they swap. Some parts of mcrs are taken whole or
replaced whole. A developer uses the vanilla world generator or writes another one, and nobody
reaches inside it, so its graph optimiser can be as intricate as the speed requires. Other
parts are material that people build with. Entity physics is touched by every minigame and
every custom mechanic that somebody invents, and there each extra concept is a cost that every
one of those developers pays. Such a part stays simple even when a more complicated version
would be faster. The light engine, the protocol and NBT codecs and the Anvil reader are of the
first kind. Items and inventories, block simulation, commands and player events are of the
second.

One way to do one thing. A second mechanism beside a first one needs a second consumer and a
measurement. A migration ends with the old path deleted.

Vanilla by default. The game is evidence of what a player expects, so mcrs does what the game
does until someone shows that the behaviour is a defect. The evidence decides, and Mojang's
resolution of a report is one part of it. The other parts are the comments and votes on the
report, whether Bedrock Edition has the same behaviour, and what the players who build on the
behaviour agree on. When Mojang contradicts itself between editions or versions, its
resolution does not decide. A defect whose result depends on order, timing, loading or the
number of threads is not carried over, and neither is an exploit that a later version fixed. A
defect that stays open for years with no fix, such as the duplication of TNT, shows a flaw in
the design, and mcrs fixes it. A behaviour that is part of the game but that the project would
rather drop, such as quasi-connectivity, can be switched by a game rule, and the default of
that rule is what the game does. The maintainer decides each case from the evidence. The
protocol and the player's own physics match exactly, because a client depends on them. World
generation matches exactly, except where the game depends on the order of generation. The rest
matches in meaning. A deliberate difference is proposed with options, chosen by the
maintainer, and written down.

Code that reads without help. Code has no comments, because a comment lets a poor design stay.
An API is obvious to a person who has not read the rest of the code. Text is plain and short.

Show it. A running client, a screenshot, a video or a measurement comes before the claim.

The maintainer's attention. It is the scarcest resource of the project. An agent finds out by
itself what can be found out, and brings a decision as options with a recommendation.

Momentum. The part that gives most of the result ships first. A feature can be minimal. Its
design cannot be wrong.

When these conflict, correctness comes first, then clarity, then speed of delivery.

## Options

The maintainer considered these other missions and rejected them.

- A server first, and crates later. A playable server comes sooner. The crates would take the
  shape of the one server, and a proxy or a bot could not take them on their own.
- The design of the Java server, written again in Rust. The design is known, and parity is
  easier to reach. The limit of one thread would stay.
- A mod or a plugin for the Java server. It runs with the real game and its plugins. The tick
  would stay on one thread, and no part could be taken alone.

## What would reopen this

- The crates are released, and no project outside this repository depends on them. Then the
  mission does not hold.
- A measurement shows that the tick on an ECS does not scale on the target machine. Then the
  choice "In parallel" does not hold.

## Objections

These are the strongest objections that the text does not already answer.

- The rules that keep TNT cannons are read from the reference and not yet confirmed by an
  experiment. Within a sequential group the order key is the UUID, and in the game it is the
  order of the ticks, so a cannon that depends on the order of its charges can still differ. It
  stands until a differential test of a real cannon runs.
- "Simple where people build" asks entity physics to stay simple even when a faster form
  exists, and entity physics is the hot path of the scale target. The answer is that scale
  comes from the engine around a simple kernel, not from inside the kernel.
