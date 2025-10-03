from __future__ import annotations

import argparse
import json
import queue as std_queue
import statistics
import sys
import threading
import time
from dataclasses import asdict, dataclass
from pathlib import Path

try:
    from syncx.collections import Queue as SyncxQueue
except ImportError as exc:  # pragma: no cover - benchmark environment guard
    raise SystemExit(
        "Failed to import syncx.collections.Queue. Build the extension via "
        "`maturin develop --release` before running benchmarks."
    ) from exc


@dataclass
class BenchmarkResult:
    implementation: str
    pairs: int
    throughput_ops: float
    avg_latency_s: float
    elapsed_s: float


class TargetProtocol:
    name: str

    def __init__(self, maxsize: int) -> None:  # pragma: no cover - benchmark plumbing
        raise NotImplementedError

    def put(self, value: object) -> None:
        raise NotImplementedError

    def get(self) -> object:
        raise NotImplementedError


class SyncxQueueTarget(TargetProtocol):
    name = "syncx.Queue"

    def __init__(self, maxsize: int) -> None:
        self.queue = SyncxQueue(maxsize=maxsize)

    def put(self, value: object) -> None:
        self.queue.put(value)

    def get(self) -> object:
        return self.queue.get()


class PythonQueueTarget(TargetProtocol):
    name = "queue.Queue"

    def __init__(self, maxsize: int) -> None:
        self.queue: std_queue.Queue[object] = std_queue.Queue(maxsize=maxsize)

    def put(self, value: object) -> None:
        self.queue.put(value)

    def get(self) -> object:
        return self.queue.get()


def run_benchmark(
    target_cls: type[TargetProtocol],
    pairs: int,
    messages_per_producer: int,
    maxsize: int,
) -> BenchmarkResult:
    target = target_cls(maxsize=maxsize)
    total_messages = pairs * messages_per_producer
    total_operations = total_messages * 2  # one put and one get per message

    start_barrier = threading.Barrier(2 * pairs + 1)
    stop_barrier = threading.Barrier(2 * pairs + 1)
    producer_durations = [0.0] * pairs
    consumer_durations = [0.0] * pairs

    def producer(slot: int) -> None:
        start_barrier.wait()
        start = time.perf_counter()
        for _ in range(messages_per_producer):
            target.put(slot)
        end = time.perf_counter()
        producer_durations[slot] = end - start
        stop_barrier.wait()

    def consumer(slot: int) -> None:
        start_barrier.wait()
        start = time.perf_counter()
        for _ in range(messages_per_producer):
            target.get()
        end = time.perf_counter()
        consumer_durations[slot] = end - start
        stop_barrier.wait()

    threads = []
    for idx in range(pairs):
        threads.append(threading.Thread(target=producer, args=(idx,)))
        threads.append(threading.Thread(target=consumer, args=(idx,)))

    for thread in threads:
        thread.start()

    start_barrier.wait()
    wall_start = time.perf_counter()
    stop_barrier.wait()
    wall_end = time.perf_counter()

    for thread in threads:
        thread.join()

    elapsed = wall_end - wall_start
    throughput = total_operations / elapsed if elapsed else float("inf")
    avg_latency = statistics.mean(
        duration / messages_per_producer
        for duration in producer_durations + consumer_durations
    )

    return BenchmarkResult(
        implementation=target.name,
        pairs=pairs,
        throughput_ops=throughput,
        avg_latency_s=avg_latency,
        elapsed_s=elapsed,
    )


def format_result(result: BenchmarkResult) -> str:
    throughput = result.throughput_ops / 1_000_000  # convert to Mops/s
    latency_us = result.avg_latency_s * 1_000_000
    return (
        f"{result.pairs:>7}  {result.implementation:>16}  "
        f"{throughput:>12.3f}  {latency_us:>14.2f}"
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Queue benchmark harness")
    parser.add_argument(
        "--pairs",
        type=int,
        nargs="+",
        default=[1, 2, 4, 8],
        help="Number of producer/consumer pairs to benchmark",
    )
    parser.add_argument(
        "--messages",
        type=int,
        default=50_000,
        help="Number of messages each producer enqueues",
    )
    parser.add_argument(
        "--maxsize",
        type=int,
        default=0,
        help="Queue max size (0 means unbounded)",
    )
    parser.add_argument(
        "--json",
        type=Path,
        help="Optional path to write benchmark results as JSON",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    targets = [SyncxQueueTarget, PythonQueueTarget]
    print(
        "Running benchmarks with "
        f"{args.messages} messages/producer, maxsize {args.maxsize}, pairs {args.pairs}"
    )
    print("  pairs        implementation    throughput(Mops/s)   avg latency(us)")

    all_results: list[BenchmarkResult] = []
    for pair_count in args.pairs:
        for target in targets:
            result = run_benchmark(target, pair_count, args.messages, args.maxsize)
            all_results.append(result)
            print("  " + format_result(result))

    if args.json:
        payload = {
            "benchmark": "queues",
            "parameters": {
                "pairs": args.pairs,
                "messages": args.messages,
                "maxsize": args.maxsize,
            },
            "results": [asdict(result) for result in all_results],
        }
        args.json.write_text(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
