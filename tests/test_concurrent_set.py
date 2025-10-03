from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import pickle

import pytest

from syncx.collections import ConcurrentSet


def test_basic_membership_and_mutation() -> None:
    cs = ConcurrentSet()
    assert len(cs) == 0
    assert not cs

    cs.add("alpha")
    assert len(cs) == 1
    assert "alpha" in cs

    cs.add("alpha")
    assert len(cs) == 1

    cs.discard("alpha")
    assert len(cs) == 0
    assert "alpha" not in cs

    cs.discard("missing")

    cs.add("beta")
    with pytest.raises(KeyError):
        cs.remove("gamma")

    cs.remove("beta")
    assert len(cs) == 0


def test_copy_creates_independent_snapshot() -> None:
    cs = ConcurrentSet()
    cs.add(1)
    cs.add(2)

    clone = cs.copy()
    assert isinstance(clone, ConcurrentSet)
    assert len(clone) == len(cs) == 2
    assert 1 in clone and 2 in clone

    clone.add(3)
    assert 3 in clone
    assert 3 not in cs

    cs.clear()
    assert len(cs) == 0
    assert len(clone) == 3


def test_threaded_additions() -> None:
    cs: ConcurrentSet[int] = ConcurrentSet()

    def writer(chunk: int, base: int) -> None:
        for offset in range(chunk):
            cs.add(base + offset)
            assert base + offset in cs

    workers = 4
    chunk = 25
    with ThreadPoolExecutor(max_workers=workers) as executor:
        futures = [executor.submit(writer, chunk, i * chunk) for i in range(workers)]
        for fut in futures:
            fut.result()

    assert len(cs) == workers * chunk


def test_pickle_roundtrip() -> None:
    cs = ConcurrentSet()
    cs.add("one")
    cs.add("two")

    payload = pickle.dumps(cs)
    restored = pickle.loads(payload)

    assert isinstance(restored, ConcurrentSet)
    assert "one" in restored
    assert "two" in restored

    cs.add("three")
    assert "three" not in restored


def test_unhashable_value_raises() -> None:
    cs = ConcurrentSet()
    with pytest.raises(TypeError):
        cs.add([])

    with pytest.raises(TypeError):
        cs.__contains__([])

    with pytest.raises(TypeError):
        cs.remove([])
