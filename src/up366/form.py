from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Any


def form_encode(data: Mapping[str, Any]) -> dict[str, str]:
    """Encode a request body the way the official PC client does.

    Keys are sent with ``###`` flattened to ``.`` and structured values are
    serialized as a single JSON form value.
    """
    encoded: dict[str, str] = {}
    for key, value in data.items():
        encoded[key.replace("###", ".")] = (
            json.dumps(value, ensure_ascii=False)
            if isinstance(value, (Mapping, list))
            else str(value)
        )
    return encoded
