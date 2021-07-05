#!/usr/bin/env python3

# Parses glibc .abilist files into a machine readable data structure.

import json
import os
import pathlib
import sys


# glibc config -> (os, arch, subarch)
TARGETS_TO_SOURCES = {
    "aarch64-linux-gnu": [
        ("unix", "aarch64", []),
    ],
    "aarch64-linux-gnu-disable-multi-arch": [
        ("unix", "aarch64", []),
    ],
    "aarch64_be-linux-gnu": [
        ("unix", "aarch64", []),
    ],
    "alpha-linux-gnu": [
        ("unix", "alpha", []),
    ],
    "arc-linux-gnu": [
        ("unix", "arc", []),
    ],
    "arc-linux-gnuhf": [
        ("unix", "arc", []),
    ],
    "arceb-linux-gnu": [
        ("unix", "arc", []),
    ],
    "arm-linux-gnueabi": [
        ("unix", "arm", ["le"]),
    ],
    "arm-linux-gnueabi-v4t": [
        ("unix", "arm", ["le"]),
    ],
    "arm-linux-gnueabihf": [
        ("unix", "arm", ["le"]),
    ],
    "arm-linux-gnueabihf-v7a": [
        ("unix", "arm", ["le"]),
    ],
    "arm-linux-gnueabihf-v7a-disable-multi-arch": [
        ("unix", "arm", ["le"]),
    ],
    "armeb-linux-gnueabi": [
        ("unix", "arm", ["be"]),
    ],
    "armeb-linux-gnueabi-be8": [
        ("unix", "arm", ["be"]),
    ],
    "armeb-linux-gnueabihf": [
        ("unix", "arm", ["be"]),
    ],
    "armeb-linux-gnueabihf-be8": [
        ("unix", "arm", ["be"]),
    ],
    "csky-linux-gnuabiv2": [
        ("unix", "csky", []),
    ],
    "csky-linux-gnuabiv2-soft": [
        ("unix", "csky", []),
    ],
    "hppa-linux-gnu": [
        ("unix", "hppa", []),
    ],
    "i486-linux-gnu": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", "x32"),
    ],
    "i586-linux-gnu": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", "x32"),
    ],
    "i686-gnu": [
        ("mach", "hurd", []),
        ("mach", "hurd", ["i386"]),
    ],
    "i686-linux-gnu": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["x32"]),
    ],
    "i686-linux-gnu-disable-multi-arch": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["x32"]),
    ],
    "i686-linux-gnu-static-pie": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["x32"]),
    ],
    "ia64-linux-gnu": [
        ("unix", "ia64", []),
    ],
    "m68k-linux-gnu": [
        ("unix", "m68k", ["m680x0"]),
    ],
    "m68k-linux-gnu-coldfire": [
        ("unix", "m68k", ["coldfire"]),
    ],
    "m68k-linux-gnu-coldfire-soft": [
        ("unix", "m68k", ["coldfire"]),
    ],
    "microblaze-linux-gnu": [
        ("unix", "microblaze", []),
        ("unix", "microblaze", ["be"]),
    ],
    "microblazeel-linux-gnu": [
        ("unix", "microblaze", []),
        ("unix", "microblaze", ["le"]),
    ],
    "mips-linux-gnu": [
        ("unix", "mips", ["mips32"]),
        ("unix", "mips", ["mips32", "fpu"]),
    ],
    "mips-linux-gnu-nan2008": [
        ("unix", "mips", ["mips32"]),
        ("unix", "mips", ["mips32", "fpu"]),
    ],
    "mips-linux-gnu-nan2008-soft": [
        ("unix", "mips", ["mips32"]),
        ("unix", "mips", ["mips32", "nofpu"]),
    ],
    "mips-linux-gnu-soft": [
        ("unix", "mips", ["mips32"]),
        ("unix", "mips", ["mips32", "nofpu"]),
    ],
    # TODO define these
    "mips64-linux-gnu-n32": [],
    "mips64-linux-gnu-n32-nan2008": [],
    "mips64-linux-gnu-n32-nan2008-soft": [],
    "mips64-linux-gnu-n32-soft": [],
    "mips64-linux-gnu-n64": [],
    "mips64-linux-gnu-n64-nan2008": [],
    "mips64-linux-gnu-n64-nan2008-soft": [],
    "mips64-linux-gnu-n64-soft": [],
    "mips64el-linux-gnu-n32": [],
    "mips64el-linux-gnu-n32-nan2008": [],
    "mips64el-linux-gnu-n32-nan2008-soft": [],
    "mips64el-linux-gnu-n32-soft": [],
    "mips64el-linux-gnu-n64": [],
    "mips64el-linux-gnu-n64-nan2008": [],
    "mips64el-linux-gnu-n64-nan2008-soft": [],
    "mips64el-linux-gnu-n64-soft": [],
    "mipsel-linux-gnu": [],
    "mipsel-linux-gnu-nan2008": [],
    "mipsel-linux-gnu-nan2008-soft": [],
    "mipsel-linux-gnu-soft": [],
    "mipsisa32r6el-linux-gnu": [],
    "mipsisa64r6el-linux-gnu-n32": [],
    "mipsisa64r6el-linux-gnu-n64": [],

    "nios2-linux-gnu": [
        ("unix", "nios2", []),
    ],
    "powerpc-linux-gnu": [
        ("unix", "powerpc", ["powerpc32"]),
        ("unix", "powerpc", ["powerpc32", "fpu"]),
    ],
    "powerpc-linux-gnu-power4": [
        ("unix", "powerpc", ["powerpc32"]),
        ("unix", "powerpc", ["powerpc32", "fpu"]),
    ],
    "powerpc-linux-gnu-soft": [
        ("unix", "powerpc", ["powerpc32"]),
        ("unix", "powerpc", ["powerpc32", "nofpu"]),
    ],
    "powerpc64-linux-gnu": [
        ("unix", "powerpc", ["powerpc64", "be"]),
    ],
    "powerpc64le-linux-gnu": [
        ("unix", "powerpc", ["powerpc64", "le"]),
    ],
    "riscv32-linux-gnu-rv32imac-ilp32": [
        ("unix", "riscv", ["rv32"]),
    ],
    "riscv32-linux-gnu-rv32imac-ilp32d": [
        ("unix", "riscv", ["rv32"]),
    ],
    "riscv64-linux-gnu-rv64imac-lp64": [
        ("unix", "riscv", ["rv64"]),
    ],
    "riscv64-linux-gnu-rv64imafdc-lp64": [
        ("unix", "riscv", ["rv64"]),
    ],
    "riscv64-linux-gnu-rv64imafdc-lp64d": [
        ("unix", "riscv", ["rv64"]),
    ],
    "s390-linux-gnu": [
        ("unix", "s390", []),
        ("unix", "s390", ["s390-32"]),
    ],
    "s390x-linux-gnu": [
        ("unix", "s390", []),
        ("unix", "s390", ["s390-64"]),
    ],
    "s390x-linux-gnu-O3": [
        ("unix", "s390", []),
        ("unix", "s390", ["s390-64"]),
    ],
    "sh3-linux-gnu": [
        ("unix", "sh", ["le"]),
    ],
    "sh3eb-linux-gnu": [
        ("unix", "sh", ["be"]),
    ],
    "sh4-linux-gnu": [
        ("unix", "sh", ["le"]),
    ],
    "sh4-linux-gnu-soft": [
        ("unix", "sh", ["le"]),
    ],
    "sh4eb-linux-gnu": [
        ("unix", "sh", ["be"]),
    ],
    "sh4eb-linux-gnu-soft": [
        ("unix", "sh", ["be"]),
    ],
    "sparc64-linux-gnu": [
        ("unix", "sparc", ["sparc64"]),
    ],
    "sparc64-linux-gnu-disable-multi-arch": [
        ("unix", "sparc", ["sparc64"]),
    ],
    "sparcv8-linux-gnu-leon3": [
        ("unix", "sparc", ["sparc32"]),
    ],
    "sparcv9-linux-gnu": [
        ("unix", "sparc", ["sparc32"]),
    ],
    "sparcv9-linux-gnu-disable-multi-arch": [
        ("unix", "sparc", ["sparc32"]),
    ],
    "x86_64-linux-gnu": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["64"]),
    ],
    "x86_64-linux-gnu-disable-multi-arch": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["64"]),
    ],
    "x86_64-linux-gnu-static-pie": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["64"]),
    ],
    "x86_64-linux-gnu-x32": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["x32"]),
    ],
    "x86_64-linux-gnu-x32-static-pie": [
        ("unix", "x86_64", []),
        ("unix", "x86_64", ["x32"]),
    ],
}


