# Worldgen preview

Generate an area of a dimension from the vanilla data pack, the packs of this
repository and zero or more extra data packs, and write PNG images of it. A
data pack author sees the terrain of a pack without a server or a client.

```sh
cargo run --release -p mcrs_minecraft_worldgen_preview -- \
  --assets assets --vanilla path/to/vanilla --pack path/to/pack \
  --seed 2 --from -2,-2 --to 1,1 --out preview --slice x=0 --slice z=-8
```

The arguments:

- `--assets`: the `assets` directory of this repository.
- `--vanilla`: the `data/minecraft` tree of the pinned client jar.
  `mcrs_minecraft_fixtures::vanilla_dir` gives it after
  `cargo run -p mcrs_minecraft_client_jar --example fetch`.
- `--pack`: a data pack directory with `pack.mcmeta` and `data/`. Repeat it for
  more packs; a later pack is above an earlier one, and every extra pack is
  above the packs of this repository.
- `--seed`: the world seed, signed or unsigned.
- `--dimension`: a dimension of the world preset `minecraft:normal`, or of a
  pack. The default is `minecraft:overworld`.
- `--from` and `--to`: two corner chunks of the area, as `<x>,<z>`.
- `--out`: the directory for the images.
- `--slice`: a vertical slice through the area at `x=<block x>` or
  `z=<block z>`. Repeat it for more slices.

The images, with north at the top:

- `height.png`: the `WORLD_SURFACE` heightmap, from black (lowest in the area)
  to white (highest).
- `top_block.png`: the map colour of the highest block that has one.
- `biome.png`: the biome below the surface, one colour for each biome name. The
  standard output lists each colour with its biome.
- `slice_x<n>.png`, `slice_z<n>.png`: the map colour of each block from the top
  of the dimension to its bottom.
