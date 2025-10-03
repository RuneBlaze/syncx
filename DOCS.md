# syncx API Reference

syncx exposes a compact set of Rust-backed concurrency primitives to Python. The package now groups its surface into two submodules—`atomic` and `collections`—so you can pick the tool you need without hunting through helper packages.

<div class="note">

**Interpreter support.** Wheels target CPython 3.9–3.13. Build free-threaded (`3.13t`) wheels with `PYTHON_GIL=0 maturin build --release --no-default-features` when you have a free-threaded interpreter available.

</div>

## Installation

### Local development build

```bash
uv run maturin develop --release
```

Compiles the extension in release mode and installs it into the current `uv` environment for immediate iteration.

### Stable release

```bash
pip install syncx
```

Upgrade in place with `pip install --upgrade syncx` once new wheels ship.

## Quick start

```python
from syncx.atomic import AtomicInt
from syncx.collections import Queue

counter = AtomicInt(41)
counter.inc()
assert counter.load() == 42

queue = Queue(maxsize=1)
queue.put("message")
assert queue.get_nowait() == "message"
```

## Module reference

### `syncx.atomic`

Sequentially consistent atomics that mirror the portable-atomic surface.

#### `AtomicInt`

- `AtomicInt(value: int = 0)` – create a zeroed atomic integer by default.
- `load() -> int` / `store(value: int) -> None`
- Arithmetic updaters: `add`, `sub`, `mul`, `div`, `inc`, `dec`
- Bitwise helpers: `fetch_and`, `fetch_or`, `fetch_xor`
- `compare_exchange(current, new) -> tuple[bool, int]` – returns the swap result and prior value.
- `update(callable) -> int` – retry loop around a Python function.

<div class="warning">

**Division safety.** `div` and in-place division raise `ZeroDivisionError` if the divisor is zero.

</div>

#### `AtomicBool`

Boolean guard built on `std::atomic::AtomicBool`.

- `AtomicBool(value: bool = False)`
- `load()` / `store()` / `swap()`
- `compare_exchange(current, new) -> tuple[bool, bool]`
- Logical fetches: `fetch_and`, `fetch_or`, `fetch_xor`
- `flip() -> bool` toggles the bit and returns the previous value.

#### `AtomicFloat`

Portable 64-bit float atomic aligned with `portable-atomic`.

- `AtomicFloat(value: float = 0.0)`
- `load()` / `store()` / `swap()`
- `add`, `sub`, `mul`, `div`
- `fetch_max(value)` / `fetch_min(value)` for monotonic clamps
- `compare_exchange(current, new)` and `update(callable)` mirror the integer API.

#### `AtomicReference`

Thread-safe reference slot that stores arbitrary Python objects.

- `AtomicReference(obj: object | None = None)`
- `get() -> object | None` clones the reference into Python space.
- `set(obj)` / `exchange(obj) -> object | None`
- `compare_exchange(expected, obj) -> bool` matches against object identity and equality (`__eq__`).

### `syncx.collections`

Thread-safe collections backed by `flume`, `DashMap`, and `DashSet`.

#### `Queue`

- `Queue(maxsize: int = 0)` – zero means unbounded.
- `put(item, block=True, timeout=None)` / `get(block=True, timeout=None)` match the standard library contract.
- `put_nowait(item)` / `get_nowait()` shortcuts.
- `qsize()` / `empty()` / `full()` mirror `queue.Queue`.
- `Empty` and `Full` exceptions are raised for non-blocking operations and timeouts.

<div class="note">

**Detaching from the GIL.** Blocking queue operations release the GIL while waiting so producers and consumers can progress concurrently.

</div>

#### `ConcurrentDict`

DashMap-backed dictionary with per-key sharding.

- `ConcurrentDict()` instantiates an empty map.
- `obj[key]`, `obj[key] = value`, `del obj[key]` follow standard mapping semantics.
- `iter(obj)` yields a live view of keys without materializing an intermediate list.
- Lock-free lookups: `get`, `setdefault`, `pop` (with optional defaults).
- `clear()` and length/truthiness helpers (`len(obj)`, `bool(obj)`).
- Pickle helpers: `__getstate__()` returns a Python `dict`; `__setstate__(state)` restores contents.

#### `ConcurrentSet`

DashSet-backed set for hashable Python values.

- `ConcurrentSet()` constructs an empty set.
- Membership via `value in obj`.
- `iter(obj)` streams members lazily without copying the entire set.
- Mutators: `add`, `discard`, `remove`, `clear`, `copy`.
- Pickle helpers move data through Python lists.

## Development & testing

- Format Rust code: `cargo fmt`
- Lint with `cargo clippy --all-targets -- -D warnings`
- Build the extension in place: `uv run maturin develop --release`
- Cross-version test matrix: `nox --default-venv-backend uv`
- Free-threaded session: `nox --default-venv-backend uv -s tests-free-threaded`

<div class="warning">

**Testing guidance.** Avoid busy-wait loops in new tests; prefer bounded thread pools and timeouts so suites remain stable under `pytest -n auto` and on CI runners.

</div>

## Further reading

- Repository: [https://github.com/RuneBlaze/syncx](https://github.com/RuneBlaze/syncx)
- Issue tracker: [https://github.com/RuneBlaze/syncx/issues](https://github.com/RuneBlaze/syncx/issues)
- Vendored PyO3 notes: `vibes/pyo3-guide`

Contributions are welcome—open an issue or PR if you spot gaps or want to expand the primitive set.
