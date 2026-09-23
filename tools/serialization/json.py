"""Decode JSON into a bounded set of Python value types.

JsonValue is an external representation. Consumers validate fields and construct
their own domain records before performing internal operations.
"""

from collections.abc import Mapping, Sequence
import json
from typing import cast

type JsonValue = None | bool | int | float | str | list[JsonValue] | dict[str, JsonValue]


def _value(value: object) -> JsonValue:
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, list):
        # The decoder owns the collection; every element is validated below.
        items = cast(list[object], value)
        return [_value(item) for item in items]
    if isinstance(value, dict):
        items = cast(dict[object, object], value)
        result: dict[str, JsonValue] = {}
        for key, item in items.items():
            if not isinstance(key, str):
                raise ValueError("JSON object key must be a string")
            result[key] = _value(item)
        return result
    raise ValueError("unsupported decoded JSON value")


def _unique(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _nonfinite(value: str) -> float:
    raise ValueError(f"nonfinite JSON number: {value}")


def _finite_float(value: str) -> float:
    result = float(value)
    # Avoid importing math here: direct generator entrypoints include their
    # directory on sys.path, where tools/generate/math.py owns that name.
    if not -float("inf") < result < float("inf"):
        return _nonfinite(value)
    return result


def decode(
    source: str | bytes,
    *,
    reject_duplicates: bool = False,
    reject_nonfinite: bool = False,
) -> JsonValue:
    """Preserve stdlib policies unless the calling contract requests rejection."""
    decoded: object = json.loads(  # pyright: ignore[reportAny]
        source,
        object_pairs_hook=_unique if reject_duplicates else None,
        parse_constant=_nonfinite if reject_nonfinite else None,
        parse_float=_finite_float if reject_nonfinite else None,
    )
    return _value(decoded)


def object_value(value: JsonValue) -> Mapping[str, JsonValue]:
    if not isinstance(value, dict):
        raise ValueError("expected JSON object")
    return value


def array(value: JsonValue) -> Sequence[JsonValue]:
    if not isinstance(value, list):
        raise ValueError("expected JSON array")
    return value


def string(value: JsonValue) -> str:
    if not isinstance(value, str):
        raise ValueError("expected JSON string")
    return value


def integer(value: JsonValue) -> int:
    if type(value) is not int:
        raise ValueError("expected JSON integer")
    return value
