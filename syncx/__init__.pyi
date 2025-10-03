"""syncx exposes Rust concurrency primitives to Python."""

from . import atomic, collections

__version__: str

__all__ = ["atomic", "collections", "__version__"]
