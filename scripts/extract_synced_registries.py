#!/usr/bin/env python3
"""Extract individual JSON files from synced_registries.json for Bevy AssetLoader consumption."""

import json
import os
import sys

SYNCED_REGISTRIES = os.path.join(
    os.path.dirname(__file__), "..", "crates", "mcrs_minecraft", "synced_registries.json"
)
ASSETS_DIR = os.path.join(os.path.dirname(__file__), "..", "assets", "minecraft")

SKIP_TYPES = {"worldgen/biome", "dimension_type", "enchantment"}


def main():
    with open(SYNCED_REGISTRIES) as f:
        registries = json.load(f)

    total_files = 0
    for registry_type, entries in registries.items():
        if registry_type in SKIP_TYPES:
            print(f"  SKIP  {registry_type} ({len(entries)} entries, files already exist)")
            continue

        out_dir = os.path.join(ASSETS_DIR, registry_type)
        os.makedirs(out_dir, exist_ok=True)

        for entry_name, element_data in entries.items():
            out_path = os.path.join(out_dir, f"{entry_name}.json")
            with open(out_path, "w") as f:
                json.dump(element_data, f, indent=2, ensure_ascii=False)
                f.write("\n")

        total_files += len(entries)
        print(f"  OK    {registry_type}: {len(entries)} entries -> {out_dir}")

    print(f"\nExtracted {total_files} files total.")


if __name__ == "__main__":
    main()
