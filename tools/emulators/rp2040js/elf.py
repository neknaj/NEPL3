"""Typed ELF32 little-endian decoding at the external binary boundary."""

from dataclasses import dataclass
import struct


@dataclass(frozen=True, slots=True)
class Segment:
    kind: int
    offset: int
    address: int
    size: int
    memory_size: int


def segments(source: bytes | bytearray) -> tuple[Segment, ...]:
    if len(source) < 52 or source[:7] != b"\x7fELF\x01\x01\x01":
        raise ValueError("expected ELF32 little endian")
    # Constant struct formats determine these element types and tuple lengths.
    header = struct.unpack_from("<16sHHIIIIIHHHHHH", source)
    _, kind, machine, version, _, phoff, _, _, ehsize, phsize, count, _, _, _ = header
    if (kind, machine, version, ehsize, phsize) != (2, 40, 1, 52, 32) or not count or phoff + count * phsize > len(source):
        raise ValueError("invalid ARM executable headers")
    result: list[Segment] = []
    for index in range(count):
        record = struct.unpack_from("<8I", source, phoff + index * phsize)
        typ, offset, _, address, size, memsize, _, _ = record
        result.append(Segment(typ, offset, address, size, memsize))
    return tuple(result)
