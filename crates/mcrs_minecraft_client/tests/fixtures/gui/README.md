# GUI parity fixtures

`gui_parity.rs` crops the hotbar and the inventory screen out of a client screenshot and
compares them pixel for pixel (one step per channel of slack, the `container.crafting`
title rectangle masked) with `hotbar_2.png` and `inventory_2.png`, captured from the
vanilla client with the same player data. The fixtures are not checked in until they have
been captured; until then the ignored test fails on the missing file.

## Shared conditions

Both clients must see the same picture behind the GUI, so both look straight up at a frozen
noon sky: nothing but the sky colour shows through the translucent hotbar and the screen dim.

- window 854x480 **physical** pixels (on a HiDPI display vanilla renders at 2x: use a
  non-scaled display, or capture at 1708x960 and set `MCRS_RESOLUTION`/`MCRS_GUI_SCALE`
  and the constants in `gui_parity.rs` to match), GUI scale 2, cursor at GUI (138, 126)
  (over main-inventory slot 9, the first slot of the top row)
- world time 6000, player pitch -90, in the same world folder, spawned at the same position
- the same `playerdata/<uuid>.dat`, holding the curated stack set the server-side
  inventory tests write (a damaged pickaxe, a stack of 64, an enchanted item, a bundle,
  armour in the armour cells, an offhand shield, and a carried stack)

## Capturing the vanilla fixtures

1. Run the vanilla 26.3-snapshot-10 client (`~/Library/Application Support/minecraft/versions/26.3-snapshot-10`)
   against the world folder above; `options.txt` lines: `guiScale:2`, `fullscreen:false`,
   `renderDistance:2`, `glintSpeed:0.5`, `glintStrength:0.75`, `hideGui:false`.
2. `/time set 6000`, `/gamerule doDaylightCycle false`, look straight up (`/tp @s ~ ~ ~ 0 -90`).
3. Open the inventory (E), move the cursor to GUI (138, 126) (physical (276, 252)), press F2.
4. Crop `screenshots/<latest>.png` at the same rectangles the test uses at scale 2:
   hotbar `(244, 436) 364x44`, inventory `(250, 74) 352x332`, and save them here as
   `hotbar_2.png` and `inventory_2.png`.
5. Record the vanilla build below.

Vanilla build: (not yet captured)

## Running the comparison

```
MCRS_GUI_PARITY=1 MCRS_GUI_PARITY_WORLD=<world folder> \
  cargo test -p mcrs_minecraft_client --test gui_parity -- --ignored
```

`MCRS_GUI_PARITY_WAIT` (seconds, default 20) is how long the client gets to join and
stream before the capture is triggered. On failure the test writes `<name>_2.diff.png`
next to the fixture with every differing pixel in magenta.
