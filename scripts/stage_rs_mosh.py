#!/usr/bin/env python3
"""Stage the checksummed Mosh runtime matching a built RS executable's architecture."""
import argparse
import hashlib
from pathlib import Path
import struct
import tempfile


def target_for(executable: Path) -> tuple[str, str]:
    with executable.open('rb') as stream:
        header = stream.read(64)
        if header[:4] == b'\xcf\xfa\xed\xfe':
            machine = struct.unpack_from('<I', header, 4)[0]
            arch = {0x100000C: 'arm64', 0x1000007: 'x86_64'}.get(machine)
            platform, name = 'macos', 'libnauterm_mosh_ffi.dylib'
        elif header[:4] == b'\x7fELF' and header[4:6] == b'\x02\x01':
            arch = {183: 'arm64', 62: 'x86_64'}.get(struct.unpack_from('<H', header, 18)[0])
            platform, name = 'linux', 'libnauterm_mosh_ffi.so'
        elif header[:2] == b'MZ' and len(header) == 64:
            offset = struct.unpack_from('<I', header, 60)[0]
            if offset > 16 * 1024 * 1024:
                raise ValueError('Invalid PE header offset')
            stream.seek(offset)
            pe = stream.read(6)
            if pe[:4] != b'PE\0\0' or len(pe) != 6:
                raise ValueError('Invalid PE header')
            arch = {0xAA64: 'arm64', 0x8664: 'x86_64'}.get(struct.unpack_from('<H', pe, 4)[0])
            platform, name = 'windows', 'nauterm_mosh_ffi.dll'
        else:
            raise ValueError('Unsupported RS executable format')
    if arch is None:
        raise ValueError('Unsupported RS executable architecture')
    return f'{platform}-{arch}', name


def stage(project: Path, executable: Path, destination: Path | None = None) -> Path:
    platform, name = target_for(executable)
    relative = Path('third_party/nauterm_mosh_ffi') / platform / name
    source = project / relative
    expected = dict((line.split()[1].lstrip('*'), line.split()[0])
                    for line in (project / 'third_party/nauterm_mosh_ffi/SHA256SUMS').read_text().splitlines()
                    if line.strip()).get(relative.as_posix())
    if source.is_symlink() or not source.is_file() or not expected:
        raise ValueError('Pinned Mosh runtime is missing or invalid')
    data = source.read_bytes()
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError('Pinned Mosh runtime checksum mismatch')
    destination = destination if destination is not None else executable.parent
    destination.mkdir(parents=True, exist_ok=True)
    target = destination / name
    if target.is_file() and not target.is_symlink() and hashlib.sha256(target.read_bytes()).hexdigest() == expected:
        return target
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(prefix='.rs-mosh-', dir=destination, delete=False) as output:
            temporary = Path(output.name)
            output.write(data)
        temporary.chmod(0o755)
        temporary.replace(target)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return target


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', type=Path, required=True)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--destination', type=Path)
    args = parser.parse_args()
    print(stage(args.project.resolve(), args.executable.resolve(), args.destination))
