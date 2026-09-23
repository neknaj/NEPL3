"""Strict ELF32/ARM load-segment to RP2040 UF2 conversion, profile version 1."""
import argparse
from pathlib import Path
import struct

from elf import segments

FLASH = 0x10000000
SIZE = 2 * 1024 * 1024
FAMILY = 0xE48BFF56


def convert(elf: bytes | bytearray) -> bytes:
    pages: dict[int, bytearray] = {}
    used: set[int] = set()
    for segment in segments(elf):
        if segment.kind != 1 or not segment.size:
            continue
        offset, address, size, memsize = segment.offset, segment.address, segment.size, segment.memory_size
        if size > memsize or offset + size > len(elf) or not FLASH <= address < address + size <= FLASH + SIZE:
            raise ValueError("load segment outside RP2040 flash image")
        for index, byte in enumerate(elf[offset:offset + size], address):
            if index in used:
                raise ValueError("overlapping ELF load segments")
            used.add(index)
            page = index & ~255
            pages.setdefault(page, bytearray(256))[index - page] = byte
    if FLASH not in pages or FLASH + 256 not in pages:
        raise ValueError("missing boot2 or vector table")
    output = bytearray()
    for number, (address, data) in enumerate(sorted(pages.items())):
        output += struct.pack("<8I", 0x0A324655, 0x9E5D5157, 0x2000, address, 256, number, len(pages), FAMILY)
        output += data + bytes(220) + struct.pack("<I", 0x0AB16F30)
    return bytes(output)


class Arguments(argparse.Namespace):
    elf: Path = Path()
    uf2: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser()
    _ = parser.add_argument("elf", type=Path)
    _ = parser.add_argument("uf2", type=Path)
    args = parser.parse_args(namespace=Arguments())
    _ = args.uf2.write_bytes(convert(args.elf.read_bytes()))


if __name__ == "__main__":
    main()
