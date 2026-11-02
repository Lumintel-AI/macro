#!/usr/bin/env python3
"""Select workspace packages that own files changed since the fork's baseline."""

import json
import os
from pathlib import Path
import subprocess
import sys


def main():
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    metadata = json.loads(Path(sys.argv[1]).read_text())
    members = set(metadata["workspace_members"])
    packages = {
        Path(package["manifest_path"]).parent.relative_to(root): package["name"]
        for package in metadata["packages"]
        if package["id"] in members
    }
    baseline = subprocess.check_output(
        ["git", "merge-base", "origin/lumintel", "HEAD"], text=True
    ).strip()
    changed = subprocess.check_output(
        ["git", "diff", "--name-only", "--no-renames", "-z", baseline, "HEAD"]
    ).decode().split("\0")
    selected = set()
    for filename in filter(None, changed):
        path = Path(filename)
        # Root manifest/toolchain changes can affect every workspace package.
        if filename in {"Cargo.toml", "rust-toolchain.toml"} or filename.startswith(".cargo/"):
            selected.update(packages.values())
        else:
            for parent in path.parents:
                if parent in packages:
                    selected.add(packages[parent])
                    break

    names = sorted(selected)
    print(f"Baseline {baseline}; changed packages: {', '.join(names) or 'none'}")
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"matrix={json.dumps({'package': names}, separators=(',', ':'))}\n")
        output.write(f"has_packages={str(bool(names)).lower()}\n")
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a") as output:
            output.write(f"Compared with `origin/lumintel` at merge-base `{baseline}`.\n\n")
            output.writelines(f"- `{name}`\n" for name in names)


if __name__ == "__main__":
    main()
