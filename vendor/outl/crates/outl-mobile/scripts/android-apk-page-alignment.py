#!/usr/bin/env python3
"""Fail the build if an APK carries a native library 16 KB pages cannot load.

Some Android devices page at 16 KB rather than 4 KB: the Pixel 8 and 9
families as a developer option, newer hardware by default, and Google Play
has required support since November 2025. There, `dlopen` refuses a `.so`
whose LOAD segments are aligned to 4 KB, and the app dies on its first
`System.loadLibrary` — for outl that
is `NativeSetup` in `MainActivity.onCreate`, so the whole app "crashes on
open" before any of our code runs. v0.12.0-beta.226 shipped that way.

Two separate alignments have to hold, from two different build steps, and
neither reports anything when it stops:

  * the ELF `p_align` of every PT_LOAD, from the linker flag in
    `.cargo/config.toml` (and from whoever built each vendored AAR);
  * the offset of the entry inside the zip, from `zipalign -P 16` in
    `release.yml`, which only matters for an uncompressed entry because
    those are mapped straight out of the APK.

Checking the finished APK covers both at once, plus any library arriving
through a dependency we do not build.

Usage: android-apk-page-alignment.py <apk>
"""

import struct
import sys
import zipfile

PAGE = 16 * 1024

# 16 KB pages are a 64-bit change; a 32-bit ABI keeps 4 KB.
ABIS_64_BIT = ("arm64-v8a", "x86_64")


def load_segment_aligns(head):
    """`p_align` of every PT_LOAD in a 64-bit little-endian ELF."""
    e_phoff = struct.unpack_from("<Q", head, 32)[0]
    e_phentsize, e_phnum = struct.unpack_from("<HH", head, 54)
    end = e_phoff + e_phnum * e_phentsize
    if end > len(head):
        raise ValueError(f"program headers end at {end}, past the {len(head)} bytes read")
    aligns = []
    for i in range(e_phnum):
        ph = e_phoff + i * e_phentsize
        if struct.unpack_from("<I", head, ph)[0] == 1:  # PT_LOAD
            aligns.append(struct.unpack_from("<Q", head, ph + 48)[0])
    return aligns


def payload_offset(apk_path, info):
    """Where the entry's bytes start: the local header is variable-length."""
    with open(apk_path, "rb") as f:
        f.seek(info.header_offset)
        name_len, extra_len = struct.unpack("<HH", f.read(30)[26:30])
    return info.header_offset + 30 + name_len + extra_len


def check(apk_path):
    failures = []
    checked = 0

    with zipfile.ZipFile(apk_path) as apk:
        entries = [i for i in apk.infolist() if i.filename.endswith(".so")]
        if not entries:
            return [f"{apk_path} holds no native libraries at all"]

        for info in entries:
            parts = info.filename.split("/")
            if len(parts) < 2 or parts[0] != "lib":
                continue
            if parts[1] not in ABIS_64_BIT:
                continue
            checked += 1

            with apk.open(info) as f:
                head = f.read(64 * 1024)
            if head[:4] != b"\x7fELF" or head[4] != 2:  # ELFCLASS64
                failures.append(f"{info.filename}: not a 64-bit ELF")
                continue

            for align in load_segment_aligns(head):
                if align < PAGE:
                    failures.append(
                        f"{info.filename}: LOAD segment aligned to {align} bytes, "
                        f"needs at least {PAGE}"
                    )
                    break

            # A deflated entry is unpacked on install, so only the ELF
            # alignment above governs it.
            if info.compress_type == zipfile.ZIP_STORED:
                offset = payload_offset(apk_path, info)
                if offset % PAGE:
                    failures.append(
                        f"{info.filename}: stored at zip offset {offset}, "
                        f"not a multiple of {PAGE}"
                    )

    if not checked:
        return [f"{apk_path} holds no 64-bit native libraries to check"]
    print(f"checked {checked} 64-bit native libraries in {apk_path}")
    return failures


def main():
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <apk>")

    failures = check(sys.argv[1])
    if failures:
        for line in failures:
            print(f"::error::16 KB page alignment: {line}")
        sys.exit(
            "this APK crashes on launch on any device with 16 KB pages; "
            "see the Android link flags in .cargo/config.toml"
        )
    print("every 64-bit native library is 16 KB page aligned")


if __name__ == "__main__":
    main()
