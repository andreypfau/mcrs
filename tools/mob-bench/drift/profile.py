"""Reads the folders that mob-drift wrote and prints, for each checkpoint, the pig density in rings
of 4 blocks of horizontal distance from the player, and the share of pigs that walked more than
1 block in the window. Usage: python3 -I profile.py <run folder>..."""

import csv
import math
import sys

RING = 4.0
MAX = 80.0
MOVED = 1.0
CELL = 0.25


def meta(folder):
    values = {}
    with open(f"{folder}/meta.txt") as text:
        for line in text:
            if line.startswith("checkpoint="):
                continue
            key, _, value = line.strip().partition("=")
            values[key] = value
    return values


def ring_areas(side, px, pz):
    half = side / 2
    rings = int(MAX / RING)
    areas = [0.0] * rings
    steps = int(side / CELL)
    for i in range(steps):
        x = -half + (i + 0.5) * CELL
        for j in range(steps):
            z = -half + (j + 0.5) * CELL
            ring = int(math.hypot(x - px, z - pz) / RING)
            if ring < rings:
                areas[ring] += CELL * CELL
    return areas


def profile(folder, checkpoint, areas, px, pz):
    rings = len(areas)
    counts = [0] * rings
    moved = [0] * rings
    with open(f"{folder}/pigs-{checkpoint}.csv") as text:
        for row in csv.DictReader(text):
            x, z = float(row["x"]), float(row["z"])
            ring = int(math.hypot(x - px, z - pz) / RING)
            if ring >= rings:
                continue
            counts[ring] += 1
            step = math.hypot(float(row["x2"]) - x, float(row["z2"]) - z)
            if step > MOVED:
                moved[ring] += 1
    return counts, moved


def main(folders):
    for folder in folders:
        values = meta(folder)
        side = int(values["side"])
        px, _, pz = (float(part) for part in values["player"].split())
        areas = ring_areas(side, px, pz)
        checkpoints = []
        with open(f"{folder}/meta.txt") as text:
            for line in text:
                if line.startswith("checkpoint="):
                    checkpoints.append(line.split()[0].split("=")[1])
        print(f"# {folder}: {values['server']}, {values['pigs']} pigs, window {values['window']} ticks")
        header = "ring   area " + " ".join(f"{'d@' + c:>7} {'mv@' + c:>7}" for c in checkpoints)
        print(header)
        results = [profile(folder, c, areas, px, pz) for c in checkpoints]
        for ring, area in enumerate(areas):
            if area < 1:
                continue
            cells = []
            for counts, moved in results:
                density = counts[ring] / area
                share = moved[ring] / counts[ring] if counts[ring] else float("nan")
                cells.append(f"{density:7.3f} {share:7.2f}")
            print(f"{int(ring * RING):3d}-{int(ring * RING + RING):<3d}{area:6.0f} " + " ".join(cells))
        print()


if __name__ == "__main__":
    main(sys.argv[1:])
