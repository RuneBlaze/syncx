# Repository Guidelines

## Project Structure & Module Organization
The Rust core lives in `src/`, where `lib.rs` registers the Python module and is backed by `atomic.rs` and `locks.rs` for the exposed primitives. Type hints for downstream users sit under `syncx/` (`__init__.pyi`, `atomic.pyi`, `locks.pyi`); update these stubs alongside any API changes. Python-facing regression tests reside in `tests/`, driven by `pytest`. Packaging and release metadata is split between `Cargo.toml` for the Rust crate and `pyproject.toml` for the maturin build backend. Reference usage notes in `DOCS.md` when adjusting behavior or examples.

## Build, Test, and Development Commands
- Always prefix Python invocations with `uv` (e.g. `uv run`, `uv pip`) to ensure the managed environment is used.
- `uv run maturin develop --release`: build the extension and install it into the active virtualenv for local iteration.
- `maturin build --release`: produce `abi3` wheels for the standard GIL build; run before publishing.
- `PYTHON_GIL=0 maturin build --release --no-default-features`: emit free-threaded (`cp3xx`t) wheels using the same source; requires a free-threaded interpreter.
- `nox --default-venv-backend uv`: rebuild the extension and run pytest across the managed CPython matrix (sessions cover 3.9-3.13).
- `nox --default-venv-backend uv -s tests-free-threaded`: run the free-threaded suite (requires a 3.13t interpreter).
- `mise run test`: shorthand for the free-threaded Nox session.
- `cargo fmt` / `cargo clippy --all-targets -- -D warnings`: enforce Rust formatting and lint the CDylib before opening a PR.
- `rg vibes/pyo3-guide`: grab the vendored PyO3 guide; feel free to search it when confirming API patterns.

## Coding Style & Naming Conventions
Rust follows edition 2021 defaults: 4-space indentation, `snake_case` for functions, and `CamelCase` for PyO3 classes. Prefer explicit `SeqCst` ordering for new atomic operations to match existing semantics. Keep docstrings and Python examples concise and in imperative mood. When updating APIs, mirror signatures and docstrings in the `syncx/*.pyi` stubs and ensure public methods stay annotated.

## Testing Guidelines
Add new scenarios under `tests/` using `test_*` functions or `Test*` classes so `pytest` auto-discovers them. Cover both happy paths and contention cases; stress concurrent behavior with thread pools when feasible. Run `nox --default-venv-backend uv -- -n auto` (via `pytest-xdist`, optional) to validate thread-safety under parallel scheduling; append additional pytest flags after the `--` delimiter as needed (for example `-k atomic`). When touching Rust internals, add targeted unit tests or assert-based checks in Python to capture regressions in atomic semantics, and re-run the `tests-free-threaded` session when feasible.

## Commit & Pull Request Guidelines
Keep commits small and present-tense, echoing the existing history (`add float div guard`, `bump version`). Reference related issues in the commit body if context is non-obvious. PRs should include: a concise summary of the change surface, verification notes (`nox --default-venv-backend uv`, `uv run maturin develop --release`), and any relevant benchmarks or screenshots from docs. Request review once CI passes and the branch rebases cleanly on `main`.
