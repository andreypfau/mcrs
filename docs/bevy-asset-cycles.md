# Cyclic asset references

Cross-references between assets of a cyclic registry are kept as ids and
joined at freeze, never as `#[dependency]` handles:

- `template_pool`, `structure_set` and `structure` files are requested flat
  (every file in the folder, recursively) rather than pulled in by reference.
- Only acyclic edges are asset dependencies: a pool's templates
  (`structure/*.nbt`) and processor lists.
- Every id is resolved once at freeze, and an id that names nothing is a load
  error naming the asset.

The cost is a recursive directory listing where the registry is nested, and
that a reference to a missing file surfaces at freeze rather than at load.
