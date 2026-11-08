"""Records the entity physics experiments from a vanilla dedicated server.

The script starts the server, freezes the ticks, builds each experiment in its own area of a flat
world, and steps one tick at a time. After each tick it reads Pos, Motion, OnGround and
fall_distance of each entity with /data get, and writes one JSON fixture per experiment. It reads
only what the server prints on its console.
"""

import argparse
import json
import os
import queue
import re
import subprocess
import sys
import threading
import time

BASE_Y = 100
SPACING = 48


def stand(tag, pos, nbt="", **extra):
    return dict(tag=tag, kind="armor_stand", pos=pos, nbt=nbt, **extra)


def experiments():
    """The experiments; block boxes and positions are relative to the origin of each area."""
    items = '{id:"minecraft:stone",count:1}'
    floor = lambda block="stone", size=12: [((-2, 0, -4), (size, 0, 4), block)]
    found = []

    found.append(dict(name="e1_fall", rule="E1", ticks=40, blocks=floor(), entities=[
        stand("a", (0.5, 11.0, 0.5), "Motion:[0.0d,0.0d,0.0d]"),
        stand("b", (4.5, 11.0, 0.5), "Motion:[0.0d,0.0d,0.0d],NoGravity:1b"),
        stand("c", (8.5, 11.0, 0.5), "Motion:[0.0d,0.0d,0.0d]", effects=["slow_falling"]),
    ]))
    for index, block in enumerate(["stone", "ice", "blue_ice", "soul_sand"]):
        top = 0.875 if block == "soul_sand" else 1.0
        found.append(dict(name=f"e2_slide_{block}", rule="E2", ticks=40, blocks=floor(block, 16), entities=[
            stand("a", (0.5, top, 0.5), "Motion:[0.5d,0.0d,0.3d]"),
        ]))
    for floor_block in ["stone", "slime_block"]:
        found.append(dict(name=f"e3_drop_{floor_block}", rule="E3", ticks=60, blocks=floor(floor_block, 24), entities=[
            dict(tag="item", kind="item", pos=(0.5, 6.0, 0.5),
                 nbt=f"Item:{items},PickupDelay:32767,Age:-32768,Motion:[0.1d,0.05d,0.08d]"),
            dict(tag="sand", kind="falling_block", pos=(6.5, 6.0, 0.5),
                 nbt='BlockState:{Name:"minecraft:sand"},Time:1,DropItem:0b,Motion:[0.1d,0.05d,0.08d]'),
            dict(tag="tnt", kind="tnt", pos=(12.5, 6.0, 0.5), nbt="fuse:400,Motion:[0.1d,0.05d,0.08d]"),
            dict(tag="orb", kind="experience_orb", pos=(18.5, 6.0, 0.5), nbt="Value:1,Motion:[0.1d,0.05d,0.08d]"),
        ]))
    obstacles = ["stone", "oak_fence", "stone_slab[type=bottom]", "stone_stairs[facing=west]", "white_carpet"]
    for index, obstacle in enumerate(obstacles):
        name = obstacle.split("[")[0]
        blocks = floor() + [((3, 1, -4), (3, 1, 4), obstacle)]
        found.append(dict(name=f"e4_wall_{name}", rule="E4", ticks=8, blocks=blocks, entities=[
            stand("a", (0.5, 1.0, 0.5), "Motion:[0.6d,0.0d,0.0d]"),
            stand("b", (1.7, 1.0, -2.5), "Motion:[0.6d,0.0d,0.0d]"),
            stand("c", (2.4, 1.0, 2.5), "Motion:[0.6d,0.0d,0.0d]"),
        ]))
    steps = ["stone_slab[type=bottom]", "stone", "snow[layers=2]", "snow[layers=3]", "snow[layers=5]"]
    for obstacle in steps:
        name = obstacle.replace("[", "_").replace("]", "").replace("=", "_").replace(",", "_")
        blocks = floor() + [((2, 1, -4), (4, 1, 4), obstacle)]
        found.append(dict(name=f"e5_step_{name}", rule="E5", ticks=8, blocks=blocks, entities=[
            stand("a", (1.3, 1.0, 0.5), "Motion:[0.3d,0.0d,0.0d],OnGround:1b"),
            stand("b", (1.3, 1.0, -2.5), "Motion:[0.3d,0.0d,0.0d],OnGround:1b", attributes={"step_height": 1.0}),
            stand("c", (1.3, 1.0, 2.5), "Motion:[0.3d,0.0d,0.0d],OnGround:1b", attributes={"step_height": 0.0}),
            stand("d", (1.3, 1.0, -1.0), "Motion:[0.3d,0.0d,0.0d],OnGround:1b", attributes={"step_height": 0.6}),
        ]))
    for block in ["slime_block", "red_bed[part=foot,facing=east]", "honey_block", "stone"]:
        name = block.split("[")[0]
        blocks = floor() + [((0, 1, 0), (0, 1, 0), block), ((3, 1, 0), (3, 1, 0), block)]
        if name == "red_bed":
            head = "red_bed[part=head,facing=east]"
            blocks += [((1, 1, 0), (1, 1, 0), head), ((4, 1, 0), (4, 1, 0), head)]
        found.append(dict(name=f"e6_bounce_{name}", rule="E6", ticks=60, blocks=blocks, entities=[
            stand("a", (0.5, 7.0, 0.5), "Motion:[0.0d,0.0d,0.0d]"),
            stand("b", (3.5, 7.0, 0.5), "Motion:[0.0d,0.0d,0.0d]", attributes={"bounciness": 0.5}),
        ]))
    for block in ["cobweb", "sweet_berry_bush[age=1]", "powder_snow"]:
        name = block.split("[")[0]
        blocks = floor() + [((0, 0, 0), (0, 0, 0), "grass_block"), ((4, 0, 0), (4, 0, 0), "grass_block")]
        blocks += [((0, 1, 0), (0, 1, 0), block), ((4, 1, 0), (4, 1, 0), block)]
        found.append(dict(name=f"e7_inside_{name}", rule="E7", ticks=20, blocks=blocks, entities=[
            stand("a", (0.5, 3.0, 0.5), "Motion:[0.0d,0.0d,0.0d]"),
            dict(tag="item", kind="item", pos=(4.5, 3.0, 0.5),
                 nbt=f"Item:{items},PickupDelay:32767,Age:-32768,Motion:[0.0d,0.0d,0.0d]"),
        ]))
    found.append(dict(name="e8_ladder", rule="E8", ticks=30, blocks=floor() + [
        ((2, 1, 1), (2, 7, 1), "stone"), ((2, 1, 0), (2, 7, 0), "ladder[facing=north]"),
    ], entities=[stand("a", (2.5, 6.0, 0.3), "Motion:[0.0d,0.0d,0.05d]")]))
    found.append(dict(name="e8_honey", rule="E8", ticks=30, blocks=floor() + [
        ((2, 1, 0), (2, 7, 0), "honey_block"),
    ], entities=[stand("a", (3.7, 6.0, 0.5), "Motion:[-0.05d,0.0d,0.0d]")]))
    found.append(dict(name="e16_shulker", rule="E16", ticks=10, blocks=floor(), entities=[
        dict(tag="shulker", kind="shulker", pos=(4.0, 1.0, 0.0), nbt="NoAI:1b", collider=True),
        stand("a", (0.5, 1.0, 0.5), "Motion:[0.4d,0.0d,0.0d],OnGround:1b"),
    ]))
    for block in ["scaffolding[distance=0,bottom=false]", "powder_snow"]:
        name = block.split("[")[0]
        blocks = floor() + [((0, 1, 0), (0, 1, 0), block), ((4, 1, 0), (4, 1, 0), block)]
        found.append(dict(name=f"e17_fall_into_{name}", rule="E17", ticks=20, blocks=blocks, entities=[
            stand("a", (0.5, 3.0, 0.5), "Motion:[0.0d,0.0d,0.0d],fall_distance:0.0d"),
            stand("b", (4.5, 3.0, 0.5), "Motion:[0.0d,0.0d,0.0d],fall_distance:3.0d"),
        ]))
    for block in ["stone", "ice"]:
        found.append(dict(name=f"e18_modifiers_{block}", rule="E18", ticks=40, blocks=floor(block, 16), entities=[
            stand("a", (0.5, 1.0, -3.5), "Motion:[0.5d,0.0d,0.3d]", attributes={"friction_modifier": 0.5}),
            stand("b", (0.5, 1.0, -1.5), "Motion:[0.5d,0.0d,0.3d]", attributes={"friction_modifier": 2.0}),
            stand("c", (0.5, 3.0, 0.5), "Motion:[0.5d,0.0d,0.3d]", attributes={"air_drag_modifier": 0.5}),
            stand("d", (0.5, 3.0, 2.0), "Motion:[0.5d,0.0d,0.3d]", attributes={"air_drag_modifier": 2.0}),
            stand("e", (0.5, 1.0, 3.5), "Motion:[0.5d,0.0d,0.3d]", attributes={"movement_efficiency": 1.0}),
        ]))
    found.append(dict(name="e19_small_speed", rule="E19", ticks=3, blocks=floor(), entities=[
        stand("a", (0.5, 1.0, 0.5), "Motion:[0.1d,0.0d,0.0001d],OnGround:1b"),
        stand("b", (0.5, 1.0, 2.5), "Motion:[0.002d,0.0d,0.002d],OnGround:1b"),
    ]))
    found.append(dict(name="e28_rest", rule="E28", ticks=12, blocks=floor(), entities=[
        stand("a", (0.5, 1.0, 0.5), "Motion:[0.0d,0.0d,0.0d]"),
        dict(tag="item", kind="item", pos=(3.5, 1.0, 0.5),
             nbt=f"Item:{items},PickupDelay:32767,Age:-32768,Motion:[0.0d,0.0d,0.0d]"),
        dict(tag="tnt", kind="tnt", pos=(6.5, 1.0, 0.5), nbt="fuse:400,Motion:[0.0d,0.0d,0.0d]"),
        dict(tag="orb", kind="experience_orb", pos=(9.5, 1.0, 0.5), nbt="Value:1,Motion:[0.0d,0.0d,0.0d]"),
    ]))
    found.append(dict(name="e32_rest_merged", rule="E32", ticks=40, merge_each_tick=True, blocks=floor(), entities=[
        dict(tag="item", kind="item", pos=(3.5, 1.0, 0.5),
             nbt=f"Item:{items},PickupDelay:32767,Age:-32768,Motion:[0.0d,0.0d,0.0d]"),
    ]))
    for distance in ["0.3", "0.05", "0.005"]:
        found.append(dict(name=f"e34_push_{distance.replace('.', '_')}", rule="E34", ticks=5, blocks=floor(), entities=[
            dict(tag="a", kind="zombie", pos=(0.5, 1.0, 0.5), nbt="NoAI:1b,OnGround:1b,Motion:[0.0d,0.0d,0.0d]"),
            dict(tag="b", kind="zombie", pos=(0.5 + float(distance), 1.0, 0.5), nbt="NoAI:1b,OnGround:1b,Motion:[0.0d,0.0d,0.0d]"),
        ]))
    box = floor() + [((-1, 1, -1), (1, 2, -1), "glass"), ((-1, 1, 1), (1, 2, 1), "glass"),
                     ((-1, 1, 0), (-1, 2, 0), "glass"), ((1, 1, 0), (1, 2, 0), "glass")]
    for order in ["ab", "ba"]:
        entities = {
            "a": dict(tag="a", kind="zombie", pos=(0.4, 1.0, 0.45), nbt="NoAI:1b,OnGround:1b,Motion:[0.0d,0.0d,0.0d]"),
            "b": dict(tag="b", kind="zombie", pos=(0.6, 1.0, 0.55), nbt="NoAI:1b,OnGround:1b,Motion:[0.0d,0.0d,0.0d]"),
        }
        found.append(dict(name=f"e35_crowd_{order}", rule="E35", ticks=5, blocks=box,
                          entities=[entities[tag] for tag in order]))
    return found


