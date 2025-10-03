from __future__ import annotations

import argparse
import json
import statistics
import sys
import threading
import time
from dataclasses import asdict, dataclass
from pathlib import Path

try:
    from syncx.locks import Lock as SyncxLock
    from syncx.locks import RLock as SyncxRLock
    from syncx.locks import RWLock as SyncxRWLock
except ImportError as exc:  # pragma: no cover - benchmark environment guard
    raise SystemExit(
        "Failed to import syncx.locks. Build the extension via `maturin develop --release` "
        "before running benchmarks."
    ) from exc


@dataclass
class MutexResult:
    implementation: str
    threads: int
    throughput_ops: float
    avg_latency_s: float
    elapsed_s: float


@dataclass
class RWResult:
    implementation: str
    readers: int
    writers: int
    throughput_ops: float
    avg_latency_s: float
    elapsed_s: float


class SyncxLockTarget:
    name = "syncx.Lock"

    def __init__(self) -> None:
        self._lock = SyncxLock()

    def acquire_release(self) -> None:
        lock = self._lock
        lock.acquire()
        lock.release()


class ThreadingLockTarget:
    name = "threading.Lock"

    def __init__(self) -> None:
        self._lock = threading.Lock()

    def acquire_release(self) -> None:
        lock = self._lock
        lock.acquire()
        lock.release()


class SyncxRLockTarget:
    name = "syncx.RLock"

    def __init__(self) -> None:
        self._lock = SyncxRLock()

    def acquire_release(self, depth: int) -> None:
        guards = [self._lock.acquire() for _ in range(depth)]
        while guards:
            guards.pop().release()


class ThreadingRLockTarget:
    name = "threading.RLock"

    def __init__(self) -> None:
        self._lock = threading.RLock()

    def acquire_release(self, depth: int) -> None:
        lock = self._lock
        for _ in range(depth):
            lock.acquire()
        for _ in range(depth):
            lock.release()


class _ThreadingGuard:
    __slots__ = ("_lock",)

    def __init__(self, lock: threading.Lock) -> None:
        self._lock = lock

    def release(self) -> None:
        self._lock.release()


class SyncxRWLockTarget:
    name = "syncx.RWLock"

    def __init__(self) -> None:
        self._lock = SyncxRWLock()

    def acquire_read_guard(self):
        lock = self._lock
        lock.acquire_read()
        return _SyncxReadHandle(lock)

    def acquire_write_guard(self):
        lock = self._lock
        lock.acquire_write()
        return _SyncxWriteHandle(lock)


class ThreadingLockRWTarget:
    name = "threading.Lock"

    def __init__(self) -> None:
        self._lock = threading.Lock()

    def acquire_read_guard(self):
        lock = self._lock
        lock.acquire()
        return _ThreadingGuard(lock)

    def acquire_write_guard(self):
        lock = self._lock
        lock.acquire()
        return _ThreadingGuard(lock)


class ThreadingRLockRWTarget:
    name = "threading.RLock"

    def __init__(self) -> None:
        self._lock = threading.RLock()

    def acquire_read_guard(self):
        lock = self._lock
        lock.acquire()
        return _ThreadingGuard(lock)

    def acquire_write_guard(self):
        lock = self._lock
        lock.acquire()
        return _ThreadingGuard(lock)


class _SyncxReadHandle:
    __slots__ = ("_lock",)

    def __init__(self, lock: SyncxRWLock) -> None:
        self._lock = lock

    def release(self) -> None:
        self._lock.read_release()


class _SyncxWriteHandle:
    __slots__ = ("_lock",)

    def __init__(self, lock: SyncxRWLock) -> None:
        self._lock = lock

    def release(self) -> None:
        self._lock.write_release()


