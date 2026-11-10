"""Records a Metal System Trace of a program and prints the GPU time of each labelled pass.

    python3 -I tools/metal-trace/metal_trace.py record OUT.trace -- PROGRAM [ARGS...]
    python3 -I tools/metal-trace/metal_trace.py report OUT.trace --process NAME [--skip N]

`record` runs `xcrun xctrace record` with the Metal System Trace template. `report` reads the
`metal-gpu-intervals` table and prints, for each encoder label of the process and each GPU
channel (vertex, fragment, compute), the count of intervals and the median and the mean
duration. The row `span` of a label is the time from the first start to the last end of the
intervals of one encoder: for a render pass, its vertex and its fragment work together. An
interval that overlaps the work of an earlier encoder, as the vertex interval of a pass that
waits, counts from its start. The table is system wide, so `--process` keeps only the rows of one program.
`--skip N` drops the first N intervals of each row, the warm-up.
"""

import argparse
import os
import re
import statistics
import subprocess
import sys
import xml.etree.ElementTree as ElementTree

GPU_INTERVALS = '/trace-toc/run[@number="1"]/data/table[@schema="metal-gpu-intervals"]'
LABEL_SUFFIX = re.compile(r"\s+\(.*\)\s+0x[0-9a-f]+$")


def record(output, command):
    program = os.path.abspath(command[0])
    arguments = [
        "xcrun", "xctrace", "record",
        "--template", "Metal System Trace",
        "--output", output,
        "--launch", "--", program, *command[1:],
    ]
    return subprocess.run(arguments, check=False).returncode


def rows(trace):
    exported = subprocess.run(
        ["xcrun", "xctrace", "export", "--input", trace, "--xpath", GPU_INTERVALS],
        check=True, capture_output=True,
    ).stdout
    root = ElementTree.fromstring(exported)
    by_id = {}
    for element in root.iter():
        identifier = element.get("id")
        if identifier is not None:
            by_id[identifier] = element
    schema = root.find(".//schema")
    columns = [column.findtext("mnemonic") for column in schema.findall("col")]
    for row in root.iter("row"):
        values = {}
        for column, element in zip(columns, list(row)):
            reference = element.get("ref")
            if reference is not None:
                element = by_id.get(reference, element)
            values[column] = element
        yield values


def report(trace, process, skip):
    durations = {}
    spans = {}
    for values in rows(trace):
        label_element = values.get("event-label")
        process_element = values.get("process")
        if label_element is None or process_element is None:
            continue
        if not process_element.get("fmt", "").startswith(process):
            continue
        label = LABEL_SUFFIX.sub("", label_element.get("fmt", "")).strip()
        channel = values["channel-name"].get("fmt", "")
        nanoseconds = int(values["duration"].text)
        start = int(values["start"].text)
        durations.setdefault((label, channel), []).append(nanoseconds)
        encoder = (label, values["encoder-id"].text)
        first, last = spans.get(encoder, (start, start + nanoseconds))
        spans[encoder] = (min(first, start), max(last, start + nanoseconds))
    for (label, _), (first, last) in spans.items():
        durations.setdefault((label, "span"), []).append(last - first)
    if not durations:
        print(f"no GPU intervals of a process named {process}", file=sys.stderr)
        return 1
    print(f"{'label':<36} {'channel':<9} {'count':>6} {'median ms':>10} {'mean ms':>9}")
    for (label, channel), samples in sorted(durations.items()):
        samples = samples[skip:] or samples
        median = statistics.median(samples) / 1e6
        mean = statistics.fmean(samples) / 1e6
        print(f"{label:<36} {channel:<9} {len(samples):>6} {median:>10.4f} {mean:>9.4f}")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    record_parser = commands.add_parser("record")
    record_parser.add_argument("output")
    record_parser.add_argument("program", nargs=argparse.REMAINDER)
    report_parser = commands.add_parser("report")
    report_parser.add_argument("trace")
    report_parser.add_argument("--process", required=True)
    report_parser.add_argument("--skip", type=int, default=0)
    arguments = parser.parse_args()
    if arguments.command == "record":
        program = arguments.program[1:] if arguments.program[:1] == ["--"] else arguments.program
        return record(arguments.output, program)
    return report(arguments.trace, arguments.process, arguments.skip)


if __name__ == "__main__":
    sys.exit(main())