def border_experiments():
    return [dict(name="e15_border", rule="E15", ticks=12, origin=(0, BASE_Y, 0), border=(0.0, 0.0, 10.0), blocks=[
        ((-6, 0, -6), (6, 0, 6), "stone"),
    ], entities=[
        stand("a", (4.0, 1.0, 0.5), "Motion:[0.4d,0.0d,0.0d],OnGround:1b"),
        stand("b", (2.0, 1.0, -2.5), "Motion:[0.4d,0.0d,0.0d],OnGround:1b"),
    ])]


def tile_experiments():
    """The experiments across the edges of the tiles of the entity pass: each origin is a corner of
    four tiles of 32 blocks, so x = 0 and z = 0 of the area are tile edges."""
    items = '{id:"minecraft:stone",count:1}'
    item = lambda tag, pos, motion: dict(
        tag=tag, kind="item", pos=pos,
        nbt=f"Item:{items},PickupDelay:32767,Age:-32768,Motion:[{motion}]")
    zombie = lambda tag, pos: dict(
        tag=tag, kind="zombie", pos=pos, nbt="NoAI:1b,OnGround:1b,Motion:[0.0d,0.0d,0.0d]")
    return [
        dict(name="t1_slide_across_tiles", rule="E3", ticks=40, origin=(2048, BASE_Y, 0),
             blocks=[((-8, 0, -8), (8, 0, 8), "ice")], entities=[
            item("x", (-1.5, 1.0, 4.5), "0.3d,0.0d,0.0d"),
            item("z", (4.5, 1.0, 1.5), "0.0d,0.0d,-0.3d"),
            item("corner", (-1.0, 3.0, -1.0), "0.15d,0.1d,0.15d"),
            dict(tag="orb", kind="experience_orb", pos=(1.5, 1.0, -4.5), nbt="Value:1,Motion:[-0.3d,0.0d,0.0d]"),
        ]),
        dict(name="t2_push_across_tiles", rule="E42", ticks=20, origin=(2112, BASE_Y, 0),
             blocks=[((-8, 0, -8), (8, 0, 8), "stone")], entities=[
            zombie("a", (-0.15, 1.0, 3.5)), zombie("b", (0.15, 1.0, 3.5)),
            zombie("c", (-0.0025, 1.0, 6.5)), zombie("d", (0.0025, 1.0, 6.5)),
            zombie("e", (-4.5, 1.0, -0.15)), zombie("f", (-4.5, 1.0, 0.15)),
        ]),
        dict(name="t3_rest_broken_support", rule="E33", ticks=20, origin=(2176, BASE_Y, 0),
             blocks=[((-8, 0, -8), (8, 0, 8), "stone"), ((0, 0, 2), (0, 0, 2), "air")],
             changes=[(10, (-2, 0, 0), (-2, 0, 0), "air"), (10, (-1, 0, 2), (-1, 0, 2), "air")], entities=[
            item("own", (-1.5, 1.0, 0.5), "0.0d,0.0d,0.0d"),
            item("next", (0.05, 1.0, 2.5), "0.0d,0.0d,0.0d"),
        ]),
    ]