def run_mutex_benchmark(target_cls, threads: int, iterations: int) -> MutexResult:
    target = target_cls()
    total_operations = threads * iterations
    start_barrier = threading.Barrier(threads + 1)
    stop_barrier = threading.Barrier(threads + 1)
    durations = [0.0] * threads

    def worker(slot: int) -> None:
        start_barrier.wait()
        start = time.perf_counter()
        acquire_release = target.acquire_release
        for _ in range(iterations):
            acquire_release()
        end = time.perf_counter()
        durations[slot] = end - start
        stop_barrier.wait()

    thread_objs = [threading.Thread(target=worker, args=(idx,)) for idx in range(threads)]
    for thread in thread_objs:
        thread.start()

    start_barrier.wait()
    wall_start = time.perf_counter()
    stop_barrier.wait()
    wall_end = time.perf_counter()

    for thread in thread_objs:
        thread.join()

    elapsed = wall_end - wall_start
    throughput = total_operations / elapsed if elapsed else float("inf")
    avg_latency = statistics.mean(duration / iterations for duration in durations)

    return MutexResult(
        implementation=target.name,
        threads=threads,
        throughput_ops=throughput,
        avg_latency_s=avg_latency,
        elapsed_s=elapsed,
    )


def run_reentrant_benchmark(target_cls, threads: int, iterations: int, depth: int) -> MutexResult:
    target = target_cls()
    total_operations = threads * iterations
    start_barrier = threading.Barrier(threads + 1)
    stop_barrier = threading.Barrier(threads + 1)
    durations = [0.0] * threads

    def worker(slot: int) -> None:
        start_barrier.wait()
        start = time.perf_counter()
        acquire_release = target.acquire_release
        for _ in range(iterations):
            acquire_release(depth)
        end = time.perf_counter()
        durations[slot] = end - start
        stop_barrier.wait()

    thread_objs = [threading.Thread(target=worker, args=(idx,)) for idx in range(threads)]
    for thread in thread_objs:
        thread.start()

    start_barrier.wait()
    wall_start = time.perf_counter()
    stop_barrier.wait()
    wall_end = time.perf_counter()

    for thread in thread_objs:
        thread.join()

    elapsed = wall_end - wall_start
    throughput = total_operations / elapsed if elapsed else float("inf")
    avg_latency = statistics.mean(duration / iterations for duration in durations)

    return MutexResult(
        implementation=target.name,
        threads=threads,
        throughput_ops=throughput,
        avg_latency_s=avg_latency,
        elapsed_s=elapsed,
    )


def run_rw_benchmark(target_cls, readers: int, writers: int, operations: int) -> RWResult:
    target = target_cls()
    total_threads = readers + writers
    total_operations = (readers + writers) * operations
    start_barrier = threading.Barrier(total_threads + 1)
    stop_barrier = threading.Barrier(total_threads + 1)
    durations = [0.0] * total_threads

    def reader(slot: int) -> None:
        start_barrier.wait()
        start = time.perf_counter()
        acquire_guard = target.acquire_read_guard
        for _ in range(operations):
            guard = acquire_guard()
            guard.release()
        end = time.perf_counter()
        durations[slot] = end - start
        stop_barrier.wait()

    def writer(slot: int) -> None:
        start_barrier.wait()
        start = time.perf_counter()
        acquire_guard = target.acquire_write_guard
        for _ in range(operations):
            guard = acquire_guard()
            guard.release()
        end = time.perf_counter()
        durations[slot] = end - start
        stop_barrier.wait()

    threads = []
    for idx in range(readers):
        threads.append(threading.Thread(target=reader, args=(idx,)))
    for w_idx in range(writers):
        slot = readers + w_idx
        threads.append(threading.Thread(target=writer, args=(slot,)))

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
    avg_latency = statistics.mean(duration / operations for duration in durations)

    return RWResult(
        implementation=target.name,
        readers=readers,
        writers=writers,
        throughput_ops=throughput,
        avg_latency_s=avg_latency,
        elapsed_s=elapsed,
    )


