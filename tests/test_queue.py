from __future__ import annotations

import threading
import time

import pytest

from syncx.collections import Empty, Full, Queue


def test_queue_round_trip_preserves_identity() -> None:
    queue = Queue()
    sentinel = object()

    queue.put(sentinel)
    assert queue.qsize() == 1
    assert not queue.empty()
    assert queue.get() is sentinel
    assert queue.empty()


def test_queue_full_and_empty_exceptions() -> None:
    queue = Queue(maxsize=1)

    queue.put("alpha")
    assert queue.full()
    with pytest.raises(Full):
        queue.put_nowait("beta")

    assert queue.get() == "alpha"
    with pytest.raises(Empty):
        queue.get_nowait()


def test_queue_timeout_behaviour() -> None:
    queue = Queue(maxsize=1)
    queue.put("held")

    start = time.perf_counter()
    with pytest.raises(Full):
        queue.put("blocked", timeout=0.05)
    elapsed = time.perf_counter() - start
    assert elapsed >= 0.04

    assert queue.get() == "held"

    start = time.perf_counter()
    with pytest.raises(Empty):
        queue.get(timeout=0.05)
    elapsed = time.perf_counter() - start
    assert elapsed >= 0.04


def test_queue_blocking_put_releases_gil() -> None:
    queue = Queue(maxsize=1)
    queue.put("seed")

    completed = threading.Event()

    def producer() -> None:
        queue.put("payload")
        completed.set()

    worker = threading.Thread(target=producer)
    worker.start()

    assert not completed.wait(0.05)
    assert queue.get() == "seed"
    assert completed.wait(1.0)
    assert queue.get() == "payload"
    worker.join(timeout=1.0)


def test_queue_threaded_flow() -> None:
    queue = Queue()
    produced = 50
    results: list[int] = []

    def producer() -> None:
        for value in range(produced):
            queue.put(value)

    def consumer() -> None:
        for _ in range(produced):
            results.append(queue.get())

    producer_thread = threading.Thread(target=producer)
    consumer_thread = threading.Thread(target=consumer)

    producer_thread.start()
    consumer_thread.start()

    producer_thread.join(timeout=1.0)
    consumer_thread.join(timeout=1.0)

    assert sorted(results) == list(range(produced))
