from concurrent.futures import ThreadPoolExecutor
import time

import pytest

from syncx.locks import Lock, RLock, RWLock


def test_lock_basic_guard_lifecycle():
    mutex = Lock()

    guard = mutex.guard()
    assert guard is not None
    assert mutex.is_locked() is True

    guard.release()
    assert mutex.is_locked() is False

    with mutex:
        assert mutex.locked() is True
        assert mutex.is_locked() is True

    assert mutex.is_locked() is False


def test_lock_try_acquire_contention():
    mutex = Lock()

    assert mutex.acquire() is True
    assert mutex.try_acquire() is False

    mutex.release()

    assert mutex.try_acquire() is True
    mutex.release()


def test_rwlock_allows_multiple_readers_and_exclusive_writer():
    lock = RWLock()

    read_guard = lock.read_guard()
    assert read_guard is not None
    second_reader = lock.read_guard(blocking=False)
    assert second_reader is not None

    second_reader.release()

    assert lock.try_acquire_write() is False

    read_guard.release()

    with lock.write_guard() as writer:
        assert writer is not None
        assert lock.is_locked() is True
        assert lock.try_acquire_read() is False

    assert lock.is_locked() is False


def test_rwlock_writer_blocks_readers():
    lock = RWLock()

    def hold_write():
        with lock.write_guard():
            time.sleep(0.1)

    def attempt_read_then_release():
        guard = lock.read_guard(blocking=False)
        if guard is None:
            return False
        guard.release()
        return True

    with ThreadPoolExecutor(max_workers=2) as pool:
        pool.submit(hold_write)
        time.sleep(0.02)
        future = pool.submit(attempt_read_then_release)

    assert future.result() is False


@pytest.mark.parametrize("mode", ["read", "write"])
def test_rwlock_context_manager(mode):
    lock = RWLock()

    ctx = lock.read_guard if mode == "read" else lock.write_guard

    with ctx() as guard:
        assert guard is not None
        assert lock.is_locked() is True

    assert lock.is_locked() is False


def test_rlock_allows_recursive_acquire():
    lock = RLock()

    guard1 = lock.acquire()
    guard2 = lock.acquire()

    guard2.release()
    guard1.release()

    with lock:
        with lock:
            assert True
