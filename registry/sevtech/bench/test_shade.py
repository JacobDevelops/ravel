"""Synthetic classfile coverage for the observer's dependency relocation."""
import unittest

from shade import relocate_class


def classfile(entries, suffix=b""):
    slots = sum(2 if entry[0] in (5, 6) else 1 for entry in entries)
    return b"\xca\xfe\xba\xbe\x00\x00\x00\x34" + (slots + 1).to_bytes(2, "big") + b"".join(entries) + suffix


def utf8(value):
    return b"\x01" + len(value).to_bytes(2, "big") + value


class RelocateClassTests(unittest.TestCase):
    def test_relocates_only_utf8_payloads(self):
        original = b"Lorg/objectweb/asm/ClassReader;org/objectweb/asm/Type"
        relocated = b"Lravel/metrics/asm/ClassReader;ravel/metrics/asm/Type"
        suffix = b"method-code:org/objectweb/asm/DoNotTouch"
        entries = [utf8(original), utf8(b"unchanged"), b"\x03\x01\x02\x03\x04"]
        source = classfile(entries, suffix)
        expected = classfile([utf8(relocated), *entries[1:]], suffix)
        self.assertEqual(relocate_class(source), expected)
        self.assertEqual(len(relocate_class(source)), len(source))
        self.assertEqual(relocate_class(expected), expected)

    def test_accepts_all_fixed_width_tags_and_reserved_slots(self):
        sizes = {3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4, 10: 4,
                 11: 4, 12: 4, 15: 3, 16: 2, 17: 4, 18: 4, 19: 2, 20: 2}
        entries = [bytes([tag]) + bytes(length) for tag, length in sizes.items()]
        entries += [utf8(b"org/objectweb/asm/Type")]
        source = classfile(entries, b"org/objectweb/asm")
        expected = classfile(entries[:-1] + [utf8(b"ravel/metrics/asm/Type")], b"org/objectweb/asm")
        self.assertEqual(relocate_class(source), expected)

    def test_rejects_malformed_headers(self):
        for source in (b"", bytes(10), b"\xca\xfe\xba\xbe", classfile([])[:8] + b"\x00\x00"):
            with self.subTest(source=source), self.assertRaises(ValueError):
                relocate_class(source)

    def test_rejects_every_truncated_constant_pool_prefix(self):
        entries = [utf8(b"org/objectweb/asm/Type"), b"\x05" + bytes(8), b"\x12" + bytes(4)]
        source = classfile(entries)
        for end in range(10, len(source)):
            with self.subTest(end=end), self.assertRaises(ValueError):
                relocate_class(source[:end])

    def test_rejects_unknown_tag(self):
        for tag in (0, 2, 13, 14, 21, 255):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                relocate_class(classfile([bytes([tag])]))

    def test_rejects_long_double_without_reserved_slot(self):
        for tag in (5, 6):
            source = classfile([])[:8] + b"\x00\x02" + bytes([tag]) + bytes(8)
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                relocate_class(source)


if __name__ == "__main__":
    unittest.main()
