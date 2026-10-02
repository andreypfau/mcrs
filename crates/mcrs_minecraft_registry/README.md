# mcrs_minecraft_registry

Registries of named entries, and the numbers that stand for those entries.

The typed core is made of five pieces. `Id<R>` is the number of one entry of the
registry `R`. `Registry<R>` is an immutable table that maps entry names to ids and
back, and knows the names of its tags. `Entries<R, T>` is a column of values, one
per entry, indexed by `Id<R>`. `RegistrySet` is the set of registries a parse can
resolve names against. `LoadReport` collects the entries a consumer asked for and
did not find. The older static tables and indices of this crate stay until the
code that reads them moves to the typed core.

## The scope

A value that names registry entries, such as an `Id<R>` or a set of entries, is
parsed inside `RegistrySet::scope`. The scope is synchronous and belongs to the
current thread: it ends when the closure returns, and a nested scope restores the
outer one. There is no default set, so a parse on another thread or in another task
needs a scope of its own. A parse with no scope, or with a scope that does not hold
the registry it needs, is an error that names the type being parsed and the
registry.

## Well-known entries

A consumer that depends on specific entries resolves them once, at load, in the
plugin that needs them, and keeps the ids in one resource. The system that does it
asks a `LoadReport` for every entry, so one pass names every entry that is
missing, sorted by registry and then by entry, each entry once. The resource is
built only when every entry resolved. When the report is not empty, startup stops
and prints it. Nothing resolves a name by string at the point of use: after load a
consumer holds ids, and an id is read without a lookup.

## Differences from the game

An unknown name or id is always an error. The game's registries of blocks and
entity types answer a default entry for an unknown name; these registries do not,
on purpose, because a default hides a name that the data got wrong.
