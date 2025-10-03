"""syncx exposes Rust concurrency primitives to Python."""

from . import atomic, collections, locks

__version__: str

__all__ = ["atomic", "collections", "locks", "__version__"]
