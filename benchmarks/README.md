# Benchmarks

This directory houses micro-benchmark drivers that compare `syncx` primitives
against their standard-library counterparts. Each script prints simple tables
with throughput (in millions of operations per second) and average latency per
operation.

Before running any benchmark, build and install the extension in the active
virtual environment:

```bash
maturin develop --release
```

## Concurrent Dictionary

`compare_dicts.py` exercises the `syncx.collections.ConcurrentDict` against Python's
builtin `dict` under mixed read/write workloads. Invoke it with:

```bash
python benchmarks/compare_dicts.py
```

It prints tables for read-heavy and write-heavy scenarios. Adjust threads,
operation counts, and key-space with `--threads`, `--operations`, and
`--key-space`.

## Queue

`compare_queues.py` benchmarks `syncx.collections.Queue` versus `queue.Queue` using
paired producer and consumer threads. Run:

```bash
python benchmarks/compare_queues.py
```

Tweak the workload with `--pairs` (producer/consumer pairs), `--messages`
(messages per producer), and `--maxsize` (queue capacity, 0 for unbounded).

## Locks

`compare_locks.py` measures lock acquisition throughput for the mutexes and reader-
writer locks in `syncx.locks`, alongside their `threading` counterparts. Invoke it
with:

```bash
python benchmarks/compare_locks.py
```

It emits three sections: exclusive mutex contention, re-entrant locking, and
reader/write workloads. Adjust iterations and participants with `--threads`,
`--iterations`, `--reentrant-depth`, `--rw-readers`, `--rw-writers`, and
`--rw-operations`.
