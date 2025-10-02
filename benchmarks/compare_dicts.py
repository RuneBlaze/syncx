from __future__ import annotations

import argparse
import random
import statistics
import sys
import threading
import time
from dataclasses import dataclass

try:
    from syncx.dict import ConcurrentDict
except ImportError as exc:  # pragma: no cover - benchmark environment guard
    raise SystemExit(
        "Failed to import syncx.dict.ConcurrentDict. Build the extension via "
        "`maturin develop --release` before running benchmarks."
    ) from exc


@dataclass
class BenchmarkResult:
    structure: str
    threads: int
    throughput_ops: float
    avg_latency_s: float
    elapsed_s: float


class TargetProtocol:
    name: str

    def populate(self, count: int) -> None:
        raise NotImplementedError

    def read(self, key: int) -> None:
        raise NotImplementedError

    def write(self, key: int, value: int) -> None:
        raise NotImplementedError


class ConcurrentDictTarget(TargetProtocol):
    name = "ConcurrentDict"

    def __init__(self) -> None:
        self.store: ConcurrentDict[int, int] = ConcurrentDict()

    def populate(self, count: int) -> None:
        for key in range(count):
            self.store[key] = key

    def read(self, key: int) -> None:
        self.store.get(key)

    def write(self, key: int, value: int) -> None:
        self.store[key] = value


class PythonDictTarget(TargetProtocol):
    name = "Python dict"

    def __init__(self) -> None:
        self.store: dict[int, int] = {}

    def populate(self, count: int) -> None:
        for key in range(count):
            self.store[key] = key

    def read(self, key: int) -> None:
        self.store.get(key)

    def write(self, key: int, value: int) -> None:
        self.store[key] = value


def run_benchmark(
    target_factory: type[TargetProtocol],
    threads: int,
    operations_per_thread: int,
    read_ratio: float,
    key_space: int,
    rng_seed: int,
) -> BenchmarkResult:
    target = target_factory()
    target.populate(key_space)

    total_operations = threads * operations_per_thread
    start_barrier = threading.Barrier(threads + 1)
    stop_barrier = threading.Barrier(threads + 1)
    durations = [0.0] * threads

    def worker(slot: int) -> None:
        thread_rng = random.Random(rng_seed + slot)
        start_barrier.wait()
        start = time.perf_counter()
        for _ in range(operations_per_thread):
            key = thread_rng.randrange(key_space)
            if thread_rng.random() < read_ratio:
                target.read(key)
            else:
                value = thread_rng.randrange(key_space)
                target.write(key, value)
        end = time.perf_counter()
        durations[slot] = end - start
        stop_barrier.wait()

    threads_list = [threading.Thread(target=worker, args=(idx,)) for idx in range(threads)]
    for th in threads_list:
        th.start()

    start_barrier.wait()
    wall_start = time.perf_counter()
    stop_barrier.wait()
    wall_end = time.perf_counter()

    for th in threads_list:
        th.join()

    elapsed = wall_end - wall_start
    throughput = total_operations / elapsed if elapsed else float("inf")
    avg_latency = statistics.mean(duration / operations_per_thread for duration in durations)

    return BenchmarkResult(
        structure=target.name,
        threads=threads,
        throughput_ops=throughput,
        avg_latency_s=avg_latency,
        elapsed_s=elapsed,
    )


def format_result(result: BenchmarkResult) -> str:
    throughput = result.throughput_ops / 1_000_000  # convert to Mops/s
    latency_us = result.avg_latency_s * 1_000_000
    return (
        f"{result.threads:>7}  {result.structure:>18}  "
        f"{throughput:>12.3f}  {latency_us:>14.2f}"
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Concurrent dict benchmark harness")
    parser.add_argument(
        "--threads",
        type=int,
        nargs="+",
        default=[1, 2, 4, 8, 16],
        help="Thread counts to benchmark",
    )
    parser.add_argument(
        "--operations",
        type=int,
        default=50_000,
        help="Number of operations each thread performs per scenario",
    )
    parser.add_argument(
        "--key-space",
        type=int,
        default=100_000,
        help="Number of distinct keys seeded into each map",
    )
    parser.add_argument(
        "--read-heavy-ratio",
        type=float,
        default=0.9,
        help="Read probability for the read-heavy scenario",
    )
    parser.add_argument(
        "--write-heavy-ratio",
        type=float,
        default=0.1,
        help="Read probability for the write-heavy scenario",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=1812,
        help="Base RNG seed used to generate workloads",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    targets = [ConcurrentDictTarget, PythonDictTarget]
    scenarios = [
        ("Read-heavy", args.read_heavy_ratio),
        ("Write-heavy", args.write_heavy_ratio),
    ]

    print(
        "Running benchmarks with "
        f"{args.operations} ops/thread, key-space {args.key_space}, threads {args.threads}"
    )

    for scenario_name, read_ratio in scenarios:
        write_ratio = 1.0 - read_ratio
        print(
            f"\n{scenario_name} "
            f"({read_ratio * 100:.0f}% reads / {write_ratio * 100:.0f}% writes)"
        )
        print("  threads        structure    throughput(Mops/s)   avg latency(us)")
        for thread_count in args.threads:
            for target in targets:
                result = run_benchmark(
                    target,
                    thread_count,
                    args.operations,
                    read_ratio,
                    args.key_space,
                    args.seed,
                )
                print("  " + format_result(result))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
