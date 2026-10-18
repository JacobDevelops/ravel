"""Isolate the benchmark observer's ASM without changing classfile layout."""


def relocate_class(data: bytes) -> bytes:
    """Relocate ASM names in UTF8 constants, rejecting malformed constant pools."""
    if len(data) < 10 or data[:4] != b"\xca\xfe\xba\xbe":
        raise ValueError("invalid or truncated classfile header")
    count = int.from_bytes(data[8:10], "big")
    if count == 0:
        raise ValueError("invalid zero constant-pool count")
    sizes = {3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4, 10: 4,
             11: 4, 12: 4, 15: 3, 16: 2, 17: 4, 18: 4, 19: 2, 20: 2}
    result = bytearray(data)
    position, slot = 10, 1
    while slot < count:
        if position >= len(data):
            raise ValueError("truncated constant-pool tag")
        tag = data[position]
        position += 1
        if tag == 1:
            if position + 2 > len(data):
                raise ValueError("truncated UTF8 constant length")
            length = int.from_bytes(data[position:position + 2], "big")
            position += 2
        elif tag in sizes:
            length = sizes[tag]
        else:
            raise ValueError("unknown constant-pool tag: " + str(tag))
        end = position + length
        if end > len(data):
            raise ValueError("truncated constant-pool payload")
        if tag == 1:
            result[position:end] = data[position:end].replace(b"org/objectweb/asm", b"ravel/metrics/asm")
        slot += 2 if tag in (5, 6) else 1
        if slot > count:
            raise ValueError("long/double constant has no reserved slot")
        position = end
    return bytes(result)
