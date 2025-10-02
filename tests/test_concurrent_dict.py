from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import pickle

import pytest

from syncx.dict import ConcurrentDict


def test_basic_kv_operations() -> None:
    cd = ConcurrentDict()
    assert len(cd) == 0
    assert not cd

    cd["a"] = 1
    assert len(cd) == 1
    assert cd["a"] == 1
    assert "a" in cd

    with pytest.raises(KeyError):
        _ = cd["missing"]

    with pytest.raises(KeyError):
        del cd["missing"]

    del cd["a"]
    assert len(cd) == 0


def test_get_and_defaults() -> None:
    cd = ConcurrentDict()

    assert cd.get("missing") is None
    assert cd.get("missing", 42) == 42

    assert cd.setdefault("alpha", 1) == 1
    assert cd["alpha"] == 1
    assert cd.setdefault("alpha", 2) == 1

    assert cd.pop("alpha") == 1
    with pytest.raises(KeyError):
        cd.pop("alpha")
    assert cd.pop("alpha", 99) == 99

    cd["beta"] = 2
    cd.clear()
    assert len(cd) == 0
    assert "beta" not in cd


def test_threaded_updates_are_visible() -> None:
    cd: ConcurrentDict[str, int] = ConcurrentDict()

    def writer(chunk: int, base: int) -> None:
        for offset in range(chunk):
            key = f"k{base + offset}"
            cd[key] = base + offset
            assert cd[key] == base + offset

    workers = 4
    chunk = 25
    with ThreadPoolExecutor(max_workers=workers) as executor:
        futures = [executor.submit(writer, chunk, i * chunk) for i in range(workers)]
        for fut in futures:
            fut.result()

    assert len(cd) == workers * chunk


def test_pickle_roundtrip() -> None:
    cd = ConcurrentDict()
    cd["alpha"] = 1
    cd["beta"] = 2

    payload = pickle.dumps(cd)
    restored = pickle.loads(payload)

    assert isinstance(restored, ConcurrentDict)
    assert restored["alpha"] == 1
    assert restored["beta"] == 2

    cd["alpha"] = 10
    assert restored["alpha"] == 1

    restored["beta"] = 20
    assert cd["beta"] == 2


def test_unhashable_key_raises() -> None:
    cd = ConcurrentDict()
    with pytest.raises(TypeError):
        cd[[1, 2, 3]] = 10

    with pytest.raises(TypeError):
        _ = cd.get([1, 2, 3])

    with pytest.raises(TypeError):
        _ = cd.pop([1, 2, 3])
