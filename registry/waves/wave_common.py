"""Shared fixture selection and atomic publication for wave producers.

A failed serialization or replacement leaves the previous manifest intact.
Generated JSON formatting and existing output permissions are preserved.
"""

import json
import os
import pathlib
import tempfile
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURE = os.environ.get("RAVEL_FIXTURE_ID", "atm10-tts-2.0.2")


def write_json(path: pathlib.Path, document: Any) -> None:
    content = json.dumps(document, indent=2, sort_keys=True) + "\n"
    with tempfile.TemporaryDirectory(prefix=f".{path.name}.", dir=path.parent) as directory:
        temporary = pathlib.Path(directory) / path.name
        temporary.write_text(content)
        if path.exists():
            temporary.chmod(path.stat().st_mode & 0o7777)
        temporary.replace(path)
