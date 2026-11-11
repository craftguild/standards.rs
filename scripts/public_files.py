#!/usr/bin/env python3
"""Review or export an explicit public inventory without including local state."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess

ROOT_FILES = {
    '.gitignore', 'AGENTS.md', 'CLAUDE.md', 'README.md', 'Cargo.toml',
    'Cargo.lock', 'Makefile', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'craftguild.toml',
    'package.json', 'package-lock.json',
}
TREES = ('.cargo/', '.github/', 'adapters/', 'assets/', 'conformance/', 'crates/', 'docs/',
         'examples/', 'schemas/', 'scripts/', 'skills/', 'spec/', 'templates/', 'tests/')
PRIVATE = {'.git', '.private', '.craftguild', 'target', 'node_modules', 'dist',
           '__pycache__', '.DS_Store'}


def inventory(root):
    output = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'],
        cwd=root)
    names = sorted({name.decode() for name in output.split(b'\0') if name})
    records = []
    for name in names:
        relative = PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts or str(relative) != name:
            raise ValueError('Unsafe inventory path: ' + name)
        if set(relative.parts) & PRIVATE or name.endswith('.pyc'):
            continue
        if name not in ROOT_FILES and not name.startswith(TREES):
            raise ValueError('Classify new public root before export: ' + name)
        path = root / name
        if path.is_symlink() or any(p.is_symlink() for p in path.parents if p != root):
            raise ValueError('Symlink in public inventory: ' + name)
        if not path.exists():
            continue
        if not path.is_file():
            raise ValueError('Nonregular public file: ' + name)
        data = path.read_bytes()
        # Bytes are assembled so this scanner's own source remains exportable.
        forbidden = [b'/Users' + b'/', b'/Volumes' + b'/data/', b'/home' + b'/']
        if any(value in data for value in forbidden):
            raise ValueError('Personal absolute path in public file: ' + name)
        records.append({'path': name, 'bytes': len(data),
                        'sha256': hashlib.sha256(data).hexdigest()})
    if not records:
        raise ValueError('Empty public inventory')
    return {'schema_version': 1, 'files': records}


def export(root, destination, records):
    if destination.exists():
        raise ValueError('Export destination must not already exist')
    destination.mkdir(parents=True)
    for record in records['files']:
        target = destination / record['path']
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / record['path'], target)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--export', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    records = inventory(root)
    if args.export:
        export(root, args.export, records)
    print(json.dumps(records, indent=2))


if __name__ == '__main__':
    main()