def format_mutex_result(result: MutexResult) -> str:
    throughput = result.throughput_ops / 1_000_000
    latency_us = result.avg_latency_s * 1_000_000
    return (
        f"{result.threads:>7}  {result.implementation:>16}  "
        f"{throughput:>12.3f}  {latency_us:>14.2f}"
    )


def format_rw_result(result: RWResult) -> str:
    throughput = result.throughput_ops / 1_000_000
    latency_us = result.avg_latency_s * 1_000_000
    label = f"{result.readers}R/{result.writers}W"
    return (
        f"{label:>7}  {result.implementation:>16}  "
        f"{throughput:>12.3f}  {latency_us:>14.2f}"
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Lock benchmark harness")
    parser.add_argument(
        "--threads",
        type=int,
        nargs="+",
        default=[1, 2, 4],
        help="Thread counts used for mutex benchmarks",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=200_000,
        help="Acquire/release repetitions per thread for mutex benchmarks",
    )
    parser.add_argument(
        "--reentrant-depth",
        type=int,
        default=2,
        help="Acquire depth per iteration for reentrant benchmarks",
    )
    parser.add_argument(
        "--rw-readers",
        type=int,
        nargs="+",
        default=[2, 4],
        help="Reader thread counts for RWLock benchmarks",
    )
    parser.add_argument(
        "--rw-writers",
        type=int,
        default=1,
        help="Writer threads paired with each reader configuration",
    )
    parser.add_argument(
        "--rw-operations",
        type=int,
        default=75_000,
        help="Acquire/release repetitions per reader and writer",
    )
    parser.add_argument(
        "--json",
        type=Path,
        help="Optional path to write benchmark results as JSON",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    mutex_targets = [SyncxLockTarget, ThreadingLockTarget]
    rlock_targets = [SyncxRLockTarget, ThreadingRLockTarget]
    rw_targets = [SyncxRWLockTarget, ThreadingLockRWTarget, ThreadingRLockRWTarget]

    mutex_results: list[MutexResult] = []
    rlock_results: list[MutexResult] = []
    rw_results: list[RWResult] = []

    print(
        "Running mutex benchmarks with "
        f"{args.iterations} iterations/thread across threads {args.threads}"
    )
    print("  threads        implementation    throughput(Mops/s)   avg latency(us)")
    for thread_count in args.threads:
        for target in mutex_targets:
            result = run_mutex_benchmark(target, thread_count, args.iterations)
            mutex_results.append(result)
            print("  " + format_mutex_result(result))

    print(
        "\nRunning reentrant benchmarks with "
        f"depth {args.reentrant_depth}, {args.iterations} iterations/thread"
    )
    print("  threads        implementation    throughput(Mops/s)   avg latency(us)")
    for thread_count in args.threads:
        for target in rlock_targets:
            result = run_reentrant_benchmark(target, thread_count, args.iterations, args.reentrant_depth)
            rlock_results.append(result)
            print("  " + format_mutex_result(result))

    print(
        "\nRunning reader/writer benchmarks with "
        f"{args.rw_operations} operations per reader/writer and {args.rw_writers} writer(s)"
    )
    print("  threads        implementation    throughput(Mops/s)   avg latency(us)")
    for reader_count in args.rw_readers:
        for target in rw_targets:
            result = run_rw_benchmark(target, reader_count, args.rw_writers, args.rw_operations)
            rw_results.append(result)
            print("  " + format_rw_result(result))

    if args.json:
        payload = {
            "benchmark": "locks",
            "parameters": {
                "threads": args.threads,
                "iterations": args.iterations,
                "reentrant_depth": args.reentrant_depth,
                "rw_readers": args.rw_readers,
                "rw_writers": args.rw_writers,
                "rw_operations": args.rw_operations,
            },
            "mutex": [asdict(result) for result in mutex_results],
            "reentrant": [asdict(result) for result in rlock_results],
            "rw": [asdict(result) for result in rw_results],
        }
        args.json.write_text(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