class Console:
    def __init__(self, work):
        self.process = subprocess.Popen(
            ["java", "-Xmx2G", "-jar", "server.jar", "nogui"],
            cwd=work, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, bufsize=1,
        )
        self.lines = queue.Queue()
        threading.Thread(target=self._pump, daemon=True).start()
        self.marker = 0

    def _pump(self):
        for line in self.process.stdout:
            self.lines.put(line.rstrip("\n"))

    def wait_for(self, pattern, timeout=300):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                line = self.lines.get(timeout=1)
            except queue.Empty:
                if self.process.poll() is not None:
                    raise RuntimeError("the server exited")
                continue
            if re.search(pattern, line):
                return line
        raise RuntimeError(f"no line matched {pattern}")

    def run(self, commands):
        """Sends the commands and returns the console lines up to an echo marker after them."""
        self.marker += 1
        token = f"physics-marker-{self.marker}"
        for command in commands:
            self.process.stdin.write(command + "\n")
        self.process.stdin.write(f"say {token}\n")
        self.process.stdin.flush()
        seen = []
        while True:
            line = self.lines.get(timeout=120)
            if token in line:
                return seen
            seen.append(line)

    def stop(self):
        self.process.stdin.write("stop\n")
        self.process.stdin.flush()
        self.process.wait(timeout=120)


