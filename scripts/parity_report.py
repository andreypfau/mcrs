#!/usr/bin/env python3
"""Compares the base and head capture of every scene in a parity run.

usage: parity_report.py <out-dir> <base-label> <head-label>
       parity_report.py --self-test

Exits 0 when every scene is identical, 3 when any differs, 1 when any errored.
"""

import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

AMPLIFY = 32


def load(path):
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.int16)


def compare(base, head):
    delta = np.abs(base - head)
    return int((delta.max(axis=2) > 0).sum()), int(delta.max()), delta


def scene_result(out, scene):
    folder = out / scene
    for side in ("base", "head"):
        if not (folder / f"{side}.png").is_file():
            return "error", f"{side}.png is missing, see {scene}/{side}.log"
    base, head = load(folder / "base.png"), load(folder / "head.png")
    if base.shape != head.shape:
        return "error", (
            f"sizes differ ({base.shape[1]}x{base.shape[0]} and "
            f"{head.shape[1]}x{head.shape[0]}), see {scene}/base.log and {scene}/head.log"
        )
    differing, largest, delta = compare(base, head)
    if differing == 0:
        return "identical", (0, 0)
    Image.fromarray(np.clip(delta * AMPLIFY, 0, 255).astype(np.uint8)).save(folder / "diff.png")
    return "differs", (differing, largest)


def report(out, base_label, head_label):
    out = Path(out)
    scenes_file = out / "scenes.txt"
    scenes = scenes_file.read_text().split() if scenes_file.is_file() else []
    if not scenes:
        print(f"error: no scenes in {scenes_file}")
        return 1
    rows, sections = [], []
    errored = differed = False
    for scene in scenes:
        result, detail = scene_result(out, scene)
        if result == "error":
            print(f"{scene}: error {detail}")
            rows.append(f"| {scene} | error: {detail} | | |")
            errored = True
        elif result == "differs":
            print(f"{scene}: {detail[0]} pixels differ, largest delta {detail[1]}")
            rows.append(f"| {scene} | differs | {detail[0]} | {detail[1]} |")
            differed = True
        else:
            print(f"{scene}: identical")
            rows.append(f"| {scene} | identical | 0 | 0 |")
        images = [f"![base]({scene}/base.png)", f"![head]({scene}/head.png)"]
        if result == "differs":
            images.append(f"![difference x{AMPLIFY}]({scene}/diff.png)")
        sections.append(f"## {scene}\n\n" + "\n".join(images) + "\n")
    text = "\n".join(
        [
            "# Classic parity",
            "",
            f"- Base: {base_label}",
            f"- Head: {head_label}",
            "",
            "| Scene | Result | Differing pixels | Largest channel delta |",
            "|-------|--------|------------------|-----------------------|",
            *rows,
            "",
            *sections,
        ]
    )
    (out / "report.md").write_text(text)
    return 1 if errored else 3 if differed else 0


def self_test():
    with tempfile.TemporaryDirectory() as temp:
        out = Path(temp)
        pixels = np.zeros((4, 6, 3), dtype=np.uint8)
        pixels[1, 2] = (10, 20, 30)
        changed = pixels.copy()
        changed[1, 2, 1] += 1
        for scene, head in (("same", pixels), ("changed", changed)):
            (out / scene).mkdir()
            Image.fromarray(pixels).save(out / scene / "base.png")
            Image.fromarray(head).save(out / scene / "head.png")
        base = load(out / "same" / "base.png")
        assert compare(base, load(out / "same" / "head.png"))[:2] == (0, 0)
        assert compare(base, load(out / "changed" / "head.png"))[:2] == (1, 1)
        (out / "scenes.txt").write_text("same\nchanged\n")
        assert report(out, "base", "head") == 3
        text = (out / "report.md").read_text()
        assert "| same | identical |" in text and "| changed | differs | 1 | 1 |" in text
        assert (out / "changed" / "diff.png").is_file()
        assert not (out / "same" / "diff.png").exists()
        (out / "scenes.txt").write_text("same\nmissing\n")
        assert report(out, "base", "head") == 1
    print("self-test passed")
    return 0


def main(args):
    if args == ["--self-test"]:
        return self_test()
    if len(args) != 3:
        print(__doc__.strip(), file=sys.stderr)
        return 1
    return report(*args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
