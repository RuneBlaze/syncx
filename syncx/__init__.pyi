"""syncx exposes Rust concurrency primitives to Python."""

from . import atomic, dict as _dict_module, locks, queue, set as _set_module

__version__: str

dict = _dict_module
set = _set_module

__all__ = ["atomic", "dict", "locks", "queue", "set", "__version__"]
