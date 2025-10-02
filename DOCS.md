# syncx Documentation

syncx exposes Rust concurrency primitives to Python. The initial release focuses on lock-free atomics and selected types from `parking_lot`, with an eye toward growing the pallet over time.

## Installation

Build from source for local development:

```bash
maturin develop --release
```

Or install from PyPI once packages are published:

```bash
pip install syncx
```

## Module layout

- `syncx.atomic` provides `AtomicInt`, `AtomicBool`, and `AtomicFloat`. These mirror the Python-friendly surface from the original atomicx crate and always use `SeqCst` ordering for predictability across interpreters.
- `syncx.locks` wraps `parking_lot` mutexes, reentrant mutexes, and read-write locks. Acquire methods follow Python's `threading` naming (`acquire`, `release`) while still returning guard objects for explicit scoping. Locks support `with lock:` usage via context manager integration.

## Usage snippets

### Atomics

```python
from syncx.atomic import AtomicInt

counter = AtomicInt()

counter.inc()
assert counter.load() == 1

# CAS updates return (success, previous value)
assert counter.compare_exchange(1, 5) == (True, 1)
```

### Lock helpers

```python
from syncx.locks import Lock, RWLock, RLock

mutex = Lock()
with mutex:
    ...

rw = RWLock()
with rw.acquire_read():
    ...

writer = rw.acquire_write()
# Do work under exclusive access
writer.release()

rlock = RLock()
with rlock:
    with rlock:
        ...
```

## Future work

syncx aims to layer in additional Rust concurrency crates (e.g., `crossbeam`, `dashmap`, channels) while keeping API ergonomics familiar for Python developers. Contributions and ideas are welcome.
