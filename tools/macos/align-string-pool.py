#!/usr/bin/env python3
"""Restore pointer alignment of a Mach-O image's LINKEDIT string pool.

Apple's linker writes the string pool directly after the indirect symbol
table, which holds four bytes per entry. When that table has an odd number of
entries the string pool starts four bytes off a pointer boundary, and the
image is rejected with "mis-aligned LINKEDIT string pool" -- by `ld` when it
links against the dylib and by `dyld` when the dylib is loaded. Neither
`strip` nor a different linker flag changes the layout, so the missing padding
is re-emitted here:

  * move the string pool to the next eight byte boundary,
  * pad the end of the string pool so the LINKEDIT data that follows (the code
    signature) also starts on a boundary,
  * shift every file offset that points past the inserted bytes,
  * drop the stale code signature and sign the file again.

The tool is a no-op on an image that is already aligned, so it can run on every
build. Only 64-bit little-endian Mach-O files are handled.

Usage: align-string-pool.py <mach-o>...
"""

from __future__ import annotations

import shutil
import struct
import subprocess
import sys
from pathlib import Path

MH_MAGIC_64 = 0xFEEDFACF
MH_HEADER_64 = struct.Struct("<8I")
SEGMENT_COMMAND_64 = struct.Struct("<II16sQQQQIIII")
LINKEDIT_DATA_COMMAND = struct.Struct("<IIII")

LC_SEGMENT_64 = 0x19
LC_SYMTAB = 0x02
LC_CODE_SIGNATURE = 0x1D

# Every load command that carries a file offset into LINKEDIT. They have to
# move along with the bytes that are inserted ahead of them.
LINKEDIT_OFFSET_COMMANDS = frozenset(
    {
        0x1D,  # LC_CODE_SIGNATURE
        0x1E,  # LC_DYLD_INFO
        0x22,  # LC_DYLD_INFO_ONLY
        0x26,  # LC_FUNCTION_STARTS
        0x29,  # LC_DATA_IN_CODE
        0x2B,  # LC_DYLIB_CODE_SIGN_DRS
        0x2E,  # LC_LINKER_OPTIMIZATION_HINT
        0x80000033,  # LC_DYLD_EXPORTS_TRIE
        0x80000034,  # LC_DYLD_CHAINED_FIXUPS
    }
)

POINTER_ALIGNMENT = 8
PAGE_SIZE = 0x4000


def fail(path: Path, message: str) -> None:
    raise SystemExit(f"align-string-pool: {path}: {message}")


def load_commands(blob: bytes, path: Path):
    """Yield (offset, command, size) for every load command of a Mach-O."""
    magic, _, _, _, ncmds, sizeofcmds, _, _ = MH_HEADER_64.unpack_from(blob, 0)
    if magic != MH_MAGIC_64:
        fail(path, "not a 64-bit little-endian Mach-O")
    end = MH_HEADER_64.size + sizeofcmds
    offset = MH_HEADER_64.size
    for _ in range(ncmds):
        if offset + 8 > end:
            fail(path, "load commands run past sizeofcmds")
        command, size = struct.unpack_from("<II", blob, offset)
        if size < 8 or offset + size > len(blob):
            fail(path, f"load command {command:#x} has an invalid size")
        yield offset, command, size
        offset += size


def layout(blob: bytes, path: Path) -> dict:
    """Collect the LINKEDIT facts the padding depends on."""
    facts: dict = {"symtab": None, "signature": None, "linkedit_segment": None, "offsets": []}
    for offset, command, size in load_commands(blob, path):
        if command == LC_SYMTAB:
            _, _, symoff, nsyms, stroff, strsize = struct.unpack_from("<6I", blob, offset)
            facts["symtab"] = (offset, symoff, nsyms, stroff, strsize)
        elif command == LC_SEGMENT_64:
            fields = SEGMENT_COMMAND_64.unpack_from(blob, offset)
            name = fields[2].rstrip(b"\0").decode("ascii", "replace")
            if name == "__LINKEDIT":
                facts["linkedit_segment"] = (offset, fields[4], fields[6])  # vmsize, filesize
        elif command in LINKEDIT_OFFSET_COMMANDS:
            dataoff, datasize = LINKEDIT_DATA_COMMAND.unpack_from(blob, offset)[2:4]
            facts["offsets"].append((offset, command, dataoff, datasize))
            if command == LC_CODE_SIGNATURE:
                facts["signature"] = (dataoff, datasize)
    if facts["symtab"] is None:
        fail(path, "no LC_SYMTAB")
    return facts


def remove_signature(path: Path) -> None:
    subprocess.run(
        ["codesign", "--remove-signature", str(path)],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )


def sign(path: Path) -> None:
    subprocess.run(
        ["codesign", "--force", "--sign", "-", str(path)],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )


def align(path: Path) -> bool:
    """Pad the string pool in place. Returns True when the file was changed."""
    blob = bytearray(path.read_bytes())
    facts = layout(blob, path)
    _, _, _, stroff, strsize = facts["symtab"]
    lead = (-stroff) % POINTER_ALIGNMENT
    tail = (-(stroff + lead + strsize)) % POINTER_ALIGNMENT
    if lead == 0 and tail == 0:
        print(f"aligned {path.name}")
        return False

    if shutil.which("codesign") is None:
        fail(path, "codesign is needed to sign the padded image")

    if facts["signature"] is not None:
        # The signature has to go first: it lives past the string pool, so
        # padding would strand it at an offset dyld also rejects, and codesign
        # would keep it there when it replaces the blob.
        remove_signature(path)
        blob = bytearray(path.read_bytes())
        facts = layout(blob, path)

    blob[stroff:stroff] = bytes(lead)
    string_pool_end = stroff + lead + strsize
    blob[string_pool_end:string_pool_end] = bytes(tail)
    start = facts["symtab"][0]
    struct.pack_into("<I", blob, start + 16, stroff + lead)
    struct.pack_into("<I", blob, start + 20, strsize + tail)
    for offset, _, dataoff, _ in facts["offsets"]:
        if dataoff > stroff + strsize:
            dataoff += lead + tail
        elif dataoff >= stroff:
            dataoff += lead
        struct.pack_into("<I", blob, offset + 8, dataoff)
    if facts["linkedit_segment"] is not None:
        offset, vmsize, filesize = facts["linkedit_segment"]
        filesize += lead + tail
        vmsize = max(vmsize, -(-filesize // PAGE_SIZE) * PAGE_SIZE)
        struct.pack_into("<Q", blob, offset + 32, vmsize)
        struct.pack_into("<Q", blob, offset + 48, filesize)
    path.write_bytes(blob)

    sign(path)
    blob = bytearray(path.read_bytes())
    _, _, _, stroff, strsize = layout(blob, path)["symtab"]
    if stroff % POINTER_ALIGNMENT or (stroff + strsize) % POINTER_ALIGNMENT:
        fail(path, "padding did not align the string pool")
    print(f"padded {path.name}: string pool {stroff - lead} -> {stroff} (+{lead}), size +{tail}")
    return True


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    for argument in argv[1:]:
        path = Path(argument)
        if not path.is_file():
            print(f"align-string-pool: {path}: no such file", file=sys.stderr)
            return 1
        align(path)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
