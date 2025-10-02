"""syncx exposes Rust concurrency primitives to Python."""

from . import atomic, dict as _dict_module, locks, queue

dict = _dict_module

__all__ = ["atomic", "dict", "locks", "queue"]
