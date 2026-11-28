# RFD 4: Versions and protocol compatibility

- Authors: andreypfau
- State: discussion
- Labels: architecture, protocol, data

This page says how mcrs relates to versions of the game: which version the core follows, which
clients can join, and how content of one version is turned off. Read it before you change the
wire layer, a registry, the edge core or the way a data pack is loaded.

## Problem

`AGENTS.md` says that mcrs targets one snapshot and has "no backward compatibility and no
datafixer". A reader takes that to mean that a server built on mcrs accepts only the client of
that snapshot. That conflicts with the mission in RFD 2. A proxy, a bot or a minigame server
that builds on the crates has players on more than one version, and it cannot depend on a
library that refuses them.

The rule was written about saves and data, not about the protocol. The difference is not
written down anywhere, so an agent that designs the wire layer or the edge core has no reason
to leave room for a second protocol version.

## Direction

This section is the maintainer's. It was given in conversation and is not yet a determination.

The core follows the latest version of the game. Its registries, rules and data come from the
pinned client jar, as they do today.

Content that a version added is a module. A version that added a footstool block is one
module, and a version that added a block of ice is another. A server that does not add the
module for the newest version runs without that content.

A client of an older version can join a server whose core is newer. The protocol layer
translates for that client. When the server runs without the modules that the old client does
not know, the old client sees the complete world.

Taken to its end, the same model runs a server of a very old version, such as Beta 1.7.3: its
world generation, and only its blocks and items, without hunger, elytra or enchantments. The
implementation of the newer content exists and is turned off. A data pack serves the server and
not the client, so the server cuts its modules from the data pack of the latest core, never
from an old one.

## What is known

Each statement says how it is known.

- The client knows the numeric ids of blocks, block states, items and entity types, and the
  ids change from one version to the next. Read in public protocol documentation; not measured
  in mcrs.
- Registries that are driven by data are sent to the client in the configuration phase.
  Blocks and items are not among them. Read in public protocol documentation.
- A chunk on the wire carries numeric block state ids in its palettes. Observed in
  `mcrs_minecraft_protocol_game`, which encodes the sections of a chunk.
- Game systems encode a packet to bytes, its packet id included, before the edge receives it.
  The edge frames and compresses bytes that it does not read. Observed in
  `send_own_entity_state` of `mcrs_minecraft_engine` and in `mcrs_minecraft_network`.
- Ids are not the only difference between versions. Packet ids and field layouts change, and
  so do the shapes of registry entries in the configuration phase, the payload of tags,
  recipes, the types and shapes of data components on an item stack, the indices of entity
  metadata, the hashes of item stacks that a newer client sends in a container click, and the
  format of a resource pack. A signed chat message cannot be signed again by a translator. Read
  in public protocol documentation and in the documentation of public translation proxies.
- The repository already holds a second protocol, in the crates `mcrs_minecraft_bedrock_*`.
  Observed.
- The game turns experimental content on and off with feature flags, and a data pack can
  require a flag. Content that was released before flags existed carries no flag. Read in
  public documentation.
- The client predicts its own movement with the rules of its own version. Assumption, from
  how the client behaves; not tested against two versions.
- Beta 1.7.3 uses a protocol from before the current framing, a world height of 128 blocks and
  a different save format. Read in public documentation.

## What a version changes

The direction above fits one kind of change well and the other kind poorly. This section is
the agent's analysis.

An addition is new content: a block, an item, a mob, a structure. It turns off cleanly. Its
code is not called and its data is not loaded.

A change alters content that already exists: how combat works, how water flows, how terrain is
shaped, how hard a block is, how fast a player moves, the collision shape of a block. It does
not turn off. The old behaviour is a second set of rules or numbers for the same mechanic.

Most versions contain both kinds. A server of an old version needs the old form of every
change, and not only the absence of every addition. RFD 2 asks for one way to do one thing, and
two forms of one mechanic look like two ways.

## Determination

The maintainer made each choice in this section, from options that the agent prepared. The
options that were not chosen are listed after each choice.

