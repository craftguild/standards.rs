#!/usr/bin/env python3
"""Package verified release-host assets without compiling or publishing."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from engine import verified_engine


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_checkout(root, revision):
    if not (root / '.git').exists() or (root / '.git').is_symlink():
        raise ValueError('Delivery requires Git at its explicit source root')
    environment = {k: v for k, v in os.environ.items() if k not in {
        'GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE',
        'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES'}}
    command = ['git', '--no-optional-locks', '-c', 'core.fsmonitor=false',
               '--git-dir', str(root / '.git'), '--work-tree', str(root)]
    head = subprocess.check_output(command + ['rev-parse', 'HEAD'], env=environment).decode().strip()
    dirty = subprocess.check_output(command + ['status', '--porcelain=v1', '-z'], env=environment)
    if head != revision or dirty:
        raise ValueError('Delivery checkout must match the clean verified build revision')


def package(root, destination):
    receipt = json.loads((root / '.craftguild/craftguild-standards-engine/receipt.json').read_text())
    build = receipt['prepared']['build']
    revision = build['commit']
    if (build['dirty'] != 'clean' or build['profile'] != 'release'
            or len(revision) != 40 or any(c not in '0123456789abcdef' for c in revision)
            or receipt['context']['host'] != build['target']):
        raise ValueError('Delivery requires a clean full-revision release build for this host')
    verify_checkout(root, revision)
    if destination.exists() or destination.is_symlink():
        raise ValueError('Delivery destination must not already exist')
    destination.parent.mkdir(parents=True, exist_ok=True)
    version, target = build['version'], build['target']
    suffix = f'v{version}-{target}'
    with tempfile.TemporaryDirectory(prefix='delivery-', dir=destination.parent) as name:
        temporary = Path(name)
        artifacts = temporary / 'artifacts'
        subprocess.run([str(verified_engine(root)), "package", "--group", "standards",
                        "--output", str(artifacts)], cwd=root, check=True)
        (artifacts / f"SHA256SUMS-{target}.txt").unlink()
        binary = verified_engine(root)
        identity = json.loads(subprocess.check_output([str(binary), 'engine-identity'], cwd=root))
        if (identity['command'] != 'craftguild-standards-engine'
                or identity['engine'] != {'version': version, 'revision': revision,
                                  'baseline': '0.1.0', 'protocol': receipt['protocol'],
                                  'entry_protocol': 'craftguild-entry-1'}
                or identity['target'] != target or identity['clean'] is not True):
            raise ValueError('Engine release identity differs from its verified receipt')
        base = f'https://github.com/craftguild/standards.rs/releases/download/v{version}/'
        roles = {}
        for role, command in [('initializer', 'craftguild-standards'),
                              ('engine', 'craftguild-standards-engine')]:
            stem = f'{command}-{suffix}'
            roles[role] = {
                'binary': {'url': base + stem, 'sha256': digest(artifacts / stem)},
                'sbom': {'url': base + stem + '.bom.json',
                         'sha256': digest(artifacts / (stem + '.bom.json'))}}
        record = {'schema_version': 3, 'identity': identity['engine'],
                  'hosts': {target: roles}}
        (artifacts / f'craftguild-release-{suffix}.json').write_text(json.dumps(record, indent=2) + '\n')
        checksums = ''.join(f'{digest(p)}  {p.name}\n' for p in sorted(artifacts.iterdir()))
        (artifacts / f'SHA256SUMS-{target}.txt').write_text(checksums)
        for artifact in ['standards', 'standards-engine']:
            subprocess.run([str(binary), 'verify-receipt', '--artifact', artifact], cwd=root, check=True)
        verify_checkout(root, revision)
        artifacts.rename(destination)
    print(destination)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    package(Path(__file__).resolve().parent.parent, args.output.resolve())
