"""Typed TOML input and format-preserving editing boundaries.

Decoded values are external representations; consumers construct domain records
from the fields they use. Editing retains TOML Kit's original document objects.
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping, Sequence
from datetime import date, datetime, time
import tomllib
from typing import cast

import tomlkit
from tomlkit.container import Container, OutOfOrderTableProxy
from tomlkit.items import AbstractTable

type TomlValue = bool | int | float | str | date | datetime | time | list[TomlValue] | dict[str, TomlValue]
type Editable = Container | AbstractTable | OutOfOrderTableProxy


def _value(value: object) -> TomlValue:
    if isinstance(value, (bool, int, float, str, date, time)):
        return value
    if isinstance(value, list):
        items = cast(list[object], value)
        return [_value(item) for item in items]
    if isinstance(value, dict):
        items = cast(dict[object, object], value)
        result: dict[str, TomlValue] = {}
        for key, item in items.items():
            if not isinstance(key, str):
                raise ValueError("TOML table key must be a string")
            result[key] = _value(item)
        return result
    raise ValueError("unsupported decoded TOML value")


def decode(source: str) -> Mapping[str, TomlValue]:
    decoded: object = tomllib.loads(source)
    return table(_value(decoded))


def table(value: TomlValue) -> Mapping[str, TomlValue]:
    if not isinstance(value, dict):
        raise ValueError("expected TOML table")
    return value


def array(value: TomlValue) -> Sequence[TomlValue]:
    if not isinstance(value, list):
        raise ValueError("expected TOML array")
    return value


def string(value: TomlValue) -> str:
    if not isinstance(value, str):
        raise ValueError("expected TOML string")
    return value


class Table:
    """A mutable external TOML table, preserving formatting during edits."""

    def __init__(self, value: Editable) -> None:
        self._value: Editable = value

    def value(self, key: str) -> object:
        return self._value[key]

    def contains(self, key: str) -> bool:
        return key in self._value

    def keys(self) -> tuple[str, ...]:
        # TOML Kit's proxy iterator lacks element annotations. Validate each key.
        keys = cast(Iterable[object], self._value)
        result: list[str] = []
        for key in keys:
            if not isinstance(key, str):
                raise ValueError("TOML table key must be a string")
            result.append(key)
        return tuple(result)

    def set(self, key: str, value: str | Sequence[str]) -> None:
        self._value[key] = value if isinstance(value, str) else list(value)

    def remove(self, key: str) -> None:
        del self._value[key]

    def child(self, key: str) -> Table:
        return editable_table(self.value(key))

    def render(self) -> str:
        if not isinstance(self._value, Container):
            raise ValueError("serialize the owning TOML document")
        return self._value.as_string()


def editable_table(value: object) -> Table:
    if not isinstance(value, (Container, AbstractTable, OutOfOrderTableProxy)):
        raise ValueError("expected editable TOML table")
    return Table(value)


def optional_table(value: object) -> Table | None:
    """Return the table view only when the external value has table shape."""
    if isinstance(value, (Container, AbstractTable, OutOfOrderTableProxy)):
        return Table(value)
    return None


def parse(source: str) -> Table:
    return Table(tomlkit.parse(source))