**What the design requires now.** Two rules hold from the first code that touches them. Neither
builds a second protocol version.

1. The protocol version is a property of a connection, not of a world.
2. A detail of the wire that differs between versions lives only in the protocol crates. A
   game system does not depend on it.

Deferred until the first real module or the first second protocol version: the table that maps
ids per version, encoding once per version among the recipients of a broadcast, separable
content data, and the parameters of a rule as data. Each has no user today, and each is cheap
to add on top of the two rules.

Not chosen: four required items now, which charged code today for a user who does not exist.

A change that alters a rule and not only its numbers stays out of scope until a real server
needs it. When one does, the second form is a module that replaces the first.

**Movement of an old client.** The server moves every player by the rules of the core. When
those rules differ from the rules of the client's version, the collision shapes of blocks
included, the server corrects the player. The movement rules are a public point, so a user who
needs the rules of the client's version replaces them with a module that chooses by
connection.

Not chosen: rules chosen per connection by mcrs itself, which costs a parameter on the physics
of the player for every user. A server that refuses the versions whose movement differs, which
limits the versions that can share a server.

**Content that an old client does not know.** By default, a server accepts a version only when
every loaded module exists in it. The table that maps ids for a version is a public point. A
proxy or a minigame server that wants to show an old client a substitute supplies the table and
answers for the result.

Not chosen: a substitute for every client by default. A substitute with another collision shape
breaks the client's prediction and confuses anti-cheat.

**Saves.** The core keeps one save format, and a `DataVersion` mismatch stays a loud error. A
converter for a save of an older version, when one is needed, is a separate offline tool. It
converts a world once, before the server opens it, and the load path of the server never
contains it.

Not chosen: a datafixer in the core.

**Data of older versions.** Deferred. Modules are cut from the data pack of the latest core.
Each entry needs the version that added it. For versions with data reports, the update tool
derives it from the reports of older client jars. For older versions, it comes from the public
history of the game. Module manifests are generated, never written by hand.

**Beta and other versions before the current framing.** They are the limit of the model and
out of the scope of this RFD. Their clients need their own wire crates and their saves need
their own reader.

## Open question

**Where a packet is translated.** The maintainer has not decided.

- A: Inside one process, game systems emit typed packets, and each is encoded once for each
  protocol version among its recipients. There is no decoding and no second encoding. Today
  game systems send bytes through `OutboundPlayerPacket`, which 67 files of the workspace use,
  so this is a migration of all of them.
- B: When the edge runs in another process, the edge or a translator crate beside it receives
  frames, decodes the ones that differ, and encodes them again for the client's version. The
  latest clients pay nothing. A proxy can use the same crate.

A and B can hold together: typed packets inside a process, translation of frames across a
process boundary. A measurement of the cost of the second encoding against the cost of keeping
typed packets decides. The translation table is layer 3 data, so the translator is a crate of
layer 3 or above, and the edge stays without game data.

## Ruled out

A copy of the core per version. Every fix would be made several times, and the crates would
stop being one library.

A wire layer that follows the oldest supported client. Every new feature would wait for a
translation, and the core would carry the oldest model forever.

## Consequences

`AGENTS.md` changes its section "Version" to say that there is no compatibility of saves and
no datafixer, and that compatibility of the protocol is a design goal.

The update tool keeps one pinned jar until the first module of older content is needed.

## What would reopen this

- A measurement that shows the translation of one connection costs more than the network
  thread of that connection can give.
- A version whose changes cannot be expressed as modules and data, so that its support would
  need a second copy of a subsystem.
- A user of the crates who needs one fixed old version and none of the newer content, which
  would favour a separate wire crate for that version over translation.

## Objections

These are the strongest objections that the text does not already answer.

- The Bedrock protocol is already a second protocol in the repository, and it has no consumer
  yet. It is the first real test of a translation table, and this RFD does not say whether
  Bedrock goes through the same translator. It stands.
- A signed chat message cannot be translated, so an old client and a new client on one server
  see chat that is not signed for one of them. It stands until the chat design answers it.
