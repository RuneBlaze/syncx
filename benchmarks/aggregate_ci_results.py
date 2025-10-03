"""Aggregate benchmark JSON outputs into a single payload for CI artifacts."""

from __future__ import annotations

import argparse
import json
import os
import platform
import sys
from datetime import datetime, timezone
from pathlib import Path


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Aggregate benchmark result files")
    parser.add_argument("results", nargs="+", type=Path, help="Input JSON benchmark files")
    parser.add_argument("--output", type=Path, required=True, help="Output JSON file path")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    combined = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "commit": os.environ.get("GITHUB_SHA"),
        "ref": os.environ.get("GITHUB_REF"),
        "job": os.environ.get("GITHUB_JOB"),
        "runner_os": os.environ.get("RUNNER_OS"),
        "platform": platform.platform(),
        "python": sys.version,
        "benchmarks": [],
    }

    for path in args.results:
        if not path.exists():
            continue
        with path.open("r", encoding="utf-8") as fh:
            payload = json.load(fh)
        combined["benchmarks"].append(payload)

    args.output.write_text(json.dumps(combined, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
