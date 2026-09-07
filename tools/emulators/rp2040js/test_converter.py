import struct
import unittest

from elf_to_uf2 import convert, FLASH


def image():
    header = struct.pack("<16sHHIIIIIHHHHHH", b"\x7fELF\x01\x01\x01", 2, 40, 1,
                         FLASH + 257, 52, 0, 0, 52, 32, 1, 0, 0, 0)
    segment = struct.pack("<8I", 1, 84, FLASH, FLASH, 512, 512, 5, 256)
    return bytearray(header + segment + bytes(range(256)) * 2)


class Converter(unittest.TestCase):
    def test_exact_payload_and_header(self):
        result = convert(image())
        self.assertEqual(len(result), 1024)
        for index in range(2):
            offset = index * 512
            self.assertEqual(struct.unpack_from("<8I", result, offset),
                             (0x0A324655, 0x9E5D5157, 0x2000, FLASH + index * 256,
                              256, index, 2, 0xE48BFF56))
            self.assertEqual(result[offset + 32:offset + 288], bytes(range(256)))
            self.assertEqual(struct.unpack_from("<I", result, offset + 508)[0], 0x0AB16F30)

    def test_malformed(self):
        for offset, value in [(0, 0), (4, 2), (5, 2), (16, 3), (18, 62),
                              (40, 0), (42, 0), (44, 0), (64, 0)]:
            with self.subTest(offset=offset):
                data = image()
                data[offset] = value
                # p_paddr low byte is initially zero; set the whole address.
                if offset == 64:
                    struct.pack_into("<I", data, offset, 0x20000000)
                with self.assertRaises(ValueError):
                    convert(data)
        for length in [0, 51, 83, 595]:
            with self.subTest(length=length), self.assertRaises(ValueError):
                convert(image()[:length])

    def test_size_and_overlap(self):
        data = image()
        struct.pack_into("<I", data, 72, 511)  # memsz < filesz
        with self.assertRaises(ValueError):
            convert(data)
        data = image()
        struct.pack_into("<H", data, 44, 2)
        struct.pack_into("<I", data, 56, 116)
        data[84:84] = data[52:84]
        with self.assertRaisesRegex(ValueError, "overlapping"):
            convert(data)


if __name__ == "__main__":
    unittest.main()
