"""Run lightweight benchmark suite for CI builds."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


LOCKS_PARAMS = [
    "--threads",
    "1",
    "2",
    "4",
    "--iterations",
    "1000",
    "--reentrant-depth",
    "2",
    "--rw-readers",
    "4",
    "--rw-writers",
    "2",
    "--rw-operations",
    "10",
]

DICTS_PARAMS = [
    "--threads",
    "1",
    "2",
    "4",
    "8",
    "--operations",
    "5000",
    "--key-space",
    "1000",
    "--seed",
    "42",
]

QUEUES_PARAMS = [
    "--pairs",
    "1",
    "2",
    "4",
    "8",
    "--messages",
    "5000",
    "--maxsize",
    "0",
]


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run CI benchmark suite")
    parser.add_argument(
        "--dist",
        type=Path,
        default=Path("dist"),
        help="Directory containing built wheels",
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=Path("benchmark-results"),
        help="Directory where intermediate JSON files should be written",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("benchmark-results.json"),
        help="Aggregated JSON output path",
    )
    return parser.parse_args(argv)


def pick_wheel(dist_dir: Path) -> Path:
    abi3 = sorted(dist_dir.glob("*abi3*.whl"))
    if abi3:
        return abi3[0]
    wheels = sorted(dist_dir.glob("*.whl"))
    if not wheels:
        raise FileNotFoundError(f"No wheels found in {dist_dir}")
    return wheels[0]


def run_command(cmd: list[str]) -> None:
    subprocess.check_call(cmd)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    args.results_dir.mkdir(parents=True, exist_ok=True)

    wheel_path = pick_wheel(args.dist)
    run_command([sys.executable, "-m", "pip", "install", "--upgrade", "pip"])
    run_command([sys.executable, "-m", "pip", "install", str(wheel_path)])

    locks_json = args.results_dir / "locks.json"
    dicts_json = args.results_dir / "dicts.json"
    queues_json = args.results_dir / "queues.json"

    run_command([sys.executable, "benchmarks/compare_locks.py", *LOCKS_PARAMS, "--json", str(locks_json)])
    run_command([sys.executable, "benchmarks/compare_dicts.py", *DICTS_PARAMS, "--json", str(dicts_json)])
    run_command([sys.executable, "benchmarks/compare_queues.py", *QUEUES_PARAMS, "--json", str(queues_json)])

    run_command(
        [
            sys.executable,
            "benchmarks/aggregate_ci_results.py",
            "--output",
            str(args.output),
            str(locks_json),
            str(dicts_json),
            str(queues_json),
        ]
    )

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