NUMBER = r"(-?[0-9.]+(?:E-?[0-9]+)?)d"


def parse(line):
    """The motion state in the reply of /data get entity."""
    if "has the following entity data" not in line:
        return None
    if not re.search(r"Pos:\s*\[", line):
        raise RuntimeError(f"no position in {line}")
    vector = lambda key: [float(value) for value in re.findall(NUMBER, re.search(key + r":\s*\[([^\]]*)\]", line).group(1))]
    fall = re.search(r"fall_distance:\s*" + NUMBER, line)
    ground = re.search(r"OnGround:\s*([01])b", line)
    return dict(
        pos=vector("Pos"),
        motion=vector("Motion"),
        on_ground=ground is not None and ground.group(1) == "1",
        fall_distance=float(fall.group(1)) if fall else 0.0,
    )


def game_time(console):
    for line in console.run(["time query gametime"]):
        match = re.search(r"game time is (\d+)", line)
        if match:
            return int(match.group(1))
    raise RuntimeError("no game time")


def selector(experiment, entity):
    return f"@e[tag={experiment['name']}_{entity['tag']},limit=1]"


def fill(origin, low, high, block):
    ox, oy, oz = origin
    return f"fill {ox + low[0]} {oy + low[1]} {oz + low[2]} {ox + high[0]} {oy + high[1]} {oz + high[2]} minecraft:{block}"


