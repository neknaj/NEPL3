"""Strict ELF32/ARM load-segment to RP2040 UF2 conversion, profile version 1."""
import argparse
from pathlib import Path
import struct

FLASH = 0x10000000
SIZE = 2 * 1024 * 1024
FAMILY = 0xE48BFF56


def convert(elf):
    if len(elf) < 52 or elf[:7] != b"\x7fELF\x01\x01\x01":
        raise ValueError("expected ELF32 little endian")
    _, kind, machine, version, _, phoff, _, _, ehsize, phsize, count, _, _, _ = struct.unpack_from("<16sHHIIIIIHHHHHH", elf)
    if (kind, machine, version, ehsize, phsize) != (2, 40, 1, 52, 32) or not count or phoff + count * phsize > len(elf):
        raise ValueError("invalid ARM executable headers")
    pages, used = {}, set()
    for i in range(count):
        typ, offset, _, address, size, memsize, _, _ = struct.unpack_from("<8I", elf, phoff + i * phsize)
        if typ != 1 or not size:
            continue
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


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("elf", type=Path)
    parser.add_argument("uf2", type=Path)
    args = parser.parse_args()
    args.uf2.write_bytes(convert(args.elf.read_bytes()))
