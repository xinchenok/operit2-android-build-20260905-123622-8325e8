"""Check an APK's ZIP integrity, architecture, required payload, and hash.

Cryptographic signature verification is performed separately by apksigner.
This check is not an installation or runtime test.
"""
from __future__ import annotations
import hashlib
import sys
import zipfile
from pathlib import Path

REQUIRED = {
    "AndroidManifest.xml",
    "lib/arm64-v8a/libflutter.so",
    "lib/arm64-v8a/libapp.so",
    "lib/arm64-v8a/liboperit_flutter_bridge.so",
    "lib/arm64-v8a/libbash.so",
    "lib/arm64-v8a/liboperit_busybox.so",
    "lib/arm64-v8a/liboperit_loader.so",
    "lib/arm64-v8a/liboperit_proot.so",
    "assets/android-runtime/arm64-v8a/rootfs.tar.gz.bin",
    "assets/android-runtime/arm64-v8a/rootfs.tar.gz.bin.sha256",
}


def verify(apk: Path) -> str:
    with zipfile.ZipFile(apk) as archive:
        broken = archive.testzip()
        if broken:
            raise RuntimeError(f"Corrupt ZIP entry: {broken}")
        names = set(archive.namelist())
        missing = REQUIRED - names
        if missing:
            raise RuntimeError(f"Missing APK payload: {sorted(missing)}")
        empty = [name for name in REQUIRED if archive.getinfo(name).file_size == 0]
        if empty:
            raise RuntimeError(f"Empty APK payload: {empty}")
        abis = {name.split('/')[1] for name in names
                if name.startswith('lib/') and name.endswith('.so')}
        if abis != {"arm64-v8a"}:
            raise RuntimeError(f"Unexpected APK architectures: {sorted(abis)}")
    with apk.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return digest


if __name__ == '__main__':
    apk = Path(sys.argv[1])
    digest = verify(apk)
    (apk.parent / 'SHA256SUMS').write_text(f'{digest}  {apk.name}\n', encoding='ascii')
    print(f'APK structure verified; SHA-256: {digest}')
    print('This does NOT verify installation, application launch, or runtime behavior.')