def box(low, high, block):
    return dict(min=list(low), max=list(high), block=f"minecraft:{block}")


def build(console, experiment, origin):
    commands = [fill(origin, low, high, block) for low, high, block in experiment["blocks"]]
    for entity in experiment["entities"]:
        x, y, z = (origin[i] + entity["pos"][i] for i in range(3))
        tag = f"{experiment['name']}_{entity['tag']}"
        attributes = entity.get("attributes", {})
        nbt = [f'Tags:["{tag}"]']
        if entity["nbt"]:
            nbt.append(entity["nbt"])
        if attributes:
            listed = ",".join(f'{{id:"minecraft:{key}",base:{value}d}}' for key, value in attributes.items())
            nbt.append(f"attributes:[{listed}]")
        commands.append(f"summon minecraft:{entity['kind']} {x!r} {y!r} {z!r} {{{','.join(nbt)}}}")
        for effect in entity.get("effects", []):
            commands.append(f"effect give @e[tag={tag}] minecraft:{effect} 1000 0 true")
    console.run(commands)


def record(console, group):
    origins = [tuple(experiment["origin"]) for experiment in group]
    console.run(["tick freeze"])
    for experiment, origin in zip(group, origins):
        if "border" in experiment:
            cx, cz, size = experiment["border"]
            console.run([f"worldborder center {origin[0] + cx} {origin[2] + cz}", f"worldborder set {size}"])
        build(console, experiment, origin)
    states = {experiment["name"]: {entity["tag"]: [] for entity in experiment["entities"]} for experiment in group}
    longest = max(experiment["ticks"] for experiment in group)

    def read():
        commands, keys = [], []
        for experiment in group:
            for entity in experiment["entities"]:
                commands.append(f"data get entity {selector(experiment, entity)}")
                keys.append((experiment["name"], entity["tag"]))
        replies = [line for line in console.run(commands) if "entity data" in line or "No entity was found" in line]
        if len(replies) != len(keys):
            raise RuntimeError(f"{len(replies)} replies for {len(keys)} entities")
        for (name, tag), line in zip(keys, replies):
            states[name][tag].append(parse(line))

    read()
    for step in range(longest):
        merges = [
            f"data merge entity {selector(experiment, entity)} {{}}"
            for experiment in group if experiment.get("merge_each_tick")
            for entity in experiment["entities"]
        ]
        if merges:
            console.run(merges)
        changes = [
            fill(origin, low, high, block)
            for experiment, origin in zip(group, origins)
            for tick, low, high, block in experiment.get("changes", [])
            if tick == step + 1
        ]
        if changes:
            console.run(changes)
        before = game_time(console)
        console.run(["tick step 1"])
        while game_time(console) != before + 1:
            time.sleep(0.02)
        read()
    results = []
    for experiment, origin in zip(group, origins):
        entities = []
        for entity in experiment["entities"]:
            entry = dict(entity)
            entry["ticks"] = states[experiment["name"]][entity["tag"]][: experiment["ticks"] + 1]
            entities.append(entry)
        blocks = [box(low, high, block) for low, high, block in experiment["blocks"]]
        result = dict(name=experiment["name"], rule=experiment["rule"], origin=list(origin), blocks=blocks, entities=entities)
        if "changes" in experiment:
            result["changes"] = [
                dict(tick=tick, **box(low, high, block)) for tick, low, high, block in experiment["changes"]
            ]
        if "border" in experiment:
            cx, cz, size = experiment["border"]
            result["border"] = dict(center=[origin[0] + cx, origin[2] + cz], size=size)
        results.append(result)
    return results


