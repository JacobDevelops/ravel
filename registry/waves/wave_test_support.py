"""Small attribute patching utility without importing asyncio beside select.py."""

import contextlib
from collections.abc import Iterator
from typing import Any


@contextlib.contextmanager
def patched_attribute(target: Any, name: str, value: Any) -> Iterator[None]:
    previous = getattr(target, name)
    setattr(target, name, value)
    try:
        yield
    finally:
        setattr(target, name, previous)
