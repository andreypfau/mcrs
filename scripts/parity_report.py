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
MISSING = "\u2014"


def load(path):
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.int16)


def compare(base, head):
    delta = np.abs(base - head)
    return int((delta.max(axis=2) > 0).sum()), int(delta.max()), delta


def load_timings(path):
    """Rows of a capture's timing file keyed by (kind, slot), or None without one."""
    if not path.is_file():
        return None
    rows = {}
    for line in path.read_text().splitlines():
        if line:
            kind, slot, median, p95 = line.split("\t")
            rows[(kind, slot)] = (float(median), float(p95))
    return rows


def gpu_total(timings):
    if not timings:
        return None
    medians = [median for (kind, _), (median, _) in timings.items() if kind == "gpu"]
    return sum(medians) if medians else None


def cpu_engine(timings):
    row = (timings or {}).get(("cpu", "engine"))
    return row[0] if row else None


def ms(value):
    return MISSING if value is None else f"{value:.3f}"


def timing_table(base, head):
    slots = list(dict.fromkeys([*(base or {}), *(head or {})]))
    if not slots:
        return "No timings recorded.\n"
    lines = [
        "| Kind | Slot | Base median ms | Base p95 ms | Head median ms | Head p95 ms |",
        "|------|------|----------------|-------------|----------------|-------------|",
    ]
    for key in slots:
        b = (base or {}).get(key, (None, None))
        h = (head or {}).get(key, (None, None))
        lines.append(f"| {key[0]} | {key[1]} | {ms(b[0])} | {ms(b[1])} | {ms(h[0])} | {ms(h[1])} |")
    return "\n".join(lines) + "\n"


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
        base_t = load_timings(out / scene / "base.tsv")
        head_t = load_timings(out / scene / "head.tsv")
        timing = (
            f"{ms(gpu_total(base_t))} \u2192 {ms(gpu_total(head_t))} | "
            f"{ms(cpu_engine(base_t))} \u2192 {ms(cpu_engine(head_t))}"
        )
        if result == "error":
            print(f"{scene}: error {detail}")
            rows.append(f"| {scene} | error: {detail} | | | {timing} |")
            errored = True
        elif result == "differs":
            print(f"{scene}: {detail[0]} pixels differ, largest delta {detail[1]}")
            rows.append(f"| {scene} | differs | {detail[0]} | {detail[1]} | {timing} |")
            differed = True
        else:
            print(f"{scene}: identical")
            rows.append(f"| {scene} | identical | 0 | 0 | {timing} |")
        images = [f"![base]({scene}/base.png)", f"![head]({scene}/head.png)"]
        if result == "differs":
            images.append(f"![difference x{AMPLIFY}]({scene}/diff.png)")
        sections.append(
            f"## {scene}\n\n" + "\n".join(images) + "\n\n" + timing_table(base_t, head_t)
        )
    text = "\n".join(
        [
            "# Parity",
            "",
            f"- Base: {base_label}",
            f"- Head: {head_label}",
            "",
            "| Scene | Result | Differing pixels | Largest channel delta "
            "| GPU total ms base \u2192 head | CPU engine ms base \u2192 head |",
            "|-------|--------|------------------|-----------------------"
            "|---------------------------|-----------------------------|",
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
        (out / "same" / "base.tsv").write_text(
            "gpu\tcull\t0.2500\t0.3000\ngpu\tworld\t4.0000\t5.0000\n"
            "cpu\tmain\t1.0000\t1.5000\ncpu\tengine\t3.0000\t3.5000\n"
        )
        (out / "same" / "head.tsv").write_text(
            "gpu\tcull\t0.5000\t0.6000\ngpu\tforward\t1.0000\t1.2000\n"
            "cpu\tengine\t2.0000\t2.5000\n"
        )
        (out / "scenes.txt").write_text("same\nchanged\n")
        assert report(out, "base", "head") == 3
        text = (out / "report.md").read_text()
        assert "| same | identical | 0 | 0 | 4.250 \u2192 1.500 | 3.000 \u2192 2.000 |" in text, text
        assert f"| changed | differs | 1 | 1 | {MISSING} \u2192 {MISSING} | {MISSING} \u2192 {MISSING} |" in text
        assert "| gpu | world | 4.000 | 5.000 | \u2014 | \u2014 |" in text
        assert "| gpu | forward | \u2014 | \u2014 | 1.000 | 1.200 |" in text
        assert (out / "changed" / "diff.png").is_file()
        assert not (out / "same" / "diff.png").exists()
        (out / "scenes.txt").write_text("same\nmissing\n")
        assert report(out, "base", "head") == 1
    print("self-test passed")
    return 0


def main(args):
    if args == ["--self-test"]:
        return self_test()
    if len(args) != 3 or args[0].startswith("--"):
        print(__doc__.strip(), file=sys.stderr)
        return 1
    return report(*args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
