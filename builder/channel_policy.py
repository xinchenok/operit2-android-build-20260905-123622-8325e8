"""Separate stock compatibility builds from explicit application modifications."""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHANNELS = ('original', 'enhanced')

def channel() -> str:
    value = os.environ.get('OPERIT2_CHANNEL', 'original')
    if value not in CHANNELS:
        raise ValueError(f'Unknown build channel: {value}')
    return value

def modification_identity() -> dict:
    kind = channel()
    if kind == 'original':
        return {'channel': kind, 'application_modifications': False}
    digest = hashlib.sha256()
    # Actual patch/importer bytes, not a manually maintained cache version alone.
    for name in ('builder/enhanced_source.py', 'builder/legacy_packages.py',
                 'builder/plugin_payload.py', 'enhancements/chat-fixes.json'):
        digest.update(name.encode()); digest.update(b'\0'); digest.update((ROOT/name).read_bytes())
    return {'channel': kind, 'application_modifications': True,
            'patch_digest': digest.hexdigest(),
            'legacy_source_commit': os.environ.get('LEGACY_SHA', '')}

def prepare_application(root: Path) -> dict:
    identity = modification_identity()
    if channel() == 'original':
        print('Original channel: application/UX/plugin sources are not patched or imported.', flush=True)
        return identity
    from enhanced_source import apply_patches
    from legacy_packages import install
    from plugin_payload import apply_source_fixes
    # Read required input before modifying the application.
    legacy_root = os.environ.get('LEGACY_SOURCE_ROOT', '')
    sha = os.environ.get('LEGACY_SHA', '')
    if not legacy_root or not sha:
        raise RuntimeError('Enhanced build needs its resolved Operit source checkout')
    identity['chat_fixes'] = apply_patches(root)
    identity['plugin_fixes'] = apply_source_fixes(root)
    identity['legacy_packages'] = install(root, Path(legacy_root), sha)
    output = root/'tools/release/dist/compatibility/enhanced/build-channel.json'
    output.write_text(json.dumps(identity, ensure_ascii=False, indent=2)+'\n',encoding='utf-8')
    return identity