def reset(console, group):
    commands = ["kill @e[type=!minecraft:player]"]
    for index in range(len(group)):
        x = SPACING * index
        commands.append(f"fill {x - 8} {BASE_Y - 2} -8 {x + 30} {BASE_Y + 14} 8 minecraft:air")
    commands.append("worldborder set 59999968")
    commands.append("worldborder center 0 0")
    console.run(commands)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--version", required=True)
    arguments = parser.parse_args()
    for leftover in ("world",):
        path = os.path.join(arguments.work, leftover)
        if os.path.isdir(path):
            subprocess.run(["rm", "-rf", path], check=True)
    console = Console(arguments.work)
    try:
        console.wait_for(r"Done \(")
        main_group = experiments()
        for index, experiment in enumerate(main_group):
            experiment["origin"] = (SPACING * index, BASE_Y, 0)
        span = SPACING * len(main_group) + 32
        console.run([
            "gamerule spawn_mobs false", "gamerule doMobSpawning false",
            "gamerule random_tick_speed 0", "gamerule randomTickSpeed 0",
            "gamerule advance_time false", "gamerule doDaylightCycle false", "time set midnight",
        ] + [f"forceload add {x} -16 {min(x + 1007, span)} 16" for x in range(-16, span, 1008)]
          + [f"forceload add {x - 16} -16 {x + 16} 16" for x in (e["origin"][0] for e in tile_experiments())])
        time.sleep(5)
        runs = [record(console, main_group)]
        reset(console, main_group)
        runs.append(record(console, [experiment for experiment in main_group if experiment["rule"] in ("E1", "E4", "E5")]))
        reset(console, main_group)
        runs.append(record(console, border_experiments()))
        reset(console, main_group)
        runs.append(record(console, tile_experiments()))
    finally:
        console.stop()
    first, repeat, border, tiles = runs
    repeated = {result["name"]: result for result in repeat}
    for result in first + border + tiles:
        if result["name"] in repeated:
            result["repeat_equal"] = repeated[result["name"]]["entities"] == result["entities"]
        result["version"] = arguments.version
        with open(os.path.join(arguments.out, result["name"] + ".json"), "w") as file:
            json.dump(result, file, indent=1)
            file.write("\n")
    print(f"wrote {len(first) + len(border) + len(tiles)} fixtures to {arguments.out}")


if __name__ == "__main__":
    sys.exit(main())