def find_abilist_files(source: pathlib.Path) -> list[pathlib.Path]:
    """Finds .abilist files in a directory tree."""
    res = []

    for root, dirs, files in os.walk(source):
        root = pathlib.Path(root)

        # Make traversal deterministic.
        dirs.sort()

        for f in sorted(files):
            if not f.endswith(".abilist"):
                continue

            p = root / f

            # Ignore empty files.
            if p.stat().st_size == 0:
                continue

            res.append(p)

    return res


def parse_abilist(path: pathlib.Path):
    """Parse a .abilist file into a data structure."""
    functions = {}
    data = {}

    with path.open("r", encoding="ascii") as fh:
        for line in fh:
            parts = line.split()

            symver = parts[0]
            symbol = parts[1]
            typ = parts[2]

            if typ == "F":
                functions[symbol] = {"version": symver}
            elif typ == "D":
                address = parts[3]
                data[symbol] = {
                    "version": symver,
                    "address": address,
                }
            else:
                raise Exception("unhandled symbol type in %s: %s" % (path, parts))

    return {
        "functions": functions,
        "data": data,
    }


def abilist_metadata(source: pathlib.Path, abilist: pathlib.Path):
    """Resolve a .abilist path into metadata about that list."""

    rel = abilist.relative_to(source)
    assert rel.parts[0] == "sysdeps"
    assert rel.name.endswith(".abilist")

    os = rel.parts[1]
    assert os in ("generic", "mach", "unix")

    arch = None
    subarch = []

    if os == "mach":
        assert rel.parts[2] == "hurd"
        assert len(rel.parts) == 5

        arch = rel.parts[3]
    elif os == "unix":
        assert rel.parts[2] == "sysv"
        assert rel.parts[3] == "linux"

        arch = rel.parts[4]

        # There are additional path components that qualify this ABI.
        if len(rel.parts) > 6:
            subarch = list(rel.parts[5:-1])

    else:
        assert "unhandled os"

    lib = rel.name[:-8]

    return {
        "os": os,
        "arch": arch,
        "subarch": subarch,
        "lib": lib,
    }


def target_abi(target: str, source: pathlib.Path):
    """Resolves the ABI for a given named target."""

    assert target in TARGETS_TO_SOURCES

    libs = {}

    for p in find_abilist_files(source):
        meta = abilist_metadata(source, p)

        for os, arch, subarch in TARGETS_TO_SOURCES[target]:
            if meta["os"] != os or meta["arch"] != arch or meta["subarch"] != subarch:
                continue

            lib = meta["lib"]

            # Each library should only be defined once per target.
            assert lib not in libs

            libs[lib] = parse_abilist(p)

    if not libs:
        print("warning: no libraries found for %s" % target, file=sys.stderr)

    return libs


def main(source: pathlib.Path, dest: pathlib.Path):
    dest.mkdir(0o775, parents=True, exist_ok=True)

    for target in TARGETS_TO_SOURCES:
        abi = target_abi(target, source)
        dest_path = dest / ("%s.json" % target)

        with dest_path.open("w", encoding="utf-8") as fh:
            json.dump(abi, fh, indent=4, sort_keys=True)


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]))
